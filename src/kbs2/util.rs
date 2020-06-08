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
        .output()
        .map_err(|_| anyhow!("failed to execute command: {}", command))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_split_args() {
        {
            let (cmd, args) = parse_and_split_args("just-a-command").unwrap();
            assert_eq!(cmd, "just-a-command");
            assert_eq!(args, Vec::<String>::new());
        }

        {
            let (cmd, args) =
                parse_and_split_args("foo -a -ab --c -d=e --f=g bar baz quux").unwrap();
            assert_eq!(cmd, "foo");
            assert_eq!(
                args,
                vec!["-a", "-ab", "--c", "-d=e", "--f=g", "bar", "baz", "quux"]
            );
        }

        {
            let (cmd, args) = parse_and_split_args("foo 'one arg' \"another arg\" ''").unwrap();

            assert_eq!(cmd, "foo");
            assert_eq!(args, vec!["one arg", "another arg", ""]);
        }

        {
            let err = parse_and_split_args("some 'bad {syntax").unwrap_err();
            assert_eq!(
                err.to_string(),
                "failed to split command-line arguments: some 'bad {syntax"
            );
        }

        {
            let err = parse_and_split_args("").unwrap_err();
            assert_eq!(err.to_string(), "missing one or more arguments in command");
        }
    }

    #[test]
    fn test_run_with_output() {
        {
            let output = run_with_output("echo", &["-n", "foo"]).unwrap();
            assert_eq!(output, "foo");
        }

        {
            let output = run_with_output("echo", &["foo"]).unwrap();
            assert_eq!(output, "foo");
        }

        {
            let err = run_with_output("this-command-should-not-exist", &[]).unwrap_err();
            assert_eq!(
                err.to_string(),
                "failed to execute command: this-command-should-not-exist"
            );
        }

        {
            let err = run_with_output("true", &[]).unwrap_err();
            assert_eq!(err.to_string(), "expected output from true, but none given");
        }

        // TODO: Small error test here for the case where the output isn't UTF-8.
    }

    // TODO: Figure out a good way to test util::get_password.

    #[test]
    fn test_current_timestamp() {
        {
            let ts = current_timestamp();
            assert!(ts != 0);
        }

        {
            let ts1 = current_timestamp();
            let ts2 = current_timestamp();

            assert!(ts2 >= ts1);
        }
    }

    // TODO: Figure out a good way to test util::warn.

    #[test]
    fn test_home_dir() {
        let dir = home_dir().unwrap();

        assert!(dir.exists());
        assert!(dir.is_dir());
    }
}
