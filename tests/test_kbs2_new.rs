mod common;

use common::{CliSession, ToJson};
use serde_json::json;

// TODO: Figure out how to test prompts instead of terse inputs.

#[test]
fn test_kbs2_new_login() {
    let session = CliSession::new();

    session
        .command()
        .args(&["new", "-k", "login", "test-record"])
        .write_stdin("fakeuser\x01fakepass")
        .assert()
        .success();

    let dump = session
        .command()
        .args(&["dump", "--json", "test-record"])
        .output()
        .unwrap()
        .json();

    let fields = dump.get("body").unwrap().get("fields").unwrap();

    assert_eq!(
        fields,
        // https://github.com/serde-rs/json/issues/867
        &json!({ "username": "fakeuser", "password": "fakepass" }),
    );
}

#[test]
fn test_kbs2_new_environment() {
    let session = CliSession::new();

    session
        .command()
        .args(&["new", "-k", "environment", "test-record"])
        .write_stdin("fakevariable\x01fakevalue")
        .assert()
        .success();

    let dump = session
        .command()
        .args(&["dump", "--json", "test-record"])
        .output()
        .unwrap()
        .json();

    let fields = dump.get("body").unwrap().get("fields").unwrap();

    assert_eq!(
        fields,
        // https://github.com/serde-rs/json/issues/867
        &json!({ "variable": "fakevariable", "value": "fakevalue" }),
    );
}

#[test]
fn test_kbs2_new_unstructured() {
    let session = CliSession::new();

    session
        .command()
        .args(&["new", "-k", "unstructured", "test-record"])
        .write_stdin("fakevalue")
        .assert()
        .success();

    let dump = session
        .command()
        .args(&["dump", "--json", "test-record"])
        .output()
        .unwrap()
        .json();

    let fields = dump.get("body").unwrap().get("fields").unwrap();

    assert_eq!(
        fields,
        // https://github.com/serde-rs/json/issues/867
        &json!({ "contents": "fakevalue" }),
    );
}
