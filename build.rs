use std::env;
use std::process::Command;

fn main() {
    let mut version = String::from(env!("CARGO_PKG_VERSION"));
    if let Some(commit_hash) = commit_hash() {
        version = format!("{} ({})", version, commit_hash);
    }
    println!("cargo:rustc-env=KBS2_BUILD_VERSION={}", version);
}

// Cribbed from Alacritty:
// https://github.com/alacritty/alacritty/blob/8ea6c3b/alacritty/build.rs
fn commit_hash() -> Option<String> {
    Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().into())
}
