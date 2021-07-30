use std::io::{self, Read};

use anyhow::{anyhow, Result};
use dialoguer::{Input, Password};

use crate::kbs2::config::Config;
use crate::kbs2::generator::Generator;
use crate::kbs2::record::FieldKind::{self, *};

/// The input separator used when input is gathered in "terse" mode.
pub static TERSE_IFS: &str = "\x01";

/// Given an array of field names and a potential generator, grabs the values for
/// those fields in a terse manner (each separated by `TERSE_IFS`).
///
/// Fields that are marked as sensitive are subsequently overwritten by the
/// generator, if one is provided.
fn terse_fields(names: &[FieldKind], generator: Option<&dyn Generator>) -> Result<Vec<String>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    if input.ends_with('\n') {
        input.pop();
    }

    // NOTE(ww): Handling generated inputs in terse mode is a bit of a mess.
    // First, we collect all inputs, expecting blank slots where we'll fill
    // in the generated values.
    let mut fields = input
        .split(TERSE_IFS)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    if fields.len() != names.len() {
        return Err(anyhow!(
            "field count mismatch: expected {}, found {}",
            names.len(),
            fields.len()
        ));
    }

    // Then, if we have a generator configured, we iterate over the
    // fields and insert them as appropriate.
    if let Some(generator) = generator {
        for (i, name) in names.iter().enumerate() {
            if let Sensitive(_) = name {
                let field = fields.get_mut(i).unwrap();
                field.clear();
                field.push_str(&generator.secret()?);
            }
        }
    }

    Ok(fields)
}

/// Given an array of field names and a potential generator, grabs the values for those
/// fields by prompting the user for each.
///
/// If a field is marked as sensitive **and** a generator is provided, the generator
/// is used to provide that field and the user is **not** prompted.
fn interactive_fields(
    names: &[FieldKind],
    config: &Config,
    generator: Option<&dyn Generator>,
) -> Result<Vec<String>> {
    let mut fields = vec![];

    for name in names {
        let field = match name {
            Sensitive(name) => {
                if let Some(generator) = generator {
                    generator.secret()?
                } else {
                    let field = Password::new()
                        .with_prompt(*name)
                        .allow_empty_password(config.commands.new.generate_on_empty)
                        .interact()?;

                    if field.is_empty() && config.commands.new.generate_on_empty {
                        log::debug!("generate-on-empty with an empty field, generating a secret");

                        let generator = config.get_generator("default").ok_or_else(|| {
                            anyhow!("generate-on-empty configured but no default generator")
                        })?;

                        generator.secret()?
                    } else {
                        field
                    }
                }
            }
            Insensitive(name) => Input::<String>::new().with_prompt(*name).interact()?,
        };

        fields.push(field);
    }

    Ok(fields)
}

/// Grabs the values for a set of field names from user input.
///
/// # Arguments
///
/// * `names` - the set of field names to grab
/// * `terse` - whether or not to get fields tersely, i.e. by splitting on
///   `TERSE_IFS` instead of prompting for each
/// * `config` - the active `Config`
/// * `generator` - the generator, if any, to use for sensitive fields
pub fn fields(
    names: &[FieldKind],
    terse: bool,
    config: &Config,
    generator: Option<&dyn Generator>,
) -> Result<Vec<String>> {
    if terse {
        terse_fields(names, generator)
    } else {
        interactive_fields(names, config, generator)
    }
}
