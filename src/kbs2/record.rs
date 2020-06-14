use serde::{Deserialize, Serialize};

use crate::kbs2::util;

// TODO(ww): Figure out how to generate this from the RecordBody enum below.
/// The stringified names of record kinds known to `kbs2`.
pub static RECORD_KINDS: &[&str] = &["login", "environment", "unstructured"];

/// The kinds of fields known to `kbs2`.
///
/// * "Insensitive" fields are accessed with terminal echo and cannot be generated.
/// * "Sensitive" fields are accessed without terminal echo and can be generated.
#[derive(Debug)]
pub enum FieldKind {
    Insensitive(&'static str),
    Sensitive(&'static str),
}

/// Represents the envelope of a `kbs2` record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Record {
    /// When the record was created, as seconds since the Unix epoch.
    pub timestamp: u64,

    /// The identifying label of the record.
    pub label: String,

    /// The type contents of the record.
    pub body: RecordBody,
}

/// Represents the core contents of a `kbs2` record.
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

/// Represents the fields of a login record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct LoginFields {
    /// The username associated with the login.
    pub username: String,

    /// The password associated with the login.
    pub password: String,
}

/// Represents the fields of an environment record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct EnvironmentFields {
    /// The variable associated with the environment.
    pub variable: String,

    /// The value associated with the environment.
    pub value: String,
}

/// Represents the fields of an unstructured record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct UnstructuredFields {
    /// The contents associated with the record.
    pub contents: String,
}

impl Record {
    /// Creates and returns a new login record with the given label, username, and password.
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

    /// Creates and returns a new environment record with the given label, variable, and value.
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

    /// Creates and returns a new unstructured record with the given label and contents.
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
