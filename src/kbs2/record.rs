use serde::{Deserialize, Serialize};

use std::time::{SystemTime, UNIX_EPOCH};

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

fn current_timestamp() -> u64 {
    // NOTE(ww): This unwrap should be safe, since every time should be
    // greater than or equal to the epoch.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn new_login(label: &str, username: &str, password: &str) -> Record {
    Record {
        timestamp: current_timestamp(),
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

pub fn new_environment(label: &str, variable: &str, value: &str) -> Record {
    Record {
        timestamp: current_timestamp(),
        label: label.to_string(),
        kind: RecordKind::Login,
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

pub fn new_unstructured(label: &str, contents: &str) -> Record {
    Record {
        timestamp: current_timestamp(),
        label: label.to_string(),
        kind: RecordKind::Login,
        fields: vec![Field {
            name: "contents".into(),
            value: contents.into(),
        }],
    }
}
