use dialoguer::{Input, Password};

use std::io::{self, Read};

use crate::kbs2::error::Error;
use crate::kbs2::record::FieldKind::{self, *};

// TODO(ww): Make this configurable.
pub static TERSE_IFS: &'static str = "\x01";

fn terse_fields(names: &[FieldKind]) -> Result<Vec<String>, Error> {
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

fn interactive_fields(names: &[FieldKind]) -> Result<Vec<String>, Error> {
    let mut fields = vec![];

    for name in names {
        let field = match name {
            Sensitive(name) => Password::new().with_prompt(*name).interact()?,
            Insensitive(name) => Input::<String>::new().with_prompt(*name).interact()?,
        };

        fields.push(field);
    }

    Ok(fields)
}

pub fn fields(names: &[FieldKind], terse: bool) -> Result<Vec<String>, Error> {
    if terse {
        terse_fields(names)
    } else {
        interactive_fields(names)
    }
}
