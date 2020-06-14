use anyhow::{anyhow, Result};
use rand::Rng;

use crate::kbs2::config;
use crate::kbs2::util;

/// Represents the operations that all generators are capable of.
pub trait Generator {
    /// Returns the name of the generator, e.g. `"default"`.
    fn name(&self) -> &str;

    /// Returns a secret produced by the generator.
    fn secret(&self) -> Result<String>;
}

impl Generator for config::GeneratorCommandConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn secret(&self) -> Result<String> {
        let (command, args) = util::parse_and_split_args(&self.command)?;
        let args = args.iter().map(AsRef::as_ref).collect::<Vec<&str>>();

        util::run_with_output(&command, &args)
    }
}

impl Generator for config::GeneratorInternalConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn secret(&self) -> Result<String> {
        // NOTE(ww): Disallow non-ASCII, to prevent gibberish indexing below.
        if !self.alphabet.is_ascii() {
            return Err(anyhow!("generator alphabet contains non-ascii characters"));
        }

        let mut rng = rand::thread_rng();
        let alphabet = self.alphabet.as_bytes();
        let secret = (0..self.length)
            .map(|_| alphabet[rng.gen_range(0, alphabet.len())] as char)
            .collect::<String>();

        Ok(secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_command_generator(command: &str) -> Box<dyn Generator> {
        Box::new(config::GeneratorCommandConfig {
            name: "dummy-command".into(),
            command: command.into(),
        })
    }

    fn dummy_internal_generator(alphabet: &str) -> Box<dyn Generator> {
        Box::new(config::GeneratorInternalConfig {
            name: "dummy-internal".into(),
            alphabet: alphabet.into(),
            length: 5,
        })
    }

    #[test]
    fn test_name() {
        {
            let gen = dummy_command_generator("true");
            assert_eq!(gen.name(), "dummy-command");
        }

        {
            let gen = dummy_internal_generator("abc");
            assert_eq!(gen.name(), "dummy-internal");
        }
    }

    #[test]
    fn test_secret() {
        {
            let gen = dummy_command_generator("echo fake-password");
            assert_eq!(gen.secret().unwrap(), "fake-password");
        }

        {
            let gen = dummy_internal_generator("abc");
            assert_eq!(gen.secret().unwrap().len(), 5);
        }

        {
            let gen = dummy_command_generator("false");
            let err = gen.secret().unwrap_err();
            assert_eq!(
                err.to_string(),
                "expected output from false, but none given"
            );
        }

        {
            let gen = dummy_internal_generator("ⓓⓔⓕⓘⓝⓘⓣⓔⓛⓨ ⓝⓞⓣ ⓐⓢⓒⓘⓘ");
            let err = gen.secret().unwrap_err();
            assert_eq!(
                err.to_string(),
                "generator alphabet contains non-ascii characters"
            );
        }
    }
}
