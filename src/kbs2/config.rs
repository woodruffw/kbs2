use anyhow::{anyhow, Result};
use secrecy::SecretString;
use serde::{de, Deserialize, Serialize};

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::kbs2::backend::{Backend, RageLib};
use crate::kbs2::generator::Generator;
use crate::kbs2::util;

/// The default base config directory name, placed relative to the user's config
/// directory by default.
pub static CONFIG_BASEDIR: &str = "kbs2";

/// The default basename for the main config file, relative to the configuration
/// directory.
pub static CONFIG_BASENAME: &str = "kbs2.conf";

/// The default generate age key is placed in this file, relative to
/// the configuration directory.
pub static DEFAULT_KEY_BASENAME: &str = "key";

/// The default base directory name for the secret store, placed relative to
/// the user's data directory by default.
pub static STORE_BASEDIR: &str = "kbs2";

/// The main kbs2 configuration structure.
/// The fields of this structure correspond directly to the fields
/// loaded from the configuration file.
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    /// The path to the directory that this configuration was loaded from.
    ///
    /// **NOTE**: This field is never loaded from the configuration file itself.
    #[serde(skip)]
    pub config_dir: String,

    /// The public component of the keypair.
    #[serde(rename = "public-key")]
    pub public_key: String,

    /// The path to a file containing the private component of the keypair,
    /// which may be wrapped with a passphrase.
    #[serde(deserialize_with = "deserialize_with_tilde")]
    pub keyfile: String,

    /// Whether or not the private component of the keypair is wrapped with
    /// a passphrase.
    pub wrapped: bool,

    /// The path to the directory where encrypted records are stored.
    #[serde(deserialize_with = "deserialize_with_tilde")]
    pub store: String,

    /// The pinentry binary to use for password prompts.
    #[serde(default)]
    pub pinentry: Pinentry,

    /// An optional command to run before each `kbs2` subcommand.
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "pre-hook")]
    #[serde(default)]
    pub pre_hook: Option<String>,

    /// An optional command to run after each `kbs2` subcommand, on success.
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    #[serde(default)]
    pub post_hook: Option<String>,

    /// Whether or not any hooks are called when a hook itself invokes `kbs2`.
    #[serde(default)]
    #[serde(rename = "reentrant-hooks")]
    pub reentrant_hooks: bool,

    /// Any secret generators configured by the user.
    #[serde(default)]
    pub generators: Vec<GeneratorConfig>,

    /// Per-command configuration.
    #[serde(default)]
    pub commands: CommandConfigs,
}

impl Config {
    /// Calls a command as a hook, meaning:
    /// * The command is run with the `kbs2` store as its working directory
    /// * The command is run with `KBS2_HOOK=1` in its environment
    ///
    /// Hooks have the following behavior:
    /// 1. If `reentrant-hooks` is `true` *or* `KBS2_HOOK` is *not* present in the environment,
    ///    the hook is run.
    /// 2. If `reentrant-hooks` is `false` (the default) *and* `KBS2_HOOK` is already present
    ///    (indicating that we're already in a hook), nothing is run.
    pub fn call_hook(&self, cmd: &str, args: &[&str]) -> Result<()> {
        if self.reentrant_hooks || env::var("KBS2_HOOK").is_err() {
            let success = Command::new(cmd)
                .args(args)
                .current_dir(Path::new(&self.store))
                .env("KBS2_HOOK", "1")
                .env("KBS2_CONFIG_DIR", &self.config_dir)
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

    /// Given the `name` of a configured generator, return that generator
    /// if it exists.
    pub fn get_generator(&self, name: &str) -> Option<&dyn Generator> {
        for generator_config in self.generators.iter() {
            let generator = generator_config.as_dyn();
            if generator.name() == name {
                return Some(generator);
            }
        }

        None
    }
}

/// A newtype wrapper around a `String`, used to provide a sensible default for `Config.pinentry`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Pinentry(String);

impl Default for Pinentry {
    fn default() -> Self {
        Self("pinentry".into())
    }
}

impl AsRef<OsStr> for Pinentry {
    fn as_ref(&self) -> &OsStr {
        &self.0.as_ref()
    }
}

/// The different types of generators known to `kbs2`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GeneratorConfig {
    Command(GeneratorCommandConfig),
    Internal(GeneratorInternalConfig),
}

impl GeneratorConfig {
    fn as_dyn(&self) -> &dyn Generator {
        match self {
            GeneratorConfig::Command(g) => g as &dyn Generator,
            GeneratorConfig::Internal(g) => g as &dyn Generator,
        }
    }
}

/// The configuration settings for a "command" generator.
#[derive(Debug, Deserialize, Serialize)]
pub struct GeneratorCommandConfig {
    /// The name of the generator.
    pub name: String,

    /// The command to run to generate a secret.
    pub command: String,
}

/// The configuration settings for an "internal" generator.
#[derive(Debug, Deserialize, Serialize)]
pub struct GeneratorInternalConfig {
    /// The name of the generator.
    pub name: String,

    /// The alphabet to sample from when generating a secret.
    pub alphabet: String,

    /// The number of characters to sample from the alphabet.
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

/// The per-command configuration settings known to `kbs2`.
#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct CommandConfigs {
    /// Settings for `kbs2 new`.
    pub new: NewConfig,

    /// Settings for `kbs2 pass`.
    pub pass: PassConfig,

    /// Settings for `kbs2 edit`.
    pub edit: EditConfig,

