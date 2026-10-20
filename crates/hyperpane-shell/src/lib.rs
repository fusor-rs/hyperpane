//! The hyperpane host: shows a fusor publication in WKWebView, in a window on
//! macOS and as the root view of an iOS app, from the same code.
//!
//! The `hyperpane-host` binary prints one JSON object per line on standard
//! output for each event a harness consumes (`ready`, `report`, `error`); see
//! `docs/consumption.md` in the repository.
mod bridge;
mod config;
mod error;
mod host;
mod loopback;
mod publication;

pub use bridge::HostEvent;
pub use config::{BUNDLE_CONFIG, DEFAULT_NAME, RunConfig, Source, read_script};
pub use error::{Error, ErrorKind};
pub use publication::{Origin, Publication};

use host::Host;
use std::process::ExitCode;

/// Loads the run configuration for this platform and runs the host until the
/// window closes. Returns only when the host cannot start.
pub fn start() -> ExitCode {
    let error = match RunConfig::load().and_then(Host::new) {
        Ok(host) => host.run(),
        Err(error) => error,
    };
    bridge::emit(&HostEvent::Error(error));
    ExitCode::FAILURE
}
