use anyhow::{anyhow, Result};

use std::fs;
use std::io;
use std::path::Path;

use crate::kbs2::backend::{self, BackendKind};
use crate::kbs2::config;
use crate::kbs2::record;

pub struct Session {
    pub backend: Box<dyn backend::Backend>,
    pub config: config::Config,
}

impl Session {
    pub fn new(config: config::Config) -> Result<Session> {
        log::debug!("backend: {:?}", config.age_backend);

        fs::create_dir_all(&config.store)?;

        let backend: Box<dyn backend::Backend> = match config.age_backend {
            BackendKind::RageLib => Box::new(backend::RageLib::new(&config)?),
            BackendKind::RageCLI => Box::new(backend::RageCLI::new(&config)?),
            BackendKind::AgeCLI => Box::new(backend::AgeCLI::new(&config)?),
        };

        Ok(Session { backend, config })
    }

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

    pub fn has_record(&self, label: &str) -> bool {
        let record_path = Path::new(&self.config.store).join(label);

        record_path.is_file()
    }

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

    pub fn add_record(&self, record: &record::Record) -> anyhow::Result<()> {
        let record_path = Path::new(&self.config.store).join(&record.label);

        let record_contents = self.backend.encrypt(record)?;
        std::fs::write(&record_path, &record_contents)?;

        Ok(())
    }

    pub fn delete_record(&self, label: &str) -> Result<()> {
        let record_path = Path::new(&self.config.store).join(label);

        std::fs::remove_file(&record_path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => anyhow!("no such record: {}", label),
            _ => e.into(),
        })
    }
}
