use anyhow::{anyhow, Result};

use std::fs;
use std::io;
use std::path::Path;

use crate::kbs2::backend;
use crate::kbs2::config;
use crate::kbs2::record;

/// Encapsulates the context needed by `kbs2` to interact with records.
pub struct Session {
    /// The age backend used to encrypt and decrypt records.
    pub backend: Box<dyn backend::Backend>,

    /// The configuration that `kbs2` was invoked with.
    pub config: config::Config,
}

impl Session {
    /// Creates a new session, given a `Config`.
    pub fn new(config: config::Config) -> Result<Session> {
        fs::create_dir_all(&config.store)?;

        let backend: Box<dyn backend::Backend> = Box::new(backend::RageLib::new(&config)?);

        Ok(Session { backend, config })
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
            let label = path.file_name().unwrap();

            // NOTE(ww): This one isn't safe, but we don't care. Non-UTF-8 labels aren't supported.
            labels.push(label.to_str().unwrap().into());
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};

    // NOTE: We pass store in here instead of creating it for lifetime reasons:
    // the temp dir is destroyed when its TempDir object is destructed, so we need
    // to keep it alive long enough for each unit test.
    fn dummy_session(store: &TempDir) -> Session {
        let backend = {
            let key = age::SecretKey::generate();

            Box::new(backend::RageLib {
                pubkey: key.to_public(),
                identities: vec![key.into()],
            })
        };

        let config = {
            config::Config {
                // NOTE: We create the backend above manually, so the public_key and keyfile
                // here are dummy values that shouldn't need to be interacted with.
                public_key: "not a real public key".into(),
                keyfile: "not a real private key file".into(),
                wrapped: false,
                store: store.path().to_str().unwrap().into(),
                pre_hook: None,
                post_hook: None,
                reentrant_hooks: false,
                generators: vec![config::GeneratorConfig::Internal(Default::default())],
                commands: Default::default(),
            }
        };

        Session { backend, config }
    }

    // TODO: Figure out how to test Session::new. Doing so will require an interface for
    // creating + initializing a config that doesn't unconditionally put the store directory
    // within the user's data directory.

    #[test]
    fn test_record_labels() {
        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

            assert_eq!(session.record_labels().unwrap(), Vec::<String>::new());
        }

        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();
            assert_eq!(session.record_labels().unwrap(), vec!["foo"]);
        }
    }

    #[test]
    fn test_has_record() {
        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();
            assert!(session.has_record("foo"));
        }

        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

            assert!(!session.has_record("does-not-exist"));
        }
    }

    #[test]
    fn test_get_record() {
        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();

            let retrieved_record = session.get_record("foo").unwrap();

            assert_eq!(record, retrieved_record);
        }

        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

            let err = session.get_record("foo").unwrap_err();
            assert_eq!(err.to_string(), "no such record: foo");
        }
    }

    #[test]
    fn test_add_record() {
        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

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
            let session = dummy_session(&store);
            let record = record::Record::login("foo", "bar", "baz");

            session.add_record(&record).unwrap();

            assert!(session.delete_record("foo").is_ok());
            assert!(!session.has_record("foo"));
            assert_eq!(session.record_labels().unwrap(), Vec::<String>::new());
        }

        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

            let record1 = record::Record::login("foo", "bar", "baz");
            session.add_record(&record1).unwrap();

            let record2 = record::Record::login("a", "b", "c");
            session.add_record(&record2).unwrap();

            assert!(session.delete_record("foo").is_ok());
            assert_eq!(session.record_labels().unwrap(), vec!["a"]);
        }

        {
            let store = tempdir().unwrap();
            let session = dummy_session(&store);

            let err = session.delete_record("does-not-exist").unwrap_err();
            assert_eq!(err.to_string(), "no such record: does-not-exist");
        }
    }
}
