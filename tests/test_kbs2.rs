mod common;

use clap::ValueEnum;
use clap_complete::Shell;
use common::kbs2;

#[test]
fn test_kbs2_help() {
    // `help`, `--help`, and `-h` all produce the same output

    let reference_output = kbs2().arg("help").output().unwrap();
    assert!(reference_output.status.success());

    for help in &["--help", "-h"] {
        let output = kbs2().arg(help).output().unwrap();
        assert!(output.status.success());
        assert_eq!(reference_output.stdout, output.stdout);
    }
}

#[test]
fn test_kbs2_completions() {
    // Tab completion generation works

    for shell in Shell::value_variants() {
        let output = kbs2()
            .args(["--completions", &shell.to_string()])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn test_kbs2_version() {
    // kbs2 --version works and outputs a string starting with `kbs2 X.Y.Z`

    let version = format!("kbs2 {}", env!("CARGO_PKG_VERSION"));

    let output = kbs2().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with(&version));
}
