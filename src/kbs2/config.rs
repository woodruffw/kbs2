use age::Decryptor;
use anyhow::{anyhow, Error, Result};
use memmap::Mmap;
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::mman;
use nix::sys::stat::Mode;
use nix::unistd;
use serde::{de, Deserialize, Serialize};
use toml;

use std::convert::TryInto;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::ops::DerefMut;
use std::os::unix::io::FromRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::kbs2::backend::{Backend, BackendKind, RageLib};
use crate::kbs2::generator::Generator;
use crate::kbs2::util;

// The default base config directory name, placed relative to the user's config
// directory by default.
pub static CONFIG_BASEDIR: &str = "kbs2";

// The default basename for the main config file, relative to the configuration
// directory.
pub static CONFIG_BASENAME: &str = "kbs2.conf";

// The default generate age key is placed in this file, relative to
// the configuration directory.
pub static DEFAULT_KEY_BASENAME: &str = "key";

// The name for the POSIX shared memory object in which the unwrapped key is stored.
pub static UNWRAPPED_KEY_SHM_NAME: &str = "/__kbs2_unwrapped_key";

// The default base directory name for the secret store, placed relative to
// the user's data directory by default.
pub static STORE_BASEDIR: &str = "kbs2";

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(rename = "age-backend")]
    pub age_backend: BackendKind,
    #[serde(rename = "public-key")]
    pub public_key: String,
    #[serde(deserialize_with = "deserialize_with_tilde")]
    pub keyfile: String,
    pub wrapped: bool,
    #[serde(deserialize_with = "deserialize_with_tilde")]
    pub store: String,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "pre-hook")]
    #[serde(default)]
    pub pre_hook: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    #[serde(default)]
    pub post_hook: Option<String>,
    #[serde(default)]
    #[serde(rename = "reentrant-hooks")]
    pub reentrant_hooks: bool,
    #[serde(default)]
    pub generators: Vec<GeneratorConfig>,
    #[serde(default)]
    pub commands: CommandConfigs,
}

impl Config {
    // Hooks have the following behavior:
    // 1. If reentrant-hooks is true *or* KBS2_HOOK is *not* present in the environment,
    //    the hook is run.
    // 2. If reentrant-hooks is false (the default) *and* KBS2_HOOK is already present
    //    (indicating that we're already at least one layer deep), nothing is run.
    pub fn call_hook(&self, cmd: &str, args: &[&str]) -> Result<()> {
        if self.reentrant_hooks || env::var("KBS2_HOOK").is_err() {
            let success = Command::new(cmd)
                .args(args)
                .current_dir(Path::new(&self.store))
                .env("KBS2_HOOK", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .status()
                .map(|s| s.success())
                .map_err(|_| anyhow!("failed to run hook: {}", cmd))?;

            if success {
                Ok(())
            } else {
                Err(anyhow!("hook exited with an error code: {}", cmd))
            }
        } else {
            util::warn("nested hook requested without reentrant-hooks; skipping");
            Ok(())
        }
    }

    pub fn get_generator(&self, name: &str) -> Option<Box<&dyn Generator>> {
        for generator_config in self.generators.iter() {
            let generator = generator_config.as_box();
            if generator.name() == name {
                return Some(generator);
            }
        }

        None
    }

    pub fn unwrap_keyfile(&self) -> Result<fs::File> {
        // Unwrapping our password-protected keyfile and returning it as a raw file descriptor
        // is a multi-step process.

        // First, create the shared memory object that we'll eventually use
        // to stash the unwrapped key. We do this early to allow it to fail ahead
        // of the password prompt and decryption steps.
        log::debug!("creating shared memory object");
        let unwrapped_fd = match mman::shm_open(
            UNWRAPPED_KEY_SHM_NAME,
            OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR,
        ) {
            Ok(unwrapped_fd) => unwrapped_fd,
            Err(nix::Error::Sys(Errno::EEXIST)) => {
                return Err(anyhow!("unwrapped key already exists"))
            }
            Err(e) => return Err(e.into()),
        };

        // Prompt the user for their "master" password (i.e., the one that decrypts their privkey).
        let password = util::get_password().or_else(|e| {
            mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
            Err(e)
        })?;

        // Read the wrapped key from disk.
        let wrapped_key = std::fs::read(&self.keyfile).or_else(|e| {
            mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
            Err(Error::from(e))
        })?;

        // Create a new decryptor for the wrapped key.
        let decryptor = match Decryptor::new(wrapped_key.as_slice()) {
            Ok(Decryptor::Passphrase(d)) => d,
            Ok(_) => {
                mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
                return Err(anyhow!(
                    "key unwrap failed; not a password-wrapped keyfile?"
                ));
            }
            Err(e) => {
                mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
                return Err(anyhow!(
                    "unable to load private key (backend reports: {:?})",
                    e
                ));
            }
        };

        // ...and decrypt (i.e., unwrap) using the master password supplied above.
        log::debug!("beginning key unwrap...");
        let mut unwrapped_key = String::new();

        // NOTE(ww): A work factor of 18 is an educated guess here; rage generated some
        // encrypted messages that needed this factor.
        decryptor
            .decrypt(&password, Some(18))
            .map_err(|e| anyhow!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut unwrapped_key)
                    .map_err(|_| anyhow!("i/o error while decrypting"))
            })
            .or_else(|e| {
                mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
                Err(e)
            })?;
        log::debug!("finished key unwrap!");

