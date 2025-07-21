use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;

use age::armor::{ArmoredReader, ArmoredWriter, Format};
use age::secrecy::{ExposeSecret as _, SecretString};
use age::Decryptor;
use anyhow::{anyhow, Context, Result};

use crate::kbs2::agent;
use crate::kbs2::config;
use crate::kbs2::record::Record;
use crate::kbs2::util;

/// The maximum size of a wrapped key file, on disk.
///
/// This is an **extremely** conservative maximum: actual plain-text formatted
/// wrapped keys should never be more than a few hundred bytes. But we need some
/// number of harden the I/O that the agent does, and a single page/4K seems reasonable.
pub const MAX_WRAPPED_KEY_FILESIZE: u64 = 4096;

/// Represents the operations that all age backends are capable of.
pub trait Backend {
    /// Creates an age keypair, saving the private component to the given path.
    ///
    /// NOTE: The private component is written in an ASCII-armored format.
    fn create_keypair<P: AsRef<Path>>(path: P) -> Result<String>;

    /// Creates a wrapped age keypair, saving the encrypted private component to the
    /// given path.
    ///
    /// NOTE: Like `create_keypair`, this writes an ASCII-armored private component.
    fn create_wrapped_keypair<P: AsRef<Path>>(path: P, password: SecretString) -> Result<String>;

    /// Unwraps the given `keyfile` using `password`, returning the unwrapped contents.
    fn unwrap_keyfile<P: AsRef<Path>>(keyfile: P, password: SecretString) -> Result<SecretString>;

    /// Wraps the given `key` using the given `password`, returning the wrapped result.
    fn wrap_key(key: SecretString, password: SecretString) -> Result<Vec<u8>>;

    /// Rewraps the given keyfile in place, decrypting it with the `old` password
    /// and re-encrypting it with the `new` password.
    ///
    /// NOTE: This function does *not* make a backup of the original keyfile.
    fn rewrap_keyfile<P: AsRef<Path>>(path: P, old: SecretString, new: SecretString) -> Result<()>;

    /// Encrypts the given record, returning it as an ASCII-armored string.
    fn encrypt(&self, record: &Record) -> Result<String>;

    /// Decrypts the given ASCII-armored string, returning it as a Record.
    fn decrypt(&self, encrypted: &str) -> Result<Record>;
}

/// Encapsulates the age crate (i.e., the `rage` CLI's backing library).
pub struct RageLib {
    pub pubkey: age::x25519::Recipient,
    pub identity: age::x25519::Identity,
}

impl RageLib {
    pub fn new(config: &config::Config) -> Result<RageLib> {
        let pubkey = config
            .public_key
            .parse::<age::x25519::Recipient>()
            .map_err(|e| anyhow!("unable to parse public key (backend reports: {:?})", e))?;

        let identity = if config.wrapped {
            log::debug!("config specifies a wrapped key");

            let client = agent::Client::new().with_context(|| "failed to connect to kbs2 agent")?;

            if !client.query_key(&config.public_key)? {
                client.add_key(
                    &config.public_key,
                    &config.keyfile,
                    util::get_password(None, &config.pinentry)?,
                )?;
            }

            let unwrapped_key = client
                .get_key(&config.public_key)
                .with_context(|| format!("agent has no unwrapped key for {}", config.keyfile))?;

            log::debug!("parsing unwrapped key");
            age::x25519::Identity::from_str(&unwrapped_key)
                .map_err(|e| anyhow!("failed to parse unwrapped key ({e:?})",))?
        } else {
            let unwrapped_key = fs::read_to_string(&config.keyfile)?;
            log::debug!("parsing unwrapped key from file");
            age::x25519::Identity::from_str(&unwrapped_key)
                .map_err(|e| anyhow!("failed to parse unwrapped key ({e:?})",))?
        };
        log::debug!("successfully parsed a private key!");

        Ok(RageLib { pubkey, identity })
    }
}

