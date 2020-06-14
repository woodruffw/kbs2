use anyhow::{anyhow, Result};
use memmap::Mmap;
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::mman;
use nix::sys::stat::Mode;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::kbs2::config;
use crate::kbs2::record::Record;
use crate::kbs2::util;

/// The kind of age backend to use for cryptographic operations.
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

/// Represents the operations that all age backends are capable of.
pub trait Backend {
    /// Creates an age keypair, saving the private component to the given path.
    ///
    /// NOTE: The private component is written in an ASCII-armored format.
    fn create_keypair(path: &Path) -> Result<String>
    where
        Self: Sized;

    /// Creates a wrapped age keypair, saving the encrypted private component to the
    /// given path.
    ///
    /// NOTE: Like `create_keypair`, this writes an ASCII-armored private component.
    /// It also prompts the user to enter a password for encrypting the generated
    /// private key.
    fn create_wrapped_keypair(path: &Path) -> Result<String>
    where
        Self: Sized;

    /// Encrypts the given record, returning it as an ASCII-armored string.
    fn encrypt(&self, record: &Record) -> Result<String>;

    /// Decrypts the given ASCII-armored string, returning it as a Record.
    fn decrypt(&self, encrypted: &str) -> Result<Record>;
}

/// Represents the operations that an age CLI backend is capable of.
pub trait CLIBackend {
    /// Returns the public component of the generated keypair.
    fn public_key(&self) -> &str;

    /// Returns a path to a file containing the private component of the generated keypair.
    fn keyfile(&self) -> &str;

    /// Returns the name of the age binary, e.g. `age` for the reference implementation
    /// or `rage` for the Rust implementation.
    fn age() -> &'static str;

    /// Returns the name of the age-keygen binary, e.g. `age-keygen` for the reference
    /// implementation or `rage-keygen` for the Rust implementation.
    fn age_keygen() -> &'static str;
}

impl<T> Backend for T
where
    T: CLIBackend,
{
    fn create_keypair(path: &Path) -> Result<String> {
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

    fn create_wrapped_keypair(path: &Path) -> Result<String> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        let private_key = match Command::new(T::age_keygen()).output() {
            Err(e) => return Err(e.into()),
            Ok(output) => String::from_utf8(output.stdout)?,
        };

        let public_key = match private_key
            .lines()
            .find(|l| l.starts_with("# public key: "))
        {
            Some(line) => line
                .trim_start_matches("# public_key: ")
                .trim_end()
                .to_string(),
            None => {
                return Err(anyhow!(
                    "couldn't find a public key in {} output",
                    T::age_keygen()
                ))
            }
        };

        // Wrap the generated private key. Our age CLI backend will handle prompting the user
        // for a master password.
        let mut child = Command::new(T::age())
            .args(&["-a", "-p", "-o"])
            .arg(path)
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow!("couldn't get input for encrypting"))?;
            stdin.write_all(private_key.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(anyhow!("key wrapping failed; password mismatch?"));
        }

        Ok(public_key)
    }

    fn encrypt(&self, record: &Record) -> Result<String> {
        let mut child = Command::new(T::age())
            .arg("-a")
            .arg("-r")
            .arg(&self.public_key())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow!("couldn't get input for encrypting"))?;
            stdin.write_all(serde_json::to_string(record)?.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        log::debug!("output: {:?}", output);

        if !output.status.success() {
            return Err(anyhow!(
                "encryption failed; misformatted key or empty input?"
            ));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn decrypt(&self, encrypted: &str) -> Result<Record> {
        let mut child = Command::new(T::age())
            .arg("-d")
            .arg("-i")
            .arg(&self.keyfile())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow!("couldn't get input for decrypting"))?;
            stdin.write_all(encrypted.as_bytes())?;
        }

        let output = child.wait_with_output()?;

        if !output.status.success() {
            return Err(anyhow!("decryption failed; bad key or corrupted record?"));
        }

        Ok(serde_json::from_str(std::str::from_utf8(&output.stdout)?)?)
    }
}

