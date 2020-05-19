use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::kbs2::error::Error;

pub fn parse_and_split_args(argv: &str) -> Result<(String, Vec<String>), Error> {
    let args = match shell_words::split(argv) {
        Ok(args) => args,
        Err(_) => return Err(format!("failed to split command-line arguments: {}", argv).into()),
    };

    let (command, args) = args
        .split_first()
        .map(|t| (t.0.to_owned(), t.1.to_owned()))
        .ok_or_else(|| "missing one or more arguments in command")?;

    Ok((command, args))
}

pub fn run_with_status(command: &str, args: &[&str]) -> Option<bool> {
    Command::new(command)
        .args(args)
        .status()
        .map_or(None, |s| Some(s.success()))
}

pub fn run_with_output(command: &str, args: &[&str]) -> Result<String, Error> {
    let output = Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;

    if output.stdout.is_empty() {
        return Err(format!("expected output from {}, but none given", command).into());
    }

    let mut output = String::from_utf8(output.stdout)?;
    if output.ends_with('\n') {
        output.pop();
    }

    Ok(output)
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
