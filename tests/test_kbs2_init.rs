mod common;

use common::CliSession;

#[test]
fn test_kbs2_init() {
    let session = CliSession::new();

    let config_dir = session.config_dir.path();
    let store_dir = session.store_dir.path();

    // Our config dir, etc. all exist; the store dir is empty.
    assert!(config_dir.is_dir());
    assert!(store_dir.is_dir());
    assert!(config_dir.join("config.toml").is_file());
    assert!(store_dir.read_dir().unwrap().next().is_none());
}
