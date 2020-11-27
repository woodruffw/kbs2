use anyhow::{anyhow, Result};
use pinentry::PassphraseInput;
use secrecy::SecretString;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Given an input string formatted according to shell quoting rules,
/// split it into its command and argument parts and return each.
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

/// Given a command and its arguments, run the command and capture the resulting
/// standard output.
///
/// NOTE: The command is run with no standard input or standard error.
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

/// Securely retrieve a password from the user.
///
/// NOTE: This function currently uses pinentry internally, which
/// will delegate to the appropriate pinentry binary on the user's
/// system.
pub fn get_password() -> Result<SecretString> {
    if let Some(mut input) = PassphraseInput::with_default_binary() {
        input
            .with_description("Enter your master kbs2 password")
            .with_prompt("Password:")
            .interact()
            .map_err(|e| anyhow!("pinentry failed: {}", e.to_string()))
    } else {
        log::debug!("no pinentry binary, falling back on rpassword");

        rpassword::read_password_from_tty(Some("Password: "))
            .map(SecretString::new)
            .map_err(|e| anyhow!("password prompt failed: {}", e.to_string()))
    }
}

/// Return the current timestamp as seconds since the UNIX epoch.
pub fn current_timestamp() -> u64 {
    // NOTE(ww): This unwrap should be safe, since every time should be
    // greater than or equal to the epoch.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Print the given message on `stderr` with a warning prefix.
pub fn warn(msg: &str) {
    eprintln!("Warn: {}", msg);
}

/// Retrieve the current user's home directory.
pub fn home_dir() -> Result<PathBuf> {
    match home::home_dir() {
        Some(dir) => Ok(dir),
        None => Err(anyhow!("couldn't find the user's home directory")),
    }
}

/// Read the entire given file into a `Vec<u8>`, or fail if its on-disk size exceeds
/// some limit.
pub fn read_guarded<P: AsRef<Path>>(path: P, limit: u64) -> Result<Vec<u8>> {
    let mut file = File::open(&path)?;
    let meta = file.metadata()?;
    if meta.len() > limit {
        return Err(anyhow!("requested file is suspiciously large, refusing"));
    }

    let mut buf = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut buf)?;

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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

    #[test]
    fn test_read_guarded() {
        {
            let mut small = NamedTempFile::new().unwrap();
            small.write(b"test").unwrap();
            small.flush().unwrap();

            let contents = read_guarded(small.path(), 1024);
            assert!(contents.is_ok());
            assert_eq!(contents.unwrap().as_slice(), b"test");
        }

        {
            let mut toobig = NamedTempFile::new().unwrap();
            toobig.write(b"slightlytoobig").unwrap();
            toobig.flush().unwrap();

            assert!(read_guarded(toobig.path(), 10).is_err());
        }
    }
}
