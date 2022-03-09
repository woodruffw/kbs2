// NOTE(ww): Dead code allowed because of this `cargo test` bug:
// https://github.com/rust-lang/rust/issues/46379
#![allow(dead_code)]

use assert_cmd::Command;
use tempfile::TempDir;

#[derive(Debug)]
pub struct CliSession {
    pub config_dir: TempDir,
    pub store_dir: TempDir,
}

impl CliSession {
    pub fn new() -> Self {
        let config_dir = TempDir::new().unwrap();
        let store_dir = TempDir::new().unwrap();

        // Run `kbs2 init` to configure the config and session directories.
        {
            kbs2()
                .arg("--config-dir")
                .arg(config_dir.path())
                .arg("init")
                .arg("--insecure-not-wrapped")
                .arg("--store-dir")
                .arg(store_dir.path())
                .assert()
                .success();
        }

        Self {
            config_dir: config_dir,
            store_dir: store_dir,
        }
    }

    pub fn command(&self) -> Command {
        let mut kbs2 = kbs2();

        kbs2.arg("--config-dir").arg(self.config_dir.path());

        kbs2
    }
}

pub fn kbs2() -> Command {
    Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}
