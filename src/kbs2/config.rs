use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use clap::ArgMatches;
use lazy_static::lazy_static;
use secrecy::SecretString;
use serde::{de, Deserialize, Serialize};

use crate::kbs2::backend::{Backend, RageLib};
use crate::kbs2::generator::Generator;
use crate::kbs2::util;

/// The default base config directory name, placed relative to the user's config
/// directory by default.
pub static CONFIG_BASEDIR: &str = "kbs2";

/// The default basename for the main config file, relative to the configuration
/// directory.
pub static CONFIG_BASENAME: &str = "config.toml";

/// A deprecated alternative default config basename.
pub static LEGACY_CONFIG_BASENAME: &str = "kbs2.conf";

/// The default generate age key is placed in this file, relative to
/// the configuration directory.
pub static DEFAULT_KEY_BASENAME: &str = "key";

/// The default base directory name for the secret store, placed relative to
/// the user's data directory by default.
pub static STORE_BASEDIR: &str = "kbs2";

lazy_static! {
    static ref HOME: PathBuf = util::home_dir();

    // TODO(ww): Respect XDG on appropriate platforms.
    pub static ref DEFAULT_CONFIG_DIR: PathBuf = HOME.join(".config").join(CONFIG_BASEDIR);
    pub static ref DEFAULT_STORE_DIR: PathBuf = HOME.join(".local/share").join(STORE_BASEDIR);
}

/// The main kbs2 configuration structure.
/// The fields of this structure correspond directly to the fields
/// loaded from the configuration file.
#[derive(Clone, Debug, Deserialize, Serialize)]
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

    /// Whether or not to auto-start the kbs2 authentication agent when
    /// creating a session.
    #[serde(rename = "agent-autostart")]
    #[serde(default = "default_as_true")]
    pub agent_autostart: bool,

    /// Whether or not the private component of the keypair is wrapped with
    /// a passphrase.
    #[serde(default = "default_as_true")]
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

    /// An optional command to run after each `kbs2` subcommand, on error.
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "error-hook")]
    #[serde(default)]
    pub error_hook: Option<String>,

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
    pub fn generator(&self, name: &str) -> Option<&dyn Generator> {
        for generator_config in self.generators.iter() {
            let generator = generator_config.as_dyn();
            if generator.name() == name {
                return Some(generator);
            }
        }

        None
    }

    /// Create a `RuntimeConfig` from this config and the given `matches`.
    pub fn with_matches<'a>(&'a self, matches: &'a ArgMatches) -> RuntimeConfig<'a> {
        RuntimeConfig {
            config: self,
            matches,
        }
    }
}

/// A newtype wrapper around a `String`, used to provide a sensible default for `Config.pinentry`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pinentry(String);

impl Default for Pinentry {
    fn default() -> Self {
        Self("pinentry".into())
    }
}

impl AsRef<OsStr> for Pinentry {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

/// The different types of generators known to `kbs2`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GeneratorConfig {
    Command(ExternalGeneratorConfig),
    Internal(InternalGeneratorConfig),
    InternalLegacy(LegacyInternalGeneratorConfig),
}

impl GeneratorConfig {
    fn as_dyn(&self) -> &dyn Generator {
        match self {
            GeneratorConfig::Command(g) => g as &dyn Generator,
            GeneratorConfig::Internal(g) => g as &dyn Generator,
            GeneratorConfig::InternalLegacy(g) => g as &dyn Generator,
        }
    }
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        GeneratorConfig::Internal(Default::default())
    }
}

/// The configuration settings for an external (i.e., separate command) generator.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExternalGeneratorConfig {
    /// The name of the generator.
    pub name: String,

    /// The command to run to generate a secret.
    pub command: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InternalGeneratorConfig {
    /// The name of the generator.
    pub name: String,

    /// The alphabets used by the generator.
    pub alphabets: Vec<String>,

    /// The length of the secrets generated.
    pub length: usize,
}

impl Default for InternalGeneratorConfig {
    fn default() -> Self {
        InternalGeneratorConfig {
            name: "default".into(),
            alphabets: vec![
                "abcdefghijklmnopqrstuvwxyz".into(),
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ".into(),
                "0123456789".into(),
                "(){}[]-_+=".into(),
            ],
            length: 16,
        }
    }
}

/// The configuration settings for a legacy "internal" generator.
///
/// This is a **legacy** generator that will be removed in an upcoming release.
///
/// Users should prefer `InternalGeneratorConfig`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LegacyInternalGeneratorConfig {
    /// The name of the generator.
    pub name: String,

    /// The alphabet to sample from when generating a secret.
    pub alphabet: String,

    /// The length of the secrets generated.
    pub length: u32,
}