    /// Settings for `kbs2 rm`.
    pub rm: RmConfig,
}

/// Configuration settings for `kbs2 new`.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NewConfig {
    #[serde(rename = "generate-on-empty")]
    pub generate_on_empty: bool,
    // TODO(ww): This deserialize_with is ugly. There's probably a better way to do this.
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "pre-hook")]
    pub pre_hook: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

/// Configuration settings for `kbs2 pass`.
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

/// Configuration settings for `kbs2 edit`.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EditConfig {
    pub editor: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

/// Configuration settings for `kbs2 rm`.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RmConfig {
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

#[doc(hidden)]
fn deserialize_with_tilde<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: &str = Deserialize::deserialize(deserializer)?;
    Ok(shellexpand::tilde(unexpanded).into_owned())
}

#[doc(hidden)]
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

/// Returns a suitable configuration directory path for `kbs2`.
///
/// NOTE: This function always chooses `$HOME/.config/kbs2`, across all platforms.
pub fn find_config_dir() -> Result<PathBuf> {
    // TODO(ww): This should respect XDG on Linux.
    let home = util::home_dir()?;
    Ok(home.join(".config").join(CONFIG_BASEDIR))
}

/// Returns a suitable record store directory path for `kbs2`.
///
/// NOTE: This function always chooses `$HOME/.local/share/kbs2`, across all platforms.
fn store_dir() -> Result<PathBuf> {
    // TODO(ww): This should respect XDG on Linux.
    let home = util::home_dir()?;
    Ok(home.join(".local/share").join(STORE_BASEDIR))
}

/// Given a path to a `kbs2` configuration directory, initializes a configuration
/// file and keypair within it.
///
/// # Arguments
///
/// * `config_dir` - The configuration directory to initialize within
/// * `password` - An optional master password for wrapping the secret
pub fn initialize(config_dir: &Path, password: Option<SecretString>) -> Result<()> {
    let keyfile = config_dir.join(DEFAULT_KEY_BASENAME);

    let mut wrapped = false;
    let public_key = if let Some(password) = password {
        wrapped = true;
        RageLib::create_wrapped_keypair(&keyfile, password)?
    } else {
        RageLib::create_keypair(&keyfile)?
    };

    log::debug!("public key: {}", public_key);

    #[allow(clippy::redundant_field_names)]
    let serialized = toml::to_string(&Config {
        // NOTE(ww): Not actually serialized; just here to make the compiler happy.
        config_dir: config_dir.to_str().unwrap().into(),
        public_key: public_key,
        keyfile: keyfile.to_str().unwrap().into(),
        wrapped: wrapped,
        store: store_dir()?.to_str().unwrap().into(),
        pinentry: Default::default(),
        pre_hook: None,
        post_hook: None,
        reentrant_hooks: false,
        generators: vec![GeneratorConfig::Internal(Default::default())],
        commands: Default::default(),
    })?;

    fs::write(config_dir.join(CONFIG_BASENAME), serialized)?;

    Ok(())
}

/// Given a path to a `kbs2` configuration directory, loads the configuration
/// file within and returns the resulting `Config`.
pub fn load(config_dir: &Path) -> Result<Config> {
    let config_path = config_dir.join(CONFIG_BASENAME);
    let contents = fs::read_to_string(config_path)?;

    Ok(Config {
        config_dir: config_dir.to_str().unwrap().into(),
        ..toml::from_str(&contents).map_err(|e| anyhow!("config loading error: {}", e))?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dummy_unwrapped_config() -> Config {
        Config {
            config_dir: "/not/a/real/dir".into(),
            public_key: "not a real public key".into(),
            keyfile: "not a real private key file".into(),
            wrapped: false,
            store: "/tmp".into(),
            pinentry: Default::default(),
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
    fn test_initialize_unwrapped() {
        {
            let dir = tempdir().unwrap();
            assert!(initialize(dir.path(), None).is_ok());

            let path = dir.path();
            assert!(path.exists());
            assert!(path.is_dir());

            assert!(path.join(CONFIG_BASENAME).exists());
            assert!(path.join(CONFIG_BASENAME).is_file());

            assert!(path.join(DEFAULT_KEY_BASENAME).exists());
            assert!(path.join(DEFAULT_KEY_BASENAME).is_file());

            let config = load(path).unwrap();
            assert!(!config.wrapped);
        }
    }

    #[test]
    fn test_initialize_wrapped() {
        {
            let dir = tempdir().unwrap();
            assert!(initialize(dir.path(), Some(SecretString::new("badpassword".into()))).is_ok());

            let path = dir.path();
            assert!(path.exists());
            assert!(path.is_dir());

            assert!(path.join(CONFIG_BASENAME).exists());
            assert!(path.join(CONFIG_BASENAME).is_file());

            assert!(path.join(DEFAULT_KEY_BASENAME).exists());
            assert!(path.join(DEFAULT_KEY_BASENAME).is_file());

            let config = load(path).unwrap();
            assert!(config.wrapped);
        }
    }

    #[test]
    fn test_load() {
        {
            let dir = tempdir().unwrap();
            initialize(dir.path(), None).unwrap();

            assert!(load(dir.path()).is_ok());
        }

        {
            let dir = tempdir().unwrap();
            initialize(dir.path(), None).unwrap();

            let config = load(dir.path()).unwrap();
            assert_eq!(dir.path().to_str().unwrap(), config.config_dir);
        }
    }

    #[test]
    fn test_call_hook() {
        let config = dummy_unwrapped_config();

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
        let config = dummy_unwrapped_config();

        assert!(config.get_generator("default").is_some());
        assert!(config.get_generator("nonexistent-generator").is_none());
    }

    // TODO: Test Config::unwrap_keyfile.
}
