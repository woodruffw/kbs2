use serde::{Deserialize, Serialize};

use crate::kbs2::error::Error;
use crate::kbs2::util;

// TODO(ww): Figure out how to generate this from the RecordKind enum below.
pub static RECORD_KINDS: &'static [&'static str] = &["login", "environment", "unstructured"];

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RecordKind {
    Login,
    Environment,
    Unstructured,
}

impl std::fmt::Display for RecordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RecordKind::Login => write!(f, "login"),
            RecordKind::Environment => write!(f, "environment"),
            RecordKind::Unstructured => write!(f, "unstructured"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Record {
    pub timestamp: u64,
    pub label: String,
    pub kind: RecordKind,
    pub fields: Vec<Field>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub name: String,
    pub value: String,
}

impl Record {
    pub fn login(label: &str, username: &str, password: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_string(),
            kind: RecordKind::Login,
            fields: vec![
                Field {
                    name: "username".into(),
                    value: username.into(),
                },
                Field {
                    name: "password".into(),
                    value: password.into(),
                },
            ],
        }
    }

    pub fn environment(label: &str, variable: &str, value: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_string(),
            kind: RecordKind::Environment,
            fields: vec![
                Field {
                    name: "variable".into(),
                    value: variable.into(),
                },
                Field {
                    name: "value".into(),
                    value: value.into(),
                },
            ],
        }
    }

    pub fn unstructured(label: &str, contents: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_string(),
            kind: RecordKind::Unstructured,
            fields: vec![Field {
                name: "contents".into(),
                value: contents.into(),
            }],
        }
    }

    // TODO(ww): Add Login, Environment, etc traits for Record to provide a nicer
    // interface than just get_expected_field.
    pub fn get_expected_field(&self, name: &str) -> Result<&str, Error> {
        Ok(&self
            .fields
            .iter()
            .find(|f| f.name == name)
            .ok_or(format!(
                "missing {} field in {} record",
                name,
                self.kind.to_string()
            ))?
            .value)
    }
}
