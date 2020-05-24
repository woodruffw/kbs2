use serde::{Deserialize, Serialize};

use crate::kbs2::util;

// TODO(ww): Figure out how to generate this from the RecordBody enum below.
pub static RECORD_KINDS: &[&str] = &["login", "environment", "unstructured"];

#[derive(Debug)]
pub enum FieldKind {
    Insensitive(&'static str),
    Sensitive(&'static str),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Record {
    pub timestamp: u64,
    pub label: String,
    pub body: RecordBody,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "fields")]
pub enum RecordBody {
    Login(LoginFields),
    Environment(EnvironmentFields),
    Unstructured(UnstructuredFields),
}

impl std::fmt::Display for RecordBody {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RecordBody::Login(_) => write!(f, "login"),
            RecordBody::Environment(_) => write!(f, "environment"),
            RecordBody::Unstructured(_) => write!(f, "unstructured"),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct LoginFields {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct EnvironmentFields {
    pub variable: String,
    pub value: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct UnstructuredFields {
    pub contents: String,
}

impl Record {
    pub fn login(label: &str, username: &str, password: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_owned(),
            body: RecordBody::Login(LoginFields {
                username: username.to_owned(),
                password: password.to_owned(),
            }),
        }
    }

    pub fn environment(label: &str, variable: &str, value: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_owned(),
            body: RecordBody::Environment(EnvironmentFields {
                variable: variable.to_owned(),
                value: value.to_owned(),
            }),
        }
    }

    pub fn unstructured(label: &str, contents: &str) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.to_owned(),
            body: RecordBody::Unstructured(UnstructuredFields {
                contents: contents.to_owned(),
            }),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq)]
pub enum RecordKindV1 {
    Login,
    Environment,
    Unstructured,
}

#[derive(Debug, Deserialize)]
pub struct FieldV1 {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct RecordV1 {
    pub timestamp: u64,
    pub label: String,
    pub kind: RecordKindV1,
    pub fields: Vec<FieldV1>,
}

impl RecordV1 {
    pub fn to_record(&self) -> Record {
        match self.kind {
            RecordKindV1::Login => {
                let username = &self
                    .fields
                    .iter()
                    .find(|f| f.name == "username")
                    .unwrap()
                    .value;
                let password = &self
                    .fields
                    .iter()
                    .find(|f| f.name == "password")
                    .unwrap()
                    .value;

                Record::login(&self.label, username, password)
            }
            RecordKindV1::Environment => {
                let variable = &self
                    .fields
                    .iter()
                    .find(|f| f.name == "variable")
                    .unwrap()
                    .value;
                let value = &self
                    .fields
                    .iter()
                    .find(|f| f.name == "value")
                    .unwrap()
                    .value;

                Record::environment(&self.label, variable, value)
            }
            RecordKindV1::Unstructured => {
                let contents = &self
                    .fields
                    .iter()
                    .find(|f| f.name == "contents")
                    .unwrap()
                    .value;

                Record::unstructured(&self.label, contents)
            }
        }
    }
}
