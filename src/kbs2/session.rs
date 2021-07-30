use std::convert::TryFrom;
use std::fs;
use std::io;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::kbs2::agent::Agent;
use crate::kbs2::backend::{Backend, RageLib};
use crate::kbs2::config;
use crate::kbs2::record;

/// Encapsulates the context needed by `kbs2` to interact with records.
pub struct Session<'a> {
    /// The `RageLib` backend used to encrypt and decrypt records.
    pub backend: RageLib,

    /// The configuration that `kbs2` was invoked with.
    pub config: &'a config::Config,
}

impl<'a> Session<'a> {
    /// Creates a new session, given a `Config`.
    fn new(config: &'a config::Config) -> Result<Session> {
        // NOTE(ww): I don't like that we do this here, but I'm not sure where else to put it.
        if config.wrapped && config.agent_autostart {
            Agent::spawn()?;
        }

        fs::create_dir_all(&config.store)?;

        #[allow(clippy::redundant_field_names)]
        Ok(Session {
            backend: RageLib::new(config)?,
            config: config,
        })
    }

    /// Returns the label of every record available in the store.
    pub fn record_labels(&self) -> Result<Vec<String>> {
        let store = Path::new(&self.config.store);

        if !store.is_dir() {
            return Err(anyhow!("secret store is not a directory"));
        }

        let mut labels = vec![];
        for entry in fs::read_dir(store)? {
            let path = entry?.path();
            if !path.is_file() {
                log::debug!("skipping non-file in store: {:?}", path);
                continue;
            }

            // NOTE(ww): This unwrap is safe, since file_name always returns Some
            // for non-directories.
            #[allow(clippy::expect_used)]
            let label = path
                .file_name()
                .expect("impossible: is_file=true for path but file_name=None");

            // NOTE(ww): This one isn't safe, but we don't care. Non-UTF-8 labels aren't supported.
            labels.push(
                label
                    .to_str()
                    .ok_or_else(|| anyhow!("unrepresentable record label: {:?}", label))?
                    .into(),
            );
        }

        Ok(labels)
    }

    /// Returns whether or not the store contains a given record.
    pub fn has_record(&self, label: &str) -> bool {
        let record_path = Path::new(&self.config.store).join(label);

        record_path.is_file()
    }

    /// Retrieves a record from the store by its label.
    pub fn get_record(&self, label: &str) -> Result<record::Record> {
        if !self.has_record(label) {
            return Err(anyhow!("no such record: {}", label));
        }

        let record_path = Path::new(&self.config.store).join(label);
        let record_contents = fs::read_to_string(&record_path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => anyhow!("no such record: {}", label),
            _ => e.into(),
        })?;

        match self.backend.decrypt(&record_contents) {
            Ok(record) => Ok(record),
            Err(e) => Err(e),
        }
    }

    /// Adds the given record to the store.
    pub fn add_record(&self, record: &record::Record) -> anyhow::Result<()> {
        let record_path = Path::new(&self.config.store).join(&record.label);

        let record_contents = self.backend.encrypt(record)?;
        std::fs::write(&record_path, &record_contents)?;

        Ok(())
    }

    /// Deletes a record from the store by label.
    pub fn delete_record(&self, label: &str) -> Result<()> {
        let record_path = Path::new(&self.config.store).join(label);

        std::fs::remove_file(&record_path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => anyhow!("no such record: {}", label),
            _ => e.into(),
        })
    }
}

impl<'a> TryFrom<&'a config::Config> for Session<'a> {
    type Error = anyhow::Error;

    fn try_from(config: &'a config::Config) -> Result<Self> {
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::{tempdir, TempDir};

    use super::*;

    // NOTE: We pass store in here instead of creating it for lifetime reasons:
    // the temp dir is unlinked when its TempDir object is destructed, so we need
    // to keep it alive long enough for each unit test.
    fn dummy_config(store: &TempDir) -> config::Config {
        config::Config {
            config_dir: "/not/a/real/dir".into(),
            // NOTE: We create the backend above manually, so the public_key and keyfile
            // here are dummy values that shouldn't need to be interacted with.
            public_key: "not a real public key".into(),
            keyfile: "not a real private key file".into(),
            agent_autostart: false,
            wrapped: false,
            store: store.path().to_str().unwrap().into(),
            pinentry: Default::default(),
            pre_hook: None,
            post_hook: None,
            error_hook: None,
            reentrant_hooks: false,
            generators: vec![config::GeneratorConfig::Internal(Default::default())],
            commands: Default::default(),
        }
    }

    fn dummy_session(config: &config::Config) -> Session {
        let backend = {
            let key = age::x25519::Identity::generate();

            RageLib {
                pubkey: key.to_public(),
                identities: vec![key.into()],
            }
        };

        Session {
            backend,
            config: &config,
        }
    }

    // TODO: Figure out how to test Session::new. Doing so will require an interface for
    // creating + initializing a config that doesn't unconditionally put the store directory
    // within the user's data directory.

    #[test]
    fn test_record_labels() {
        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            assert_eq!(session.record_labels().unwrap(), Vec::<String>::new());
        }

        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();
            assert_eq!(session.record_labels().unwrap(), vec!["foo"]);
        }
    }

    #[test]
    fn test_has_record() {
        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();
            assert!(session.has_record("foo"));
        }

        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            assert!(!session.has_record("does-not-exist"));
        }
    }

    #[test]
    fn test_get_record() {
        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();

            let retrieved_record = session.get_record("foo").unwrap();

            assert_eq!(record, retrieved_record);
        }

        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            let err = session.get_record("foo").unwrap_err();
            assert_eq!(err.to_string(), "no such record: foo");
        }
    }

    #[test]
    fn test_add_record() {
        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            let record1 = record::Record::login("foo", "bar", "baz");
            session.add_record(&record1).unwrap();

            let record2 = record::Record::login("a", "b", "c");
            session.add_record(&record2).unwrap();

            // NOTE: record_labels() returns labels in a platform dependent order,
            // which is why we don't compared against a fixed-order vec here or below.
            assert_eq!(session.record_labels().unwrap().len(), 2);
            assert!(session.record_labels().unwrap().contains(&"foo".into()));
            assert!(session.record_labels().unwrap().contains(&"a".into()));

            // Overwrite foo; still only two records.
            let record3 = record::Record::login("foo", "quux", "zap");
            session.add_record(&record3).unwrap();

            assert_eq!(session.record_labels().unwrap().len(), 2);
            assert!(session.record_labels().unwrap().contains(&"foo".into()));
            assert!(session.record_labels().unwrap().contains(&"a".into()));
        }
    }

    #[test]
    fn test_delete_record() {
        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();

            assert!(session.delete_record("foo").is_ok());
            assert!(!session.has_record("foo"));
            assert_eq!(session.record_labels().unwrap(), Vec::<String>::new());
        }

        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            let record1 = record::Record::login("foo", "bar", "baz");
            session.add_record(&record1).unwrap();

            let record2 = record::Record::login("a", "b", "c");
            session.add_record(&record2).unwrap();

            assert!(session.delete_record("foo").is_ok());
            assert_eq!(session.record_labels().unwrap(), vec!["a"]);
        }

        {
            let store = tempdir().unwrap();
            let config = dummy_config(&store);
            let session = dummy_session(&config);

            let err = session.delete_record("does-not-exist").unwrap_err();
            assert_eq!(err.to_string(), "no such record: does-not-exist");
        }
    }
}
