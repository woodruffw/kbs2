use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_with_status(command: &str, args: &[&str]) -> Option<bool> {
    Command::new(command)
        .args(args)
        .status()
        .map_or(None, |s| Some(s.success()))
}

pub fn current_timestamp() -> u64 {
    // NOTE(ww): This unwrap should be safe, since every time should be
    // greater than or equal to the epoch.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
