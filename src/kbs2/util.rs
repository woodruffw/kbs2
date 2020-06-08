use anyhow::{anyhow, Result};
use pinentry::PassphraseInput;
use secrecy::SecretString;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn parse_and_split_args(argv: &str) -> Result<(String, Vec<String>)> {
    let args = match shell_words::split(argv) {
        Ok(args) => args,
        Err(_) => return Err(anyhow!("failed to split command-line arguments: {}", argv)),
    };

    let (command, args) = args
        .split_first()
        .map(|t| (t.0.to_owned(), t.1.to_owned()))
        .ok_or_else(|| anyhow!("missing one or more arguments in command"))?;

    Ok((command, args))
}

pub fn run_with_output(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;

    if output.stdout.is_empty() {
        return Err(anyhow!("expected output from {}, but none given", command));
    }

    let mut output = String::from_utf8(output.stdout)?;
    if output.ends_with('\n') {
        output.pop();
    }

    Ok(output)
}

pub fn get_password() -> Result<SecretString> {
    if let Some(mut input) = PassphraseInput::with_default_binary() {
        input
            .with_description("Enter your master kbs2 password")
            .with_prompt("Password:")
            .interact()
            .map_err(|e| anyhow!("pinentry failed: {}", e.to_string()))
    } else {
        Err(anyhow!("Couldn't get pinentry program for password prompt"))
    }
}

pub fn current_timestamp() -> u64 {
    // NOTE(ww): This unwrap should be safe, since every time should be
    // greater than or equal to the epoch.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn warn(msg: &str) {
    eprintln!("Warn: {}", msg);
}

pub fn home_dir() -> Result<PathBuf> {
    match home::home_dir() {
        Some(dir) => Ok(dir),
        None => Err(anyhow!("couldn't find the user's home directory")),
    }
}
