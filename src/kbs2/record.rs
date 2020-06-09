use serde::{Deserialize, Serialize};

use crate::kbs2::util;

// TODO(ww): Figure out how to generate this from the RecordBody enum below.
pub static RECORD_KINDS: &[&str] = &["login", "environment", "unstructured"];

#[derive(Debug)]
pub enum FieldKind {
    Insensitive(&'static str),
    Sensitive(&'static str),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login() {
        let record = Record::login("foo", "bar", "baz");

        assert_eq!(record.label, "foo");
        assert_eq!(
            record.body,
            RecordBody::Login(LoginFields {
                username: "bar".into(),
                password: "baz".into(),
            })
        );
    }

    #[test]
    fn test_environment() {
        let record = Record::environment("foo", "bar", "baz");

        assert_eq!(record.label, "foo");
        assert_eq!(
            record.body,
            RecordBody::Environment(EnvironmentFields {
                variable: "bar".into(),
                value: "baz".into(),
            })
        );
    }

    #[test]
    fn test_unstructured() {
        let record = Record::unstructured("foo", "bar");

        assert_eq!(record.label, "foo");
        assert_eq!(
            record.body,
            RecordBody::Unstructured(UnstructuredFields {
                contents: "bar".into(),
            })
        );
    }
}