        // Use ftruncate to tell the shared memory region how much space we'd like.
        // NOTE(ww): as_bytes returns usize, but ftruncate takes an i64.
        // We're already in big trouble if this conversion fails, so just unwrap.
        log::debug!("truncating shm obj");
        unistd::ftruncate(
            unwrapped_fd,
            unwrapped_key.as_bytes().len().try_into().unwrap(),
        )
        .or_else(|e| {
            mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
            Err(Error::from(e))
        })?;

        // NOTE(ww): This is safe, assuming nix::shm_open doesn't lie about
        // success when returning a file descriptor.
        let file = unsafe { fs::File::from_raw_fd(unwrapped_fd) };

        // Toss unwrapped_key into our shared memory.
        // Ideally we'd just call write(2) here, but that only works on Linux.
        // See the NOTE under RageLib::new() for more ranting about
        // macOS concessions.
        log::debug!("writing the unwrapped key");
        {
            let mut mmap = unsafe { Mmap::map(&file)? }.make_mut().or_else(|e| {
                mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
                Err(Error::from(e))
            })?;

            mmap.deref_mut()
                .write_all(unwrapped_key.as_bytes())
                .or_else(|e| {
                    mman::shm_unlink(UNWRAPPED_KEY_SHM_NAME)?;
                    Err(Error::from(e))
                })?;
        }

