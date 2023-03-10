mod common;

use common::CliSession;

#[test]
fn test_kbs2_rename() {
    let session = CliSession::new();

    // `rename` deletes the old record.
    session
        .command()
        .args(["new", "-k", "login", "test-record"])
        .write_stdin("fakeuser\x01fakepass")
        .assert()
        .success();

    session
        .command()
        .args(["rename", "test-record", "test-record-1"])
        .assert()
        .success();

    session
        .command()
        .args(["dump", "test-record"])
        .assert()
        .failure();

    session
        .command()
        .args(["dump", "test-record-1"])
        .assert()
        .success();
}

// TODO: `kbs2 rename --force`
// TODO: `kbs2 rename` with the same record twice
