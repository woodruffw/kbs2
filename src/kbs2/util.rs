use std::process::Command;

pub fn run_with_status(command: &str, args: &[&str]) -> Option<bool> {
    Command::new(command)
        .args(args)
        .status()
        .map_or(None, |s| Some(s.success()))
}
