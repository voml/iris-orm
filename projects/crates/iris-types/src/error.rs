//! Error type for iris-types.

use crate::diagnostic::Diagnostic;

/// Core errors.
#[derive(Debug)]
pub enum Error {
    /// Structured diagnostic failure (parse/plan/execute).
    Diagnostic(Diagnostic),
    /// Envelope / IR version mismatch.
    Ir(iris_ir::Error),
    /// Reference store / runtime failure.
    Runtime(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostic(d) => write!(f, "{}", d.message),
            Self::Ir(e) => write!(f, "{e}"),
            Self::Runtime(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ir(e) => Some(e),
            _ => None,
        }
    }
}

impl From<iris_ir::Error> for Error {
    fn from(value: iris_ir::Error) -> Self {
        Self::Ir(value)
    }
}

impl Error {
    /// Convenience: wrap a diagnostic.
    pub fn diagnostic(diag: Diagnostic) -> Self {
        Self::Diagnostic(diag)
    }

    /// Source span when this is a diagnostic error.
    pub fn span(&self) -> Option<vos::ast::Span> {
        match self {
            Self::Diagnostic(d) => Some(d.span),
            _ => None,
        }
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
