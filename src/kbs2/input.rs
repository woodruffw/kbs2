use anyhow::{anyhow, Result};
use dialoguer::{Input, Password};

use std::io::{self, Read};

use crate::kbs2::generator::Generator;
use crate::kbs2::record::FieldKind::{self, *};

pub static TERSE_IFS: &str = "\x01";

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

fn interactive_fields(
    names: &[FieldKind],
    generator: Option<&dyn Generator>,
) -> Result<Vec<String>> {
    let mut fields = vec![];

    for name in names {
        let field = match name {
            Sensitive(name) => {
                if let Some(generator) = generator {
                    generator.secret()?
                } else {
                    Password::new().with_prompt(*name).interact()?
                }
            }
            Insensitive(name) => Input::<String>::new().with_prompt(*name).interact()?,
        };

        fields.push(field);
    }

    Ok(fields)
}

pub fn fields(
    names: &[FieldKind],
    terse: bool,
    generator: Option<&dyn Generator>,
) -> Result<Vec<String>> {
    if terse {
        terse_fields(names, generator)
    } else {
        interactive_fields(names, generator)
    }
}
