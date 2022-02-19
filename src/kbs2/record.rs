use secrecy::Zeroize;
use serde::{Deserialize, Serialize};

use crate::kbs2::util;

// TODO(ww): Figure out how to generate this from the RecordBody enum below.
/// The stringified names of record kinds known to `kbs2`.
pub static RECORD_KINDS: &[&str] = &["login", "environment", "unstructured"];

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

impl Zeroize for Record {
    fn zeroize(&mut self) {
        self.timestamp.zeroize();
        self.label.zeroize();
        self.body.zeroize();
    }
}

/// Represents the core contents of a `kbs2` record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "fields")]
pub enum RecordBody {
    Login(LoginFields),
    Environment(EnvironmentFields),
    Unstructured(UnstructuredFields),
}

impl Zeroize for RecordBody {
    fn zeroize(&mut self) {
        match self {
            RecordBody::Login(l) => l.zeroize(),
            RecordBody::Environment(e) => e.zeroize(),
            RecordBody::Unstructured(u) => u.zeroize(),
        };
    }
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

impl Zeroize for LoginFields {
    fn zeroize(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
    }
}

/// Represents the fields of an environment record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct EnvironmentFields {
    /// The variable associated with the environment.
    pub variable: String,

    /// The value associated with the environment.
    pub value: String,
}

impl Zeroize for EnvironmentFields {
    fn zeroize(&mut self) {
        self.variable.zeroize();
        self.value.zeroize();
    }
}

/// Represents the fields of an unstructured record.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct UnstructuredFields {
    /// The contents associated with the record.
    pub contents: String,
}

impl Zeroize for UnstructuredFields {
    fn zeroize(&mut self) {
        self.contents.zeroize();
    }
}

impl Record {
    pub fn new(label: &str, body: RecordBody) -> Record {
        Record {
            timestamp: util::current_timestamp(),
            label: label.into(),
            body,
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