impl Backend for RageLib {
    fn create_keypair<P: AsRef<Path>>(path: P) -> Result<String> {
        let keypair = age::x25519::Identity::generate();

        std::fs::write(path, keypair.to_string().expose_secret())?;

        Ok(keypair.to_public().to_string())
    }

    fn create_wrapped_keypair<P: AsRef<Path>>(path: P, password: SecretString) -> Result<String> {
        let keypair = age::x25519::Identity::generate();
        let wrapped_key = Self::wrap_key(keypair.to_string(), password)?;
        std::fs::write(path, wrapped_key)?;

        Ok(keypair.to_public().to_string())
    }

    fn unwrap_keyfile<P: AsRef<Path>>(keyfile: P, password: SecretString) -> Result<SecretString> {
        let wrapped_key = util::read_guarded(&keyfile, MAX_WRAPPED_KEY_FILESIZE)?;

        // NOTE(ww): A work factor of 22 is an educated guess here; rage has generated messages
        // that have needed 17 and 18 before, so this should (hopefully) give us some
        // breathing room.
        let mut identity = age::scrypt::Identity::new(password);
        identity.set_max_work_factor(22);

        // Create a new decryptor for the wrapped key.
        let decryptor = Decryptor::new(ArmoredReader::new(wrapped_key.as_slice()))
            .map_err(|e| anyhow!("unable to load private key (backend reports: {:?})", e))?;

        if !decryptor.is_scrypt() {
            return Err(anyhow!(
                "key unwrap failed: not a password-wrapped keyfile?"
            ));
        }

        // ...and decrypt (i.e., unwrap) using the master password.
        log::debug!("beginning key unwrap...");
        let mut unwrapped_key = String::new();

        decryptor
            .decrypt([&identity as &dyn age::Identity].into_iter())
            .map_err(|e| anyhow!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut unwrapped_key)
                    .map_err(|_| anyhow!("i/o error while decrypting"))
            })?;
        log::debug!("finished key unwrap!");

        Ok(SecretString::from(unwrapped_key))
    }

    fn wrap_key(key: SecretString, password: SecretString) -> Result<Vec<u8>> {
        let encryptor = age::Encryptor::with_user_passphrase(password);

        let mut wrapped_key = vec![];
        let mut writer = encryptor.wrap_output(ArmoredWriter::wrap_output(
            &mut wrapped_key,
            Format::AsciiArmor,
        )?)?;
        writer.write_all(key.expose_secret().as_bytes())?;
        writer.finish().and_then(|armor| armor.finish())?;

        Ok(wrapped_key)
    }

    fn rewrap_keyfile<P: AsRef<Path>>(
        keyfile: P,
        old: SecretString,
        new: SecretString,
    ) -> Result<()> {
        let unwrapped_key = Self::unwrap_keyfile(&keyfile, old)?;
        let rewrapped_key = Self::wrap_key(unwrapped_key, new)?;

        std::fs::write(&keyfile, rewrapped_key)?;
        Ok(())
    }

    fn encrypt(&self, record: &Record) -> Result<String> {
        #[allow(clippy::unwrap_used)]
        let encryptor =
            age::Encryptor::with_recipients([&self.pubkey as &dyn age::Recipient].into_iter())
                .unwrap();
        let mut encrypted = vec![];
        let mut writer = encryptor
            .wrap_output(ArmoredWriter::wrap_output(
                &mut encrypted,
                Format::AsciiArmor,
            )?)
            .map_err(|e| anyhow!("wrap_output failed (backend report: {:?})", e))?;
        writer.write_all(serde_json::to_string(record)?.as_bytes())?;
        writer.finish().and_then(|armor| armor.finish())?;

        Ok(String::from_utf8(encrypted)?)
    }

    fn decrypt(&self, encrypted: &str) -> Result<Record> {
        let decryptor = age::Decryptor::new(ArmoredReader::new(encrypted.as_bytes()))
            .map_err(|e| anyhow!("unable to load private key (backend reports: {:?})", e))?;

        let mut decrypted = String::new();

        decryptor
            .decrypt([&self.identity as &dyn age::Identity].into_iter())
            .map_err(|e| anyhow!("unable to decrypt (backend reports: {:?})", e))
            .and_then(|mut r| {
                r.read_to_string(&mut decrypted)
                    .map_err(|e| anyhow!("i/o error while decrypting: {:?}", e))
            })?;

        Ok(serde_json::from_str(&decrypted)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kbs2::record::{LoginFields, RecordBody};

    fn dummy_login() -> Record {
        Record::new(
            "dummy",
            RecordBody::Login(LoginFields {
                username: "foobar".into(),
                password: "bazqux".into(),
            }),
        )
    }

    fn ragelib_backend() -> RageLib {
        let key = age::x25519::Identity::generate();

        RageLib {
            pubkey: key.to_public(),
            identity: key,
        }
    }

    fn ragelib_backend_bad_keypair() -> RageLib {
        let key1 = age::x25519::Identity::generate();
        let key2 = age::x25519::Identity::generate();

        RageLib {
            pubkey: key1.to_public(),
            identity: key2,
        }
    }

    #[test]
    fn test_ragelib_create_keypair() {
        let keyfile = tempfile::NamedTempFile::new().unwrap();

        assert!(RageLib::create_keypair(&keyfile).is_ok());
    }

    #[test]
    fn test_ragelib_create_wrapped_keypair() {
        let keyfile = tempfile::NamedTempFile::new().unwrap();

        // Creating a wrapped keypair with a particular password should succeed.
        assert!(RageLib::create_wrapped_keypair(
            &keyfile,
            SecretString::new("weakpassword".into())
        )
        .is_ok());

        // Unwrapping the keyfile using the same password should succeed.
        assert!(
            RageLib::unwrap_keyfile(&keyfile, SecretString::new("weakpassword".into())).is_ok()
        );
    }

    #[test]
    fn test_ragelib_rewrap_keyfile() {
        let keyfile = tempfile::NamedTempFile::new().unwrap();

        RageLib::create_wrapped_keypair(&keyfile, SecretString::new("weakpassword".into()))
            .unwrap();

        let wrapped_key_a = std::fs::read(&keyfile).unwrap();
        let unwrapped_key_a =
            RageLib::unwrap_keyfile(&keyfile, SecretString::new("weakpassword".into())).unwrap();

        // Changing the password on a wrapped keyfile should succeed.
        assert!(RageLib::rewrap_keyfile(
            &keyfile,
            SecretString::new("weakpassword".into()),
            SecretString::new("stillweak".into()),
        )
        .is_ok());

        let wrapped_key_b = std::fs::read(&keyfile).unwrap();
        let unwrapped_key_b =
            RageLib::unwrap_keyfile(&keyfile, SecretString::new("stillweak".into())).unwrap();

        // The wrapped envelopes should not be equal, since the password has changed.
        assert_ne!(wrapped_key_a, wrapped_key_b);

        // However, the wrapped key itself should be preserved.
        assert_eq!(
            unwrapped_key_a.expose_secret(),
            unwrapped_key_b.expose_secret()
        );
    }

    #[test]
    fn test_ragelib_encrypt() {
        {
            let backend = ragelib_backend();
            let record = dummy_login();
            assert!(backend.encrypt(&record).is_ok());
        }

        // TODO: Test RageLib::encrypt failure modes.
    }

    #[test]
    fn test_ragelib_decrypt() {
        {
            let backend = ragelib_backend();
            let record = dummy_login();

            let encrypted = backend.encrypt(&record).unwrap();
            let decrypted = backend.decrypt(&encrypted).unwrap();

            assert_eq!(record, decrypted);
        }

        {
            let backend = ragelib_backend_bad_keypair();
            let record = dummy_login();

            let encrypted = backend.encrypt(&record).unwrap();
            let err = backend.decrypt(&encrypted).unwrap_err();

            assert_eq!(
                err.to_string(),
                "unable to decrypt (backend reports: NoMatchingKeys)"
            );
        }
    }
}
