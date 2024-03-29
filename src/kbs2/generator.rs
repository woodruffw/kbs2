use anyhow::{anyhow, Result};
use rand::seq::{IteratorRandom, SliceRandom};

use crate::kbs2::config;

/// Represents the operations that all generators are capable of.
pub trait Generator {
    /// Returns the name of the generator, e.g. `"default"`.
    fn name(&self) -> &str;

    /// Returns a secret produced by the generator.
    fn secret(&self) -> Result<String>;
}

impl Generator for config::GeneratorConfig {
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
        let mut secret = Vec::with_capacity(self.length);
        for alphabet in self.alphabets.iter() {
            if alphabet.is_empty() {
                return Err(anyhow!("generator alphabet(s) must not be empty"));
            }

            // NOTE(ww): Disallow non-ASCII, to prevent gibberish indexing below.
            if !alphabet.is_ascii() {
                return Err(anyhow!(
                    "generator alphabet(s) contain non-ascii characters"
                ));
            }

            // Safe unwrap: alphabet.chars() is always nonempty.
            #[allow(clippy::unwrap_used)]
            secret.push(alphabet.chars().choose(&mut rng).unwrap());
        }

        // If step 1 generated a longer password than "length" allows, fail.
        if secret.len() >= self.length {
            return Err(anyhow!(
                "generator invariant failure (too many separate alphabets for length?)"
            ));
        }

        // Pad out with the combined alphabet.
        let combined_alphabet = self.alphabets.iter().flat_map(|a| a.chars());
        let remainder = combined_alphabet.choose_multiple(&mut rng, self.length - secret.len());
        secret.extend(remainder);

        // Shuffle and return.
        secret.shuffle(&mut rng);
        Ok(secret.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_internal_generator(alphabets: &[&str]) -> Box<dyn Generator> {
        Box::new(config::GeneratorConfig {
            name: "dummy-internal".into(),
            alphabets: alphabets.iter().map(|a| (*a).into()).collect(),
            length: 5,
        })
    }

    #[test]
    fn test_internal_generator_invariants() {
        // Fails with no alphabets.
        {
            let gen = config::GeneratorConfig {
                name: "dummy-internal".into(),
                alphabets: vec![],
                length: 10,
            };

            assert_eq!(
                gen.secret().unwrap_err().to_string(),
                "generator must have at least one alphabet"
            );
        }

        // Fails with a length of 0.
        {
            let gen = config::GeneratorConfig {
                name: "dummy-internal".into(),
                alphabets: vec!["abcd".into()],
                length: 0,
            };

            assert_eq!(
                gen.secret().unwrap_err().to_string(),
                "generator length is invalid (must be nonzero)"
            );
        }

        // Fails if an alphabet is non-ASCII.
        {
            let gen = dummy_internal_generator(&["ⓓⓔⓕⓘⓝⓘⓣⓔⓛⓨ ⓝⓞⓣ ⓐⓢⓒⓘⓘ"]);
            let err = gen.secret().unwrap_err();
            assert_eq!(
                err.to_string(),
                "generator alphabet(s) contain non-ascii characters"
            );
        }

        // Fails if any individual alphabet is empty.
        {
            let gen = dummy_internal_generator(&[""]);
            let err = gen.secret().unwrap_err();
            assert_eq!(err.to_string(), "generator alphabet(s) must not be empty");
        }

        // Fails if there are more alphabets than available length.
        {
            let gen = config::GeneratorConfig {
                name: "dummy-internal".into(),
                alphabets: vec!["abc", "def", "ghi"]
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                length: 2,
            };

            assert_eq!(
                gen.secret().unwrap_err().to_string(),
                "generator invariant failure (too many separate alphabets for length?)"
            );
        }

        // Succeeds and upholds length and inclusion invariants.
        {
            let alphabets = ["abcd", "1234", "!@#$"];

            let gen = config::GeneratorConfig {
                name: "dummy-internal".into(),
                alphabets: alphabets.into_iter().map(Into::into).collect(),
                length: 10,
            };

            for secret in (0..100).map(|_| gen.secret()) {
                assert!(secret.is_ok());

                let secret = secret.unwrap();
                assert_eq!(secret.len(), 10);
                assert!(alphabets
                    .iter()
                    .all(|a| a.chars().any(|c| secret.contains(c))));
            }
        }
    }
}
