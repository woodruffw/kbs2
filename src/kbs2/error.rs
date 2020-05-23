use std::error;
use std::fmt;

// TODO(ww): This custom Error and collection of From<...>s is terrible.
// It should be replaced with anyhow: https://github.com/dtolnay/anyhow
#[derive(Debug, Clone)]
pub struct Error {
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<String> for Error {
    fn from(err: String) -> Error {
        Error { message: err }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(err: serde_json::error::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}

impl From<pinentry::Error> for Error {
    fn from(err: pinentry::Error) -> Error {
        Error {
            message: err.to_string(),
        }
    }
}
