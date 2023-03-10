mod common;

use common::CliSession;

#[test]
fn test_kbs2_rm() {
    let session = CliSession::new();

    // `kbs2 rm` with a nonexistent record fails.
    {
        session
            .command()
            .args(["rm", "does-not-exist"])
            .assert()
            .failure();
    }

    // `kbs2 rm` works as expected with a record that exists.
    {
        session
            .command()
            .args(["new", "-k", "login", "test-record"])
            .write_stdin("fakeuser\x01fakepass")
            .assert()
            .success();

        session
            .command()
            .args(["rm", "test-record"])
            .assert()
            .success();

        session
            .command()
            .args(["dump", "test-record"])
            .assert()
            .failure();
    }

    // `kbs2 rm` works as expected with multiple records.
    {
        session
            .command()
            .args(["new", "-k", "login", "test-record-1"])
            .write_stdin("fakeuser\x01fakepass")
            .assert()
            .success();

        session
            .command()
            .args(["new", "-k", "login", "test-record-2"])
            .write_stdin("fakeuser\x01fakepass")
            .assert()
            .success();

        session
            .command()
            .args(["rm", "test-record-1", "test-record-2"])
            .assert()
            .success();

        session
            .command()
            .args(["dump", "test-record-1", "test-record-2"])
            .assert()
            .failure();
    }
}
