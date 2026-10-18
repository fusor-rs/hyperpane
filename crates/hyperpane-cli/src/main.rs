//! The hyperpane command: run fusor publications as macOS and iOS apps.
mod bundle;
mod doctor;
mod host;
mod report;
mod run;
mod simulator;
mod tool;

use clap::{Args, Parser, Subcommand};
use hyperpane_shell::{DEFAULT_NAME, Error};
use std::{path::PathBuf, process::ExitCode, time::Duration};

const DEFAULT_BUNDLE_ID: &str = "build.fusor.hyperpane";

#[derive(Parser)]
#[command(
    name = "hyperpane",
    version,
    about = "Run fusor publications as macOS and iOS apps",
    after_help = "Examples:\n  hyperpane run dist\n  hyperpane run dist --ios\n  hyperpane run --url http://127.0.0.1:4173/"
)]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Show a publication in a macOS window, or in the iOS Simulator with --ios
    Run {
        /// The directory that `fusor build` published
        #[arg(required_unless_present = "url")]
        publication: Option<PathBuf>,
        /// Show a development server in a macOS window instead
        #[arg(long, conflicts_with_all = ["publication", "ios"])]
        url: Option<String>,
        /// Build, install and launch the app in the iOS Simulator
        #[arg(long)]
        ios: bool,
        /// The simulator to use, by name or UDID
        #[arg(long, requires = "ios")]
        simulator: Option<String>,
        /// Fail unless the page reports within this many seconds
        #[arg(long)]
        timeout: Option<u64>,
        #[command(flatten)]
        app: AppOptions,
    },
    /// Assemble the iOS Simulator app without launching it
    Bundle {
        /// The directory that `fusor build` published
        publication: PathBuf,
        /// Bundle for the iOS Simulator
        #[arg(long, required = true)]
        ios: bool,
        /// The directory that receives <name>.app
        #[arg(long)]
        out: PathBuf,
        #[command(flatten)]
        app: AppOptions,
    },
    /// Check the tools that iOS builds need
    Doctor,
}

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

fn main() -> ExitCode {
    match run(Cli::parse().action) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report::failure(&error);
            ExitCode::FAILURE
        }
    }
}

fn run(action: Action) -> Result<(), Error> {
    match action {
        Action::Run {
            publication,
            url,
            ios,
            simulator,
            timeout,
            app,
        } => {
            let timeout = timeout.map(Duration::from_secs);
            match (publication, url) {
                (Some(publication), None) if ios => {
                    run::ios(&publication, &app, simulator.as_deref(), timeout)
                }
                (Some(publication), None) => run::mac(&[publication.into()], &app, timeout),
                (None, Some(url)) => run::mac(&["--url".into(), url.into()], &app, timeout),
                _ => unreachable!("clap accepts exactly one of a publication and --url"),
            }
        }
        Action::Bundle {
            publication,
            out,
            app,
            ..
        } => {
            let bundle = bundle::ios(&publication, &app, &out)?;
            report::step(&format!("assembled {}", bundle.display()));
            Ok(())
        }
        Action::Doctor => doctor::run(),
    }
}
