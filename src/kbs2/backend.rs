use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::kbs2::config;
use crate::kbs2::error::Error;
use crate::kbs2::record::Record;

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum BackendKind {
    AgeCLI,
    RageCLI,
    RageLib,
}

impl Default for BackendKind {
    fn default() -> Self {
        BackendKind::RageLib
    }
}

pub trait Backend {
    fn create_keypair(path: &Path) -> Result<String, Error>
    where
        Self: Sized;
    fn encrypt(&self, config: &config::Config, record: &Record) -> Result<String, Error>;
    fn decrypt(&self, config: &config::Config, encrypted: &str) -> Result<Record, Error>;
}

pub trait CLIBackend {
    fn age() -> &'static str;
    fn age_keygen() -> &'static str;
}

impl<T> Backend for T
where
    T: CLIBackend,
{
    fn create_keypair(path: &Path) -> Result<String, Error> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        match Command::new(T::age_keygen()).arg("-o").arg(path).output() {
            Err(e) => Err(e.into()),
            Ok(output) => {
                log::debug!("output: {:?}", output);
                let public_key = {
                    let stderr = String::from_utf8(output.stderr)?;
                    stderr
                        .trim_start_matches("Public key: ")
                        .trim_end()
                        .to_string()
                };
                Ok(public_key)
            }
        }
    }

    fn encrypt(&self, config: &config::Config, record: &Record) -> Result<String, Error> {
        let mut child = Command::new(T::age())
            .arg("-a")
            .arg("-r")
            .arg(&config.public_key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "couldn't get input for encrypting")?;
            stdin.write_all(serde_json::to_string(record)?.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        log::debug!("output: {:?}", output);

        if !output.status.success() {
            return Err("encryption failed; misformatted key or empty input?".into());
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn decrypt(&self, config: &config::Config, encrypted: &str) -> Result<Record, Error> {
        let mut child = Command::new(T::age())
            .arg("-d")
            .arg("-i")
            .arg(&config.keyfile)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "couldn't get input for decrypting")?;
            stdin.write_all(encrypted.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            return Err("decryption failed; bad key or corrupted record?".into());
        }

        Ok(serde_json::from_str(std::str::from_utf8(&output.stdout)?)?)
    }
}

pub struct AgeCLI {}

impl CLIBackend for AgeCLI {
    fn age() -> &'static str {
        "age"
    }

    fn age_keygen() -> &'static str {
        "age-keygen"
    }
}

pub struct RageCLI {}

impl CLIBackend for RageCLI {
    fn age() -> &'static str {
        "rage"
    }

    fn age_keygen() -> &'static str {
        "rage-keygen"
    }
}

pub struct RageLib {
    pubkey: age::keys::RecipientKey,
    identities: Vec<age::keys::Identity>,
}

impl RageLib {
    pub fn new(config: &config::Config) -> Result<RageLib, Error> {
        let pubkey = config
            .public_key
            .parse::<age::keys::RecipientKey>()
            .map_err(|e| format!("unable to parse public key (backend reports: {:?})", e))?;

        let identities = age::keys::Identity::from_file(config.keyfile.clone())?;

        if identities.len() != 1 {
            return Err(format!(
                "expected exactly one private key in the keyfile, but got {}",
                identities.len()
            )
            .into());
        }

        Ok(RageLib { pubkey, identities })
    }
}

impl Backend for RageLib {
    fn create_keypair(path: &Path) -> Result<String, Error> {
        let key = age::SecretKey::generate();

        std::fs::write(path, key.to_string().expose_secret())?;

        Ok(key.to_public().to_string())
    }

    fn encrypt(&self, _config: &config::Config, record: &Record) -> Result<String, Error> {
        let encryptor = age::Encryptor::with_recipients(vec![self.pubkey.clone()]);
        let mut encrypted = vec![];
        let mut writer = encryptor.wrap_output(&mut encrypted, age::Format::AsciiArmor)?;
        writer.write_all(serde_json::to_string(record)?.as_bytes())?;
        writer.finish()?;

        Ok(String::from_utf8(encrypted)?)
    }

    fn decrypt(&self, _config: &config::Config, encrypted: &str) -> Result<Record, Error> {
        let decryptor = match age::Decryptor::new(encrypted.as_bytes())
            .map_err(|e| format!("unable to load private key (backend reports: {:?})", e))?
        {
            age::Decryptor::Recipients(d) => d,
            // NOTE(ww): kbs2 doesn't support secret keys with passphrases.
            _ => unreachable!(),
        };

        let mut decrypted = String::new();

        decryptor
            .decrypt(&self.identities)
            .map_err(|e| format!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut decrypted)
                    .map_err(|_| "i/o error while decrypting".into())
            })?;

        Ok(serde_json::from_str(&decrypted)?)
    }
}