/// Encapsulates the `age` reference implementation CLI.
pub struct AgeCLI {
    public_key: String,
    keyfile: String,
}

impl AgeCLI {
    pub fn new(config: &config::Config) -> Result<AgeCLI> {
        if config.wrapped {
            Err(anyhow!("the RageCLI backend doesn't support wrapped keys"))
        } else {
            Ok(AgeCLI {
                public_key: config.public_key.clone(),
                keyfile: config.keyfile.clone(),
            })
        }
    }
}

impl CLIBackend for AgeCLI {
    fn public_key(&self) -> &str {
        self.public_key.as_ref()
    }

    fn keyfile(&self) -> &str {
        self.keyfile.as_ref()
    }

    fn age() -> &'static str {
        "age"
    }

    fn age_keygen() -> &'static str {
        "age-keygen"
    }
}

/// Encapsulates the `rage` Rust CLI.
pub struct RageCLI {
    public_key: String,
    keyfile: String,
}

impl RageCLI {
    pub fn new(config: &config::Config) -> Result<RageCLI> {
        if config.wrapped {
            Err(anyhow!("the RageCLI backend doesn't support wrapped keys"))
        } else {
            Ok(RageCLI {
                public_key: config.public_key.clone(),
                keyfile: config.keyfile.clone(),
            })
        }
    }
}

impl CLIBackend for RageCLI {
    fn public_key(&self) -> &str {
        self.public_key.as_ref()
    }

    fn keyfile(&self) -> &str {
        self.keyfile.as_ref()
    }

    fn age() -> &'static str {
        "rage"
    }

    fn age_keygen() -> &'static str {
        "rage-keygen"
    }
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

            // NOTE(ww): It's be nice if we could call open or one of the direct
            // I/O helpers here, but UNWRAPPED_KEY_SHM_NAME isn't a real filename.
            // NOTE(ww): This should always be safe, as we either directly
            // return a fresh fd from shm_open or indirectly return a fresh one
            // via unwrap_keyfile.
            let unwrapped_file = match mman::shm_open(
                config::UNWRAPPED_KEY_SHM_NAME,
                OFlag::O_RDONLY,
                Mode::empty(),
            ) {
                Ok(unwrapped_fd) => unsafe { File::from_raw_fd(unwrapped_fd) },
                Err(nix::Error::Sys(Errno::ENOENT)) => {
                    log::debug!("unwrapped key not available, requesting unwrap");
                    config.unwrap_keyfile()?
                }
                Err(e) => return Err(e.into()),
            };

            // NOTE(ww): And now some more (macOS specific?) stupidity:
            // our unwrapped_key is in a shared memory object, which is page-aligned
            // (i.e., probably aligned on 4K bytes). Accessing it directly
            // via Deref<Target=[u8]> causes the entire page to get parsed as the
            // key since ASCII NUL is valid UTF-8 and subsequently blow up.
            // Rust's File::metadata() calls fstat64 internally which POSIX
            // *says* is supposed to return an accurate size for the shared memory
            // object, but macOS still returns the aligned size.
            // We give up on doing it the right way and just find the index of the
            // first NUL (defaulting to len() for sensible platforms, like Linux).
            let unwrapped_key = unsafe { Mmap::map(&unwrapped_file)? };
            let nul_index = unwrapped_key
                .iter()
                .position(|&x| x == b'\x00')
                .unwrap_or_else(|| unwrapped_key.len());

            let reader = BufReader::new(&unwrapped_key[..nul_index]);
            log::debug!("parsing unwrapped key");
            age::keys::Identity::from_buffer(reader)?
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

    fn create_wrapped_keypair(path: &Path) -> Result<String> {
        let password = util::get_password()?;
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

    // TODO: Conditionally enable tests for AgeCLI and RageCLI.
}