        Ok(file)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GeneratorConfig {
    Command(GeneratorCommandConfig),
    Internal(GeneratorInternalConfig),
}

impl GeneratorConfig {
    fn as_box(&self) -> Box<&dyn Generator> {
        match self {
            GeneratorConfig::Command(g) => Box::new(g),
            GeneratorConfig::Internal(g) => Box::new(g),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeneratorCommandConfig {
    pub name: String,
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeneratorInternalConfig {
    pub name: String,
    pub alphabet: String,
    pub length: u32,
}

impl Default for GeneratorInternalConfig {
    fn default() -> Self {
        GeneratorInternalConfig {
            name: "default".into(),
            // NOTE(ww): This alphabet should be a decent default, as it contains
            // symbols but not commonly blacklisted ones (e.g. %, $).
            alphabet: "abcdefghijklmnopqrstuvwxyz0123456789(){}[]-_+=".into(),
            length: 16,
        }
    }
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct CommandConfigs {
    pub new: NewConfig,
    pub pass: PassConfig,
    pub edit: EditConfig,
    pub rm: RmConfig,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NewConfig {
    // TODO(ww): This deserialize_with is ugly. There's probably a better way to do this.
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "pre-hook")]
    pub pre_hook: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PassConfig {
    #[serde(rename = "clipboard-duration")]
    pub clipboard_duration: u64,
    #[serde(rename = "clear-after")]
    pub clear_after: bool,
    #[serde(rename = "x11-clipboard")]
    pub x11_clipboard: X11Clipboard,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "pre-hook")]
    pub pre_hook: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "clear-hook")]
    pub clear_hook: Option<String>,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum X11Clipboard {
    Clipboard,
    Primary,
}

impl Default for PassConfig {
    fn default() -> Self {
        PassConfig {
            clipboard_duration: 10,
            clear_after: true,
            x11_clipboard: X11Clipboard::Clipboard,
            pre_hook: None,
            post_hook: None,
            clear_hook: None,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EditConfig {
    pub editor: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RmConfig {
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

fn deserialize_with_tilde<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: &str = Deserialize::deserialize(deserializer)?;
    Ok(shellexpand::tilde(unexpanded).into_owned())
}

fn deserialize_optional_with_tilde<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: Option<&str> = Deserialize::deserialize(deserializer)?;

    match unexpanded {
        Some(unexpanded) => Ok(Some(shellexpand::tilde(unexpanded).into_owned())),
        None => Ok(None),
    }
}

pub fn find_config_dir() -> Result<PathBuf> {
    let home = util::home_dir()?;
    Ok(home.join(".config").join(CONFIG_BASEDIR))
}

fn data_dir() -> Result<PathBuf> {
    let home = util::home_dir()?;
    Ok(home.join(".local/share").join(STORE_BASEDIR))
}

pub fn initialize(config_dir: &Path, wrapped: bool) -> Result<()> {
    // NOTE(ww): Default initialization uses the rage-lib backend unconditionally.
    let keyfile = config_dir.join(DEFAULT_KEY_BASENAME);

    let public_key = if wrapped {
        RageLib::create_wrapped_keypair(&keyfile)?
    } else {
        RageLib::create_keypair(&keyfile)?
    };

    log::debug!("public key: {}", public_key);

    #[allow(clippy::redundant_field_names)]
    let serialized = toml::to_string(&Config {
        age_backend: BackendKind::RageLib,
        public_key: public_key,
        keyfile: keyfile.to_str().unwrap().into(),
        wrapped: wrapped,
        store: data_dir()?.to_str().unwrap().into(),
        pre_hook: None,
        post_hook: None,
        reentrant_hooks: false,
        generators: vec![GeneratorConfig::Internal(Default::default())],
        commands: Default::default(),
    })?;

    fs::write(config_dir.join(CONFIG_BASENAME), serialized)?;

    Ok(())
}

pub fn load(config_dir: &Path) -> Result<Config> {
    let config_path = config_dir.join(CONFIG_BASENAME);
    let contents = fs::read_to_string(config_path)?;

    toml::from_str(&contents).map_err(|e| anyhow!("config loading error: {}", e))
}

mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dummy_config() -> Config {
        Config {
            age_backend: BackendKind::RageLib,
            public_key: "not a real public key".into(),
            keyfile: "not a real private key file".into(),
            wrapped: false,
            store: "/tmp".into(),
            pre_hook: Some("true".into()),
            post_hook: Some("false".into()),
            reentrant_hooks: false,
            generators: vec![GeneratorConfig::Internal(Default::default())],
            commands: CommandConfigs {
                rm: RmConfig {
                    post_hook: Some("this-command-does-not-exist".into()),
                },
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_find_config_dir() {
        let dir = find_config_dir().unwrap();
        // NOTE: We can't check whether the main config dir exists since we create it if it
        // doesn't; instead, we just check that it isn't something weird like a regular file.
        assert!(!dir.is_file());

        let parent = dir.parent().unwrap();

        assert!(parent.exists());
        assert!(parent.is_dir());
    }

    #[test]
    fn test_initialize() {
        // TODO: Figure out how to test initialization with a wrapped key.
        // The current API requires graphical interaction.
        // {
        //     let dir = tempdir().unwrap();
        //     assert!(initialize(dir.path(), true).is_ok());
        // }

        {
            let dir = tempdir().unwrap();
            assert!(initialize(dir.path(), false).is_ok());

            let path = dir.path();
            assert!(path.exists());
            assert!(path.is_dir());

            assert!(path.join(CONFIG_BASENAME).exists());
            assert!(path.join(CONFIG_BASENAME).is_file());

            assert!(path.join(DEFAULT_KEY_BASENAME).exists());
            assert!(path.join(DEFAULT_KEY_BASENAME).is_file());
        }
    }

    #[test]
    fn test_load() {
        {
            let dir = tempdir().unwrap();
            initialize(dir.path(), false).unwrap();

            assert!(load(dir.path()).is_ok());
        }
    }

    #[test]
    fn test_call_hook() {
        let config = dummy_config();

        {
            assert!(config
                .call_hook(config.pre_hook.as_ref().unwrap(), &[])
                .is_ok());
        }

        {
            let err = config
                .call_hook(config.commands.rm.post_hook.as_ref().unwrap(), &[])
                .unwrap_err();

            assert_eq!(
                err.to_string(),
                "failed to run hook: this-command-does-not-exist"
            );
        }

        {
            let err = config
                .call_hook(config.post_hook.as_ref().unwrap(), &[])
                .unwrap_err();

            assert_eq!(err.to_string(), "hook exited with an error code: false");
        }
    }

    #[test]
    fn test_get_generator() {
        let config = dummy_config();

        assert!(config.get_generator("default").is_some());
        assert!(config.get_generator("nonexistent-generator").is_none());
    }

    // TODO: Test Config::unwrap_keyfile.
}
