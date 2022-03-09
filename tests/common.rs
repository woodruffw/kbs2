use std::ffi::OsStr;

use assert_cmd::{assert::Assert, Command};
use delegate::delegate;
use tempfile::TempDir;

#[derive(Debug)]
pub struct CliSession {
    command: Command,
    pub config_dir: Option<TempDir>,
    pub store_dir: Option<TempDir>,
}

impl CliSession {
    fn blank() -> Self {
        Self {
            command: Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap(),
            config_dir: None,
            store_dir: None,
        }
    }

    pub fn new() -> Self {
        let config_dir = TempDir::new().unwrap();
        let store_dir = TempDir::new().unwrap();
        let mut command = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();

        // Run `kbs2 init` to configure the config and session directories.
        {
            Self::blank()
                .arg("--config-dir")
                .arg(config_dir.path())
                .arg("init")
                .arg("--insecure-not-wrapped")
                .arg("--store-dir")
                .arg(store_dir.path())
                .assert()
                .success();
        }

        command.arg("--config-dir").arg(config_dir.path());

        Self {
            command,
            config_dir: Some(config_dir),
            store_dir: Some(store_dir),
        }
    }

    delegate! {
        to self.command {
            pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command;

            pub fn args<I, S>(&mut self, args: I) -> &mut Command
            where
                I: IntoIterator<Item = S>,
                S: AsRef<OsStr>;

            pub fn assert(&mut self) -> Assert;
        }
    }
}

pub fn kbs2() -> CliSession {
    CliSession::blank()
}
