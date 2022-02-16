use anyhow::{anyhow, Result};
use rand::seq::{IteratorRandom, SliceRandom};
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

impl Generator for config::ExternalGeneratorConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn secret(&self) -> Result<String> {
        let (command, args) = util::parse_and_split_args(&self.command)?;
        let args = args.iter().map(AsRef::as_ref).collect::<Vec<&str>>();

        util::run_with_output(&command, &args)
    }
}

impl Generator for config::InternalGeneratorConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn secret(&self) -> Result<String> {
        // Invariants: we need at least one alphabet, and our length has to be nonzero.
        if self.alphabets.is_empty() {
            return Err(anyhow!("generator must have at least one alphabet"));
        }

        if self.length == 0 {
            return Err(anyhow!("generator length is invalid (must be nonzero)"));
        }

        // Our secret generation strategy:
        // 1. Sample each alphabet once
        // 2. Pad the secret out to the remaining length, sampling from all alphabets
        // 3. Shuffle the result

        let mut rng = rand::thread_rng();
        let mut secret = Vec::with_capacity(self.length as usize);
        for alphabet in self.alphabets.iter() {
            if alphabet.is_empty() {
                return Err(anyhow!("generator alphabet(s) must not be empty"));
            }

            // NOTE(ww): Disallow non-ASCII, to prevent gibberish indexing below.
            if alphabet.is_ascii() {
                return Err(anyhow!(
                    "generator alphabet(s) contain non-ascii characters"
                ));
            }

            secret.push(alphabet.chars().choose(&mut rng).unwrap());
        }

        // If step 1 generated a longer password than "length" allows, fail.
        if secret.len() >= self.length {
            return Err(anyhow!(
                "generator invariant failure (too many separate alphabets for length?)"
            ));
        }

        // Pad out with the combined alphabet.
        let combined_alphabet = self.alphabets.iter().map(|a| a.chars()).flatten();
        let remainder = combined_alphabet.choose_multiple(&mut rng, self.length - secret.len());
        secret.extend(remainder.into_iter());

        // Shuffle and return.
        secret.shuffle(&mut rng);
        Ok(secret.into_iter().collect())
    }
}

impl Generator for config::LegacyInternalGeneratorConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn secret(&self) -> Result<String> {
        if self.length == 0 {
            return Err(anyhow!("generator length is invalid (must be nonzero)"));
        }

        // NOTE(ww): Disallow non-ASCII, to prevent gibberish indexing below.
        if !self.alphabet.is_ascii() {
            return Err(anyhow!("generator alphabet contains non-ascii characters"));
        }

        let mut rng = rand::thread_rng();
        let alphabet = self.alphabet.as_bytes();
        let secret = (0..self.length)
            .map(|_| alphabet[rng.gen_range(0..alphabet.len())] as char)
            .collect::<String>();

        Ok(secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_command_generator(command: &str) -> Box<dyn Generator> {
        Box::new(config::ExternalGeneratorConfig {
            name: "dummy-command".into(),
            command: command.into(),
        })
    }

    fn dummy_internal_generator(alphabet: &str) -> Box<dyn Generator> {
        Box::new(config::LegacyInternalGeneratorConfig {
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
