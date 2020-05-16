use dirs;
use serde::{de, Deserialize, Serialize};
use toml;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::kbs2::backend::{AgeCLI, Backend};
use crate::kbs2::error::Error;
use crate::kbs2::util;

// The default base config directory name, placed relative to the user's config
// directory by default.
pub static CONFIG_BASEDIR: &'static str = "kbs2";

// The default basename for the main config file, relative to the configuration
// directory.
pub static CONFIG_BASENAME: &'static str = "kbs2.conf";

// The default generate age key is placed in this file, relative to
// the configuration directory.
pub static DEFAULT_KEY_BASENAME: &'static str = "key";

// The default base directory name for the secret store, placed relative to
// the user's data directory by default.
pub static STORE_BASEDIR: &'static str = "kbs2";

pub static KNOWN_AGE_CLIS: &'static [&(&'static str, &'static str)] =
    &[&("rage", "rage-keygen"), &("age", "age-keygen")];

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(rename = "age-backend")]
    pub age_backend: String,
    #[serde(rename = "age-keygen-backend")]
    pub age_keygen_backend: String,
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
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct CommandConfigs {
    pub new: NewConfig,
    pub pass: PassConfig,
}

#[derive(Debug, Deserialize, Serialize)]
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

impl Default for NewConfig {
    fn default() -> Self {
        NewConfig {
            pre_hook: None,
            post_hook: None,
        }
    }
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
        }
    }
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
            .ok_or::<Error>("couldn't stringify user data dir".into())?
            .into()),
        None => Err("couldn't find a suitable data directory for the secret store".into()),
    }
}

pub fn find_age_cli() -> Result<AgeCLI, Error> {
    for (age, age_keygen) in KNOWN_AGE_CLIS {
        if Command::new(age).arg("-h").output().is_ok()
            && Command::new(age_keygen).arg("-h").output().is_ok()
        {
            return Ok(AgeCLI {
                age: (*age).into(),
                age_keygen: (*age_keygen).into(),
            });
        }
    }

    Err("couldn't find an age-compatible CLI".into())
}

pub fn initialize(config_dir: &Path) -> Result<(), Error> {
    let backend = find_age_cli()?;

    let keyfile = config_dir.join(DEFAULT_KEY_BASENAME);
    let public_key = backend.create_keypair(&keyfile)?;
    log::debug!("public key: {}", public_key);

    let serialized = toml::to_string(&Config {
        age_backend: backend.age,
        age_keygen_backend: backend.age_keygen,
        public_key: public_key,
        keyfile: keyfile.to_str().unwrap().into(),
        store: data_dir()?,
        pre_hook: None,
        post_hook: None,
        reentrant_hooks: false,
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
