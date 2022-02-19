use std::io::{self, Read};

use anyhow::{anyhow, Result};
use inquire::{Password as Pass, Text};

use super::record::{EnvironmentFields, LoginFields, RecordBody, UnstructuredFields};
use crate::kbs2::config::RuntimeConfig;

/// The input separator used when input is gathered in "terse" mode.
pub static TERSE_IFS: &str = "\x01";

pub trait Input {
    const FIELD_COUNT: usize;

    fn from_prompt(config: &RuntimeConfig) -> Result<RecordBody>;
    fn from_terse(config: &RuntimeConfig) -> Result<RecordBody>;

    fn take_terse_fields() -> Result<Vec<String>> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;

        if input.ends_with('\n') {
            input.pop();
        }

        let fields = input
            .splitn(Self::FIELD_COUNT, TERSE_IFS)
            .map(Into::into)
            .collect::<Vec<String>>();

        if fields.len() != Self::FIELD_COUNT {
            return Err(anyhow!(
                "field count mismatch: expected {}, got {}",
                Self::FIELD_COUNT,
                fields.len()
            ));
        }

        Ok(fields)
    }

    fn input(config: &RuntimeConfig) -> Result<RecordBody> {
        if config.terse() {
            Self::from_terse(config)
        } else {
            Self::from_prompt(config)
        }
    }
}

impl Input for LoginFields {
    const FIELD_COUNT: usize = 2;

    fn from_prompt(config: &RuntimeConfig) -> Result<RecordBody> {
        let username = if let Some(default_username) = &config.config.commands.new.default_username
        {
            Text::new("Username?")
                .with_default(default_username)
                .prompt()?
        } else {
            Text::new("Username?").prompt()?
        };

        let mut password = Pass::new("Password?")
            .with_help_message("Press [enter] to auto-generate")
            .prompt()?;

        if password.is_empty() {
            password = config.generator()?.secret()?;
        }

        Ok(RecordBody::Login(LoginFields { username, password }))
    }

    fn from_terse(config: &RuntimeConfig) -> Result<RecordBody> {
        // NOTE: Backwards order here because we're popping from the vector.
        let (mut password, username) = {
            let mut fields = Self::take_terse_fields()?;

            // Unwrap safety: take_terse_fields checks FIELD_COUNT to ensure sufficient elements.
            #[allow(clippy::unwrap_used)]
            (fields.pop().unwrap(), fields.pop().unwrap())
        };

        if password.is_empty() {
            password = config.generator()?.secret()?;
        }

        Ok(RecordBody::Login(LoginFields { username, password }))
    }
}

impl Input for EnvironmentFields {
    const FIELD_COUNT: usize = 2;

    fn from_prompt(config: &RuntimeConfig) -> Result<RecordBody> {
        let variable = Text::new("Variable?").prompt()?;
        let mut value = Pass::new("Value?")
            .with_help_message("Press [enter] to auto-generate")
            .prompt()?;

        if value.is_empty() {
            value = config.generator()?.secret()?;
        }

        Ok(RecordBody::Environment(EnvironmentFields {
            variable,
            value,
        }))
    }

    fn from_terse(config: &RuntimeConfig) -> Result<RecordBody> {
        // NOTE: Backwards order here because we're popping from the vector.
        let (mut value, variable) = {
            let mut fields = Self::take_terse_fields()?;

            // Unwrap safety: take_terse_fields checks FIELD_COUNT to ensure sufficient elements.
            #[allow(clippy::unwrap_used)]
            (fields.pop().unwrap(), fields.pop().unwrap())
        };

        if value.is_empty() {
            value = config.generator()?.secret()?;
        }

        Ok(RecordBody::Environment(EnvironmentFields {
            variable,
            value,
        }))
    }
}

impl Input for UnstructuredFields {
    const FIELD_COUNT: usize = 1;

    fn from_prompt(_config: &RuntimeConfig) -> Result<RecordBody> {
        let contents = Text::new("Contents?").prompt()?;

        Ok(RecordBody::Unstructured(UnstructuredFields { contents }))
    }

