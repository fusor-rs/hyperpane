use serde::{Deserialize, Serialize};
use std::fmt;

/// The boundary at which hyperpane failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    Configuration,
    Publication,
    Host,
    WebView,
    Loopback,
    Toolchain,
    Simulator,
    Bundle,
    Upgrade,
}

/// A failure, what caused it and what to do about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub remedy: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            remedy: remedy.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}\n  fix: {}", self.message, self.remedy)
    }
}

impl std::error::Error for Error {}