/// The per-command configuration settings known to `kbs2`.
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
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

    /// External command settings.
    pub ext: HashMap<String, HashMap<String, toml::Value>>,
}

/// Configuration settings for `kbs2 new`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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

/// Configuration settings for `kbs2 pass`.
#[derive(Clone, Debug, Deserialize, Serialize)]
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
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EditConfig {
    pub editor: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

/// Configuration settings for `kbs2 rm`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RmConfig {
    #[serde(deserialize_with = "deserialize_optional_with_tilde")]
    #[serde(rename = "post-hook")]
    pub post_hook: Option<String>,
}

/// A "view" for an active configuration, composed with some set of argument matches
/// from the command line.
pub struct RuntimeConfig<'a> {
    pub config: &'a Config,
    pub matches: &'a ArgMatches,
}

impl<'a> RuntimeConfig<'a> {
    pub fn generator(&self) -> Result<&dyn Generator> {
        // If the user explicitly requests a specific generator, use it.
        // Otherwise, use the default generator, which is always present.
        if let Some(generator) = self.matches.value_of("generator") {
            self.config
                .generator(generator)
                .ok_or_else(|| anyhow!("no generator named {generator}"))
        } else {
            // Failure here indicates a bug, since we should always have a default.
            self.config
                .generator("default")
                .ok_or_else(|| anyhow!("missing default generator?"))
        }
    }

    pub fn terse(&self) -> bool {
        atty::isnt(atty::Stream::Stdin) || self.matches.is_present("terse")
    }
}

#[doc(hidden)]
#[inline]
fn deserialize_with_tilde<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: &str = Deserialize::deserialize(deserializer)?;
    Ok(shellexpand::tilde(unexpanded).into_owned())
}

#[doc(hidden)]
#[inline]
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

#[doc(hidden)]
#[inline]
fn default_as_true() -> bool {
    // https://github.com/serde-rs/serde/issues/1030
    true
}

/// Given a path to a `kbs2` configuration directory, initializes a configuration
/// file and keypair within it.
///
/// # Arguments
///
/// * `config_dir` - The configuration directory to initialize within
/// * `store_dir` - The record store directory to use
/// * `password` - An optional master password for wrapping the secret
pub fn initialize<P: AsRef<Path>>(
    config_dir: P,
    store_dir: P,
    password: Option<SecretString>,
) -> Result<()> {
    fs::create_dir_all(&config_dir)?;

    let keyfile = config_dir.as_ref().join(DEFAULT_KEY_BASENAME);

    let mut wrapped = false;
    let public_key = if let Some(password) = password {
        wrapped = true;
        RageLib::create_wrapped_keypair(&keyfile, password)?
    } else {
        RageLib::create_keypair(&keyfile)?
    };

    log::debug!("public key: {}", public_key);

    let serialized = {
        let config_dir = config_dir
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("unencodable config dir"))?
            .into();

        let store = store_dir
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("unencodable store dir"))?
            .into();

        #[allow(clippy::redundant_field_names)]
        toml::to_string(&Config {
            // NOTE(ww): Not actually serialized; just here to make the compiler happy.
            config_dir: config_dir,
            public_key: public_key,
            keyfile: keyfile
                .to_str()
                .ok_or_else(|| anyhow!("unrepresentable keyfile path: {:?}", keyfile))?
                .into(),
            agent_autostart: true,
            wrapped: wrapped,
            store: store,
            pinentry: Default::default(),
            pre_hook: None,
            post_hook: None,
            error_hook: None,
            reentrant_hooks: false,
            generators: vec![GeneratorConfig::Internal(Default::default())],
            commands: Default::default(),
        })?
    };

    fs::write(config_dir.as_ref().join(CONFIG_BASENAME), serialized)?;

    Ok(())
}

