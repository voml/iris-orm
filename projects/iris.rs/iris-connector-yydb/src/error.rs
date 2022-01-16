//! Error mapping for the YYDB connector.

use crate::ReadinessReport;

/// Connector errors.
#[derive(Debug)]
pub enum Error {
    /// YYDB backend error.
    Yydb(yydb::Error),
    /// YYDB VOS executor is not ready for Iris.
    NotReady(ReadinessReport),
    /// Connector-local failure.
    Runtime(String),
    /// Policy / contract violation.
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Yydb(e) => write!(f, "{e}"),
            Self::NotReady(r) => write!(f, "{}: {}", r.code, r.message),
            Self::Runtime(s) | Self::Policy(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yydb(e) => Some(e),
            _ => None,
        }
    }
}

impl From<yydb::Error> for Error {
    fn from(value: yydb::Error) -> Self {
        Self::Yydb(value)
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
