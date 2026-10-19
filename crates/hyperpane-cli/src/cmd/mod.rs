mod bundle;
mod doctor;
mod run;
mod upgrade;

use clap::{Args, Parser, Subcommand};
use hyperpane_shell::{DEFAULT_NAME, Error};
use std::path::PathBuf;

const DEFAULT_BUNDLE_ID: &str = "build.fusor.hyperpane";

#[derive(Parser)]
#[command(
    name = "hyperpane",
    version,
    about = "Run fusor applications as macOS and iOS apps",
    after_help = "Examples:\n  hyperpane run\n  hyperpane run --ios\n  hyperpane run --dist dist\n  hyperpane run --url http://127.0.0.1:4173/"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build the fusor application and show it in a macOS window, or in the iOS
    /// Simulator with --ios
    Run(run::Run),
    /// Build the fusor application and assemble its iOS Simulator app without
    /// launching it
    Bundle(bundle::Bundle),
    /// Check the tools that iOS builds need
    Doctor(doctor::Doctor),
    /// Upgrade to the latest release
    Upgrade(upgrade::Upgrade),
}

/// The app's identity and what the host loads into it, shared by `run` and
/// `bundle`.
#[derive(Args)]
pub(crate) struct AppOptions {
    /// The app's name
    #[arg(long, default_value = DEFAULT_NAME)]
    pub(crate) name: String,
    /// The app's bundle identifier
    #[arg(long, default_value = DEFAULT_BUNDLE_ID, requires = "ios")]
    pub(crate) bundle_id: String,
    /// A JavaScript file to inject into every page; repeatable
    #[arg(long = "script")]
    pub(crate) scripts: Vec<PathBuf>,
    /// Use this host executable instead of building one from this checkout
    #[arg(long)]
    pub(crate) host_binary: Option<PathBuf>,
}

impl Cli {
    pub(crate) fn run(self) -> Result<(), Error> {
        match self.command {
            Command::Run(command) => command.run(),
            Command::Bundle(command) => command.run(),
            Command::Doctor(command) => command.run(),
            Command::Upgrade(command) => command.run(),
        }
    }
}