/// Given a path to a `kbs2` configuration directory, loads the configuration
/// file within and returns the resulting `Config`.
pub fn load<P: AsRef<Path>>(config_dir: P) -> Result<Config> {
    let config_dir = config_dir.as_ref();
    let config_path = config_dir.join(CONFIG_BASENAME);

    let contents = if config_path.is_file() {
        fs::read_to_string(config_path)?
    } else {
        // Try the legacy config file. This behavior will be removed in a future stable release.
        util::warn(&format!(
            "{} not found in config dir; trying {}",
            CONFIG_BASENAME, LEGACY_CONFIG_BASENAME
        ));
        util::warn("note: this behavior will be removed in a future stable release");
        fs::read_to_string(config_dir.join(LEGACY_CONFIG_BASENAME))?
    };

    let mut config = Config {
        config_dir: config_dir
            .to_str()
            .ok_or_else(|| anyhow!("unrepresentable config dir path: {:?}", config_dir))?
            .into(),
        ..toml::from_str(&contents).map_err(|e| anyhow!("config loading error: {}", e))?
    };

    // Always put a default generator in the generator list.
    if config.generators.is_empty() {
        config.generators.push(Default::default());
    }

    // Warn if the user has any old-style generators.
    for gen in config.generators.iter() {
        if matches!(gen, GeneratorConfig::InternalLegacy(_)) {
            util::warn(&format!("loaded legacy generator: {}", gen.as_dyn().name()));
            util::warn("note: this behavior will be removed in a future stable release");
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn dummy_config_unwrapped_key() -> Config {
        Config {
            config_dir: "/not/a/real/dir".into(),
            public_key: "not a real public key".into(),
            keyfile: "not a real private key file".into(),
            agent_autostart: false,
            wrapped: false,
            store: "/tmp".into(),
            pinentry: Default::default(),
            pre_hook: Some("true".into()),
            post_hook: Some("false".into()),
            error_hook: Some("true".into()),
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
    fn test_find_default_config_dir() {
        // NOTE: We can't check whether the main config dir exists since we create it if it
        // doesn't; instead, we just check that it isn't something weird like a regular file.
        assert!(!DEFAULT_CONFIG_DIR.is_file());

        // The default config dir's parents aren't guaranteed to exist; we create them
        // if they don't.
    }

    #[test]
    fn test_find_default_store_dir() {
        // NOTE: Like above: just make sure it isn't something weird like a regular file.
        assert!(!DEFAULT_STORE_DIR.is_file());

        // The default store dir's parents aren't guaranteed to exist; we create them
        // if they don't.
    }

    #[test]
    fn test_initialize_unwrapped() {
        {
            let config_dir = tempdir().unwrap();
            let store_dir = tempdir().unwrap();
            assert!(initialize(&config_dir, &store_dir, None).is_ok());

            let config_dir = config_dir.path();
            assert!(config_dir.exists());
            assert!(config_dir.is_dir());

            assert!(config_dir.join(CONFIG_BASENAME).exists());
            assert!(config_dir.join(CONFIG_BASENAME).is_file());

            assert!(config_dir.join(DEFAULT_KEY_BASENAME).exists());
            assert!(config_dir.join(DEFAULT_KEY_BASENAME).is_file());

            let config = load(config_dir).unwrap();
            assert!(!config.wrapped);
        }
    }

    #[test]
    fn test_initialize_wrapped() {
        {
            let config_dir = tempdir().unwrap();
            let store_dir = tempdir().unwrap();
            assert!(initialize(
                &config_dir,
                &store_dir,
                Some(SecretString::new("badpassword".into()))
            )
            .is_ok());

            let config_dir = config_dir.path();
            assert!(config_dir.exists());
            assert!(config_dir.is_dir());

            assert!(config_dir.join(CONFIG_BASENAME).exists());
            assert!(config_dir.join(CONFIG_BASENAME).is_file());

            assert!(config_dir.join(DEFAULT_KEY_BASENAME).exists());
            assert!(config_dir.join(DEFAULT_KEY_BASENAME).is_file());

            let config = load(config_dir).unwrap();
            assert!(config.wrapped);
        }
    }

    #[test]
    fn test_load() {
        {
            let config_dir = tempdir().unwrap();
            let store_dir = tempdir().unwrap();
            initialize(&config_dir, &store_dir, None).unwrap();

            assert!(load(&config_dir).is_ok());
        }

        {
            let config_dir = tempdir().unwrap();
            let store_dir = tempdir().unwrap();
            initialize(&config_dir, &store_dir, None).unwrap();

            let config = load(&config_dir).unwrap();
            assert_eq!(config_dir.path().to_str().unwrap(), config.config_dir);
            assert_eq!(store_dir.path().to_str().unwrap(), config.store);
        }
    }

    #[test]
    fn test_call_hook() {
        let config = dummy_config_unwrapped_key();

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

        {
            assert!(config
                .call_hook(config.error_hook.as_ref().unwrap(), &[])
                .is_ok());
        }
    }

    #[test]
    fn test_get_generator() {
        let config = dummy_config_unwrapped_key();

        assert!(config.generator("default").is_some());
        assert!(config.generator("nonexistent-generator").is_none());
    }
}
