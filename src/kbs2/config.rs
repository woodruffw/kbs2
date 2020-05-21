use dirs;
use serde::{de, Deserialize, Serialize};
use toml;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::kbs2::backend::{Backend, BackendKind, RageLib};
use crate::kbs2::error::Error;
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

// The default base directory name for the secret store, placed relative to
// the user's data directory by default.
pub static STORE_BASEDIR: &str = "kbs2";

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(rename = "age-backend")]
    pub age_backend: BackendKind,
    #[serde(rename = "public-key")]
    pub public_key: String,
    #[serde(deserialize_with = "deserialize_with_tilde")]
    pub keyfile: String,
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
    pub fn call_hook(&self, cmd: &str, args: &[&str]) -> Result<(), Error> {
        if self.reentrant_hooks || env::var("KBS2_HOOK").is_err() {
            Command::new(cmd)
                .args(args)
                .current_dir(Path::new(&self.store))
                .env("KBS2_HOOK", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .status()
                .map(|_| ())
                .map_err(|_| format!("hook failed: {}", cmd).into())
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
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct CommandConfigs {
    pub new: NewConfig,
    pub pass: PassConfig,
    pub edit: EditConfig,
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

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
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

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EditConfig {
    pub editor: Option<String>,
}

fn deserialize_with_tilde<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: &str = Deserialize::deserialize(deserializer)?;
    Ok(shellexpand::tilde(unexpanded).into_owned())
}

fn deserialize_optional_with_tilde<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let unexpanded: Option<&str> = Deserialize::deserialize(deserializer)?;

    match unexpanded {
        Some(unexpanded) => Ok(Some(shellexpand::tilde(unexpanded).into_owned())),
        None => Ok(None),
    }
}

pub fn find_config_dir() -> Result<PathBuf, Error> {
    match dirs::config_dir() {
        Some(path) => Ok(path.join(CONFIG_BASEDIR)),
        // NOTE(ww): Probably excludes *BSD users for no good reason.
        None => Err("couldn't find a suitable config directory".into()),
    }
}

fn data_dir() -> Result<String, Error> {
    match dirs::data_dir() {
        Some(dir) => Ok(dir
            .join(STORE_BASEDIR)
            .to_str()
            .ok_or_else(|| "couldn't stringify user data dir")?
            .into()),
        None => Err("couldn't find a suitable data directory for the secret store".into()),
    }
}

pub fn initialize(config_dir: &Path) -> Result<(), Error> {
    // NOTE(ww): Default initialization uses the rage-lib backend unconditionally.
    let keyfile = config_dir.join(DEFAULT_KEY_BASENAME);
    let public_key = RageLib::create_keypair(&keyfile)?;

    log::debug!("public key: {}", public_key);

    #[allow(clippy::redundant_field_names)]
    let serialized = toml::to_string(&Config {
        age_backend: BackendKind::RageLib,
        public_key: public_key,
        keyfile: keyfile.to_str().unwrap().into(),
        store: data_dir()?,
        pre_hook: None,
        post_hook: None,
        reentrant_hooks: false,
        generators: vec![GeneratorConfig::Internal(Default::default())],
        commands: Default::default(),
    })?;

    fs::write(config_dir.join(CONFIG_BASENAME), serialized)?;

    Ok(())
}

pub fn load(config_dir: &Path) -> Result<Config, Error> {
    let config_path = config_dir.join(CONFIG_BASENAME);
    let contents = fs::read_to_string(config_path)?;

    toml::from_str(&contents).map_err(|e| format!("config loading error: {}", e).into())
}
