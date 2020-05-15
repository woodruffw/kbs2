use dialoguer::{Input, Password};

use std::io::{self, Read};

use crate::kbs2::error::Error;

// TODO(ww): Make this configurable.
pub static TERSE_IFS: &'static str = "\x01";

fn terse_fields(names: &[(&str, bool)]) -> Result<Vec<String>, Error> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    if input.ends_with('\n') {
        input.pop();
    }

    let fields = input.split(TERSE_IFS).collect::<Vec<&str>>();
    if fields.len() == names.len() {
        Ok(fields.iter().map(|f| f.to_string()).collect())
    } else {
        Err(format!(
            "field count mismatch: expected {}, found {}",
            names.len(),
            fields.len()
        )
        .as_str()
        .into())
    }
}

fn interactive_fields(names: &[(&str, bool)]) -> Result<Vec<String>, Error> {
    let mut fields = vec![];

    for (name, sensitive) in names {
        let field = match sensitive {
            true => Password::new().with_prompt(*name).interact()?,
            false => Input::<String>::new().with_prompt(*name).interact()?,
        };

        fields.push(field);
    }

    Ok(fields)
}

pub fn fields(names: &[(&str, bool)], terse: bool) -> Result<Vec<String>, Error> {
    if terse {
        terse_fields(names)
    } else {
        interactive_fields(names)
    }
}