    fn from_terse(_config: &RuntimeConfig) -> Result<RecordBody> {
        // Unwrap safety: take_terse_fields checks FIELD_COUNT to ensure sufficient elements.
        #[allow(clippy::unwrap_used)]
        let contents = Self::take_terse_fields()?.pop().unwrap();

        Ok(RecordBody::Unstructured(UnstructuredFields { contents }))
    }
}

// /// Given an array of field names and a potential generator, grabs the values for
// /// those fields in a terse manner (each separated by `TERSE_IFS`).
// ///
// /// Fields that are marked as sensitive are subsequently overwritten by the
// /// generator, if one is provided.
// fn terse_fields(names: &[FieldKind], generator: Option<&dyn Generator>) -> Result<Vec<String>> {
//     let mut input = String::new();
//     io::stdin().read_to_string(&mut input)?;

//     if input.ends_with('\n') {
//         input.pop();
//     }

//     // NOTE(ww): Handling generated inputs in terse mode is a bit of a mess.
//     // First, we collect all inputs, expecting blank slots where we'll fill
//     // in the generated values.
//     let mut fields = input
//         .split(TERSE_IFS)
//         .map(|s| s.to_string())
//         .collect::<Vec<String>>();
//     if fields.len() != names.len() {
//         return Err(anyhow!(
//             "field count mismatch: expected {}, found {}",
//             names.len(),
//             fields.len()
//         ));
//     }

//     // Then, if we have a generator configured, we iterate over the
//     // fields and insert them as appropriate.
//     if let Some(generator) = generator {
//         for (i, name) in names.iter().enumerate() {
//             if let Sensitive(_) = name {
//                 let field = fields.get_mut(i).unwrap();
//                 field.clear();
//                 field.push_str(&generator.secret()?);
//             }
//         }
//     }

//     Ok(fields)
// }

// /// Given an array of field names and a potential generator, grabs the values for those
// /// fields by prompting the user for each.
// ///
// /// If a field is marked as sensitive **and** a generator is provided, the generator
// /// is used to provide that field and the user is **not** prompted.
// fn interactive_fields(
//     names: &[FieldKind],
//     config: &Config,
//     generator: Option<&dyn Generator>,
// ) -> Result<Vec<String>> {
//     let mut fields = vec![];

//     for name in names {
//         let field = match name {
//             Sensitive(name) => {
//                 if let Some(generator) = generator {
//                     generator.secret()?
//                 } else {
//                     let field = Password::new()
//                         .with_prompt(*name)
//                         .allow_empty_password(config.commands.new.generate_on_empty)
//                         .interact()?;

//                     if field.is_empty() && config.commands.new.generate_on_empty {
//                         log::debug!("generate-on-empty with an empty field, generating a secret");

//                         let generator = config.get_generator("default").ok_or_else(|| {
//                             anyhow!("generate-on-empty configured but no default generator")
//                         })?;

//                         generator.secret()?
//                     } else {
//                         field
//                     }
//                 }
//             }
//             Insensitive(name) => Input::<String>::new().with_prompt(*name).interact()?,
//         };

//         fields.push(field);
//     }

//     Ok(fields)
// }

// /// Grabs the values for a set of field names from user input.
// ///
// /// # Arguments
// ///
// /// * `names` - the set of field names to grab
// /// * `terse` - whether or not to get fields tersely, i.e. by splitting on
// ///   `TERSE_IFS` instead of prompting for each
// /// * `config` - the active `Config`
// /// * `generator` - the generator, if any, to use for sensitive fields
// pub fn fields(
//     names: &[FieldKind],
//     terse: bool,
//     config: &Config,
//     generator: Option<&dyn Generator>,
// ) -> Result<Vec<String>> {
//     if terse {
//         terse_fields(names, generator)
//     } else {
//         interactive_fields(names, config, generator)
//     }
// }
