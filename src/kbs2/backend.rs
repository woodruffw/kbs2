use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::kbs2::config;
use crate::kbs2::error::Error;
use crate::kbs2::record::Record;

pub trait Backend {
    fn create_keypair(&self, path: &Path) -> Result<String, Error>;
    fn encrypt(&self, config: &config::Config, record: &Record) -> Result<String, Error>;
    fn decrypt(&self, config: &config::Config, encrypted: &str) -> Result<Record, Error>;
}

pub struct AgeCLI {
    pub age: String,
    pub age_keygen: String,
}

impl Backend for AgeCLI {
    fn create_keypair(&self, path: &Path) -> Result<String, Error> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        match Command::new(&self.age_keygen).arg("-o").arg(path).output() {
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
        let mut child = Command::new(&self.age)
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
                .ok_or::<Error>("couldn't get input for encrypting".into())?;
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
        let mut child = Command::new(&self.age)
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
                .ok_or::<Error>("couldn't get input for decrypting".into())?;
            stdin.write_all(encrypted.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            return Err("decryption failed; bad key or corrupted record?".into());
        }

        Ok(serde_json::from_str(std::str::from_utf8(&output.stdout)?)?)
    }
}
