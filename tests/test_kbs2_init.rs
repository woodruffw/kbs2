mod common;

use common::CliSession;

#[test]
fn test_kbs2_init() {
    let mut kbs2 = CliSession::new();
    kbs2.assert();

    let config_dir = kbs2.config_dir.as_ref().unwrap().path();
    let store_dir = kbs2.store_dir.as_ref().unwrap().path();

    // Our config dir, etc. all exist; the store dir is empty.
    assert!(config_dir.is_dir());
    assert!(store_dir.is_dir());
    assert!(config_dir.join("config.toml").is_file());
    assert!(store_dir.read_dir().unwrap().next().is_none());
}
