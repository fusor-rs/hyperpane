use crate::{
    host::{IOS_SIMULATOR_TARGET, MINIMUM_IOS},
    project, report, tool,
};
use clap::Args;
use hyperpane_shell::{Error, ErrorKind};
use serde::Deserialize;
use std::process::Command;

/// The oldest Xcode hyperpane is tested with.
const MINIMUM_XCODE: &[u32] = &[27];
/// The first fusor release that marks shared-memory publications with
/// `.fusor-worker-headers.json`.
const MINIMUM_FUSOR: &[u32] = &[0, 1, 2];
const IOS_DEVICE_TARGET: &str = "aarch64-apple-ios";
const XCODE_DEVELOPER_SUFFIX: &str = ".app/Contents/Developer";

type Probe = fn() -> Result<String, String>;

const CHECKS: [(&str, Probe, &str); 6] = [
    (
        "Xcode developer directory",
        developer_directory,
        "sudo xcode-select -s /Applications/Xcode.app/Contents/Developer",
    ),
    ("Xcode license", license, "sudo xcodebuild -license accept"),
    (
        "Xcode version",
        xcode_version,
        "install Xcode 27 or later from the App Store, then run `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`",
    ),
    (
        "iOS Simulator runtime",
        simulator_runtime,
        "xcodebuild -downloadPlatform iOS",
    ),
    (
        "Rust iOS targets",
        rust_targets,
        "rustup target add aarch64-apple-ios-sim aarch64-apple-ios",
    ),
    (
        "fusor CLI",
        fusor_version,
        "cargo install fusor-cli --locked",
    ),
];

#[derive(Deserialize)]
struct RuntimeList {
    runtimes: Vec<Runtime>,
}

#[derive(Deserialize)]
struct Runtime {
    name: String,
    platform: String,
}

#[derive(Args)]
pub(crate) struct Doctor;

impl Doctor {
    /// Prints one line per check and fails when any check fails.
    pub(super) fn run(self) -> Result<(), Error> {
        report::banner("doctor");
        let mut failed = 0;
        for (name, probe, remedy) in CHECKS {
            let outcome = probe();
            report::check(name, &outcome, remedy);
            failed += usize::from(outcome.is_err());
        }
        if failed == 0 {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::Toolchain,
            format!("{failed} of {} checks failed", CHECKS.len()),
            "run the commands shown after each failed check",
        ))
    }
}

fn developer_directory() -> Result<String, String> {
    let directory = output(Command::new("xcode-select").arg("-p"))?;
    if directory.ends_with(XCODE_DEVELOPER_SUFFIX) {
        return Ok(directory);
    }
    Err(format!("{directory} is not inside Xcode.app"))
}

fn license() -> Result<String, String> {
    output(Command::new("xcodebuild").args(["-license", "check"])).map(|_| "accepted".into())
}

fn xcode_version() -> Result<String, String> {
    let description = output(Command::new("xcodebuild").arg("-version"))?;
    let release = description.lines().next().unwrap_or_default().to_owned();
    at_least(&release, MINIMUM_XCODE)
}

fn simulator_runtime() -> Result<String, String> {
    let listing =
        output(Command::new("xcrun").args(["simctl", "list", "runtimes", "available", "--json"]))?;
    let list: RuntimeList = serde_json::from_str(&listing).map_err(|error| error.to_string())?;
    let names: Vec<_> = list
        .runtimes
        .into_iter()
        .filter(|runtime| runtime.platform == "iOS")
        .map(|runtime| runtime.name)
        .collect();
    if names.is_empty() {
        return Err("no iOS runtime is installed".into());
    }
    Ok(format!(
        "{} (apps require iOS {MINIMUM_IOS})",
        names.join(", ")
    ))
}

fn rust_targets() -> Result<String, String> {
    let installed = output(Command::new("rustup").args(["target", "list", "--installed"]))?;
    let missing: Vec<_> = [IOS_SIMULATOR_TARGET, IOS_DEVICE_TARGET]
        .into_iter()
        .filter(|target| !installed.lines().any(|line| line == *target))
        .collect();
    if missing.is_empty() {
        return Ok(format!("{IOS_SIMULATOR_TARGET}, {IOS_DEVICE_TARGET}"));
    }
    Err(format!("missing {}", missing.join(", ")))
}

fn fusor_version() -> Result<String, String> {
    at_least(&output(project::fusor().arg("--version"))?, MINIMUM_FUSOR)
}

fn output(command: &mut Command) -> Result<String, String> {
    tool::capture(command, ErrorKind::Toolchain, "")
        .map(|stdout| stdout.trim().to_owned())
        .map_err(|error| error.message)
}

/// `description` ends with a dotted version, such as `Xcode 27.1`.
fn at_least(description: &str, minimum: &[u32]) -> Result<String, String> {
    let version: Option<Vec<u32>> = description
        .split_whitespace()
        .last()
        .and_then(|word| word.split('.').map(|part| part.parse().ok()).collect());
    let minimum_text = minimum
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    match version {
        Some(version) if version.as_slice() >= minimum => Ok(description.to_owned()),
        Some(_) => Err(format!("{description} is older than {minimum_text}")),
        None => Err(format!("cannot read a version from {description:?}")),
    }
}
