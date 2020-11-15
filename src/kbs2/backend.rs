use age::Decryptor;
use anyhow::{anyhow, Context, Result};
use secrecy::{ExposeSecret, SecretString};

use std::io::{Read, Write};
use std::path::Path;

use crate::kbs2::config;
use crate::kbs2::record::Record;
use crate::kbs2::util;

/// Represents the operations that all age backends are capable of.
pub trait Backend {
    /// Creates an age keypair, saving the private component to the given path.
    ///
    /// NOTE: The private component is written in an ASCII-armored format.
    fn create_keypair(path: &Path) -> Result<String>
    where
        Self: Sized;

    /// Creates a wrapped age keypair with the given password, saving the encrypted private
    /// component to the given path.
    ///
    /// NOTE: Like `create_keypair`, this writes an ASCII-armored private component.
    fn create_wrapped_keypair(password: SecretString, path: &Path) -> Result<String>
    where
        Self: Sized;

    /// Encrypts the given record, returning it as an ASCII-armored string.
    fn encrypt(&self, record: &Record) -> Result<String>;

    /// Decrypts the given ASCII-armored string, returning it as a Record.
    fn decrypt(&self, encrypted: &str) -> Result<Record>;
}

/// Encapsulates the age crate (i.e., the `rage` CLI's backing library).
pub struct RageLib {
    pub pubkey: age::keys::RecipientKey,
    pub identities: Vec<age::keys::Identity>,
}

impl RageLib {
    pub fn new(config: &config::Config) -> Result<RageLib> {
        let pubkey = config
            .public_key
            .parse::<age::keys::RecipientKey>()
            .map_err(|e| anyhow!("unable to parse public key (backend reports: {:?})", e))?;

        let identities = if config.wrapped {
            log::debug!("config specifies a wrapped key");

            let wrapped_key = std::fs::read(&config.keyfile)?;

            // Get the user's master password from an OS-supplied keyring.
            let password = {
                let keyring = util::open_keyring(&config.keyfile);
                keyring
                    .get_password()
                    .map(SecretString::new)
                    .with_context(|| format!("missing master password for {}", config.keyfile))?
            };

            // Create a new decryptor for the wrapped key.
            let decryptor = match Decryptor::new(wrapped_key.as_slice()) {
                Ok(Decryptor::Passphrase(d)) => d,
                Ok(_) => {
                    return Err(anyhow!(
                        "key unwrap failed; not a password-wrapped keyfile?"
                    ));
                }
                Err(e) => {
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
                .or_else(|e| Err(e))?;
            log::debug!("finished key unwrap!");

            age::keys::Identity::from_buffer(unwrapped_key.as_bytes())?
        } else {
            age::keys::Identity::from_file(config.keyfile.clone())?
        };
        log::debug!("successfully parsed a private key!");

        if identities.len() != 1 {
            return Err(anyhow!(
                "expected exactly one private key in the keyfile, but got {}",
                identities.len()
            ));
        }

        Ok(RageLib { pubkey, identities })
    }
}

impl Backend for RageLib {
    fn create_keypair(path: &Path) -> Result<String> {
        let keypair = age::SecretKey::generate();

        std::fs::write(path, keypair.to_string().expose_secret())?;

        Ok(keypair.to_public().to_string())
    }

    fn create_wrapped_keypair(password: SecretString, path: &Path) -> Result<String> {
        let keypair = age::SecretKey::generate();

        let wrapped_key = {
            let encryptor = age::Encryptor::with_user_passphrase(password);

            let mut wrapped_key = vec![];
            let mut writer = encryptor.wrap_output(&mut wrapped_key, age::Format::AsciiArmor)?;
            writer.write_all(keypair.to_string().expose_secret().as_bytes())?;
            writer.finish()?;

            wrapped_key
        };

        std::fs::write(path, wrapped_key)?;

        Ok(keypair.to_public().to_string())
    }

    fn encrypt(&self, record: &Record) -> Result<String> {
        let encryptor = age::Encryptor::with_recipients(vec![self.pubkey.clone()]);
        let mut encrypted = vec![];
        let mut writer = encryptor.wrap_output(&mut encrypted, age::Format::AsciiArmor)?;
        writer.write_all(serde_json::to_string(record)?.as_bytes())?;
        writer.finish()?;

        Ok(String::from_utf8(encrypted)?)
    }

    fn decrypt(&self, encrypted: &str) -> Result<Record> {
        let decryptor = match age::Decryptor::new(encrypted.as_bytes())
            .map_err(|e| anyhow!("unable to load private key (backend reports: {:?})", e))?
        {
            age::Decryptor::Recipients(d) => d,
            // NOTE(ww): we should be fully unwrapped (if we were wrapped to begin with)
            // in this context, so all other kinds of keys should be unreachable here.
            _ => unreachable!(),
        };

        let mut decrypted = String::new();

        decryptor
            .decrypt(&self.identities)
            .map_err(|e| anyhow!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut decrypted)
                    .map_err(|_| anyhow!("i/o error while decrypting"))
            })?;

        Ok(serde_json::from_str(&decrypted)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ragelib_backend() -> Box<dyn Backend> {
        let key = age::SecretKey::generate();

        Box::new(RageLib {
            pubkey: key.to_public(),
            identities: vec![key.into()],
        })
    }

    fn ragelib_backend_bad_keypair() -> Box<dyn Backend> {
        let key1 = age::SecretKey::generate();
        let key2 = age::SecretKey::generate();

        Box::new(RageLib {
            pubkey: key1.to_public(),
            identities: vec![key2.into()],
        })
    }

    #[test]
    fn test_ragelib_create_keypair() {
        let keyfile = tempfile::NamedTempFile::new().unwrap();

        assert!(RageLib::create_keypair(keyfile.path()).is_ok());
    }

    #[test]
    fn test_ragelib_encrypt() {
        {
            let backend = ragelib_backend();
            let record = Record::login("foo", "username", "password");
            assert!(backend.encrypt(&record).is_ok());
        }

        // TODO: Test RageLib::encrypt failure modes.
    }

    #[test]
    fn test_ragelib_decrypt() {
        {
            let backend = ragelib_backend();
            let record = Record::login("foo", "username", "password");

            let encrypted = backend.encrypt(&record).unwrap();
            let decrypted = backend.decrypt(&encrypted).unwrap();

            assert_eq!(record, decrypted);
        }

        {
            let backend = ragelib_backend_bad_keypair();
            let record = Record::login("foo", "username", "password");

            let encrypted = backend.encrypt(&record).unwrap();
            let err = backend.decrypt(&encrypted).unwrap_err();

            assert_eq!(
                err.to_string(),
                "unable to decrypt (backend reports: NoMatchingKeys)"
            );
        }
    }
}
