//! The hyperpane command: run fusor applications as macOS and iOS apps.
mod cmd;
mod host;
mod project;
mod report;
mod tool;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cmd::Cli::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report::failure(&error);
            ExitCode::FAILURE
        }
    }
}
