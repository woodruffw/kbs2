use assert_cmd::Command;

pub fn kbs2() -> Command {
    Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}
