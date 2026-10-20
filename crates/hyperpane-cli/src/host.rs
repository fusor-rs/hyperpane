use crate::{report, tool};
use hyperpane_shell::{Error, ErrorKind};
use serde::Deserialize;
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) const HOST_BINARY: &str = "hyperpane-host";
pub(crate) const IOS_SIMULATOR_TARGET: &str = "aarch64-apple-ios-sim";
pub(crate) const MAC_TARGET: &str = if cfg!(target_arch = "aarch64") {
    "aarch64-apple-darwin"
} else {
    "x86_64-apple-darwin"
};
/// The oldest iOS a bundle declares; the host is compiled for the same release.
/// fusor states no oldest WebKit: https://github.com/fusor-rs/fusor/issues/66
pub(crate) const MINIMUM_IOS: &str = "17.0";
/// A release archive, and `install.sh`, put `hosts/<target>/` beside `bin/`.
const SHIPPED_HOSTS: &str = "../hosts";
/// Where a CLI installed from crates.io keeps the hosts it installs, in the
/// home directory.
const INSTALLED_HOSTS: &str = ".hyperpane/cache";
/// The checkout this CLI was built from, when it was built from one.
const WORKSPACE_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
/// Every crate shares the workspace version, so it is also the shell's.
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HOST_REMEDY: &str = "fix the errors above, or pass a prebuilt host with --host-binary";

#[derive(Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoMessage {
    CompilerArtifact {
        executable: Option<PathBuf>,
    },
    #[serde(other)]
    Other,
}

/// The host executable for `target`: the one given, the one shipped with this
/// CLI, one built from the checkout this CLI was built from, or one installed
/// from crates.io at this CLI's version.
pub(crate) fn locate(given: Option<&Path>, target: &str) -> Result<PathBuf, Error> {
    if let Some(given) = given {
        return Ok(given.to_path_buf());
    }
    let shipped = shipped(target)?;
    if shipped.is_file() {
        return Ok(shipped);
    }
    if Path::new(WORKSPACE_MANIFEST).is_file() {
        return build_from_checkout(target);
    }
    install_published(target)
}

fn shipped(target: &str) -> Result<PathBuf, Error> {
    let executable = env::current_exe().map_err(|error| {
        Error::new(
            ErrorKind::Configuration,
            format!("cannot locate the hyperpane executable: {error}"),
            "pass a prebuilt host with --host-binary",
        )
    })?;
    let bin = executable
        .parent()
        .expect("an executable's path names a file inside a directory");
    Ok(bin.join(SHIPPED_HOSTS).join(target).join(HOST_BINARY))
}

fn build_from_checkout(target: &str) -> Result<PathBuf, Error> {
    let building = report::step(&format!("Building {HOST_BINARY} for {target}"));
    let output = tool::capture(
        cargo()
            .args(["build", "--release", "--locked", "--quiet"])
            .args(["--message-format", "json-render-diagnostics"])
            .args(["--manifest-path", WORKSPACE_MANIFEST])
            .args([
                "--package",
                "hyperpane-shell",
                "--bin",
                HOST_BINARY,
                "--target",
                target,
            ]),
        ErrorKind::Toolchain,
        HOST_REMEDY,
    )?;
    let mut executable = None;
    for line in output.lines() {
        let message = serde_json::from_str(line).map_err(|error| {
            Error::new(
                ErrorKind::Toolchain,
                format!("cannot read Cargo's build output: {error}"),
                "pass a prebuilt host with --host-binary",
            )
        })?;
        if let CargoMessage::CompilerArtifact {
            executable: Some(path),
        } = message
        {
            executable = Some(path);
        }
    }
    let executable = executable.ok_or_else(|| {
        Error::new(
            ErrorKind::Toolchain,
            format!("Cargo built no {HOST_BINARY} executable"),
            "pass a prebuilt host with --host-binary",
        )
    })?;
    building.done(&format!("Built {HOST_BINARY}"));
    Ok(executable)
}

/// Installs once per version and target; later runs reuse the installation.
fn install_published(target: &str) -> Result<PathBuf, Error> {
    let home = env::var_os("HOME").ok_or_else(|| {
        Error::new(
            ErrorKind::Configuration,
            "HOME is not set, so there is nowhere to install the host",
            "set HOME, or pass a prebuilt host with --host-binary",
        )
    })?;
    let root = PathBuf::from(home)
        .join(INSTALLED_HOSTS)
        .join(VERSION)
        .join(target);
    let executable = root.join("bin").join(HOST_BINARY);
    if executable.is_file() {
        return Ok(executable);
    }
    let installing = report::step(&format!(
        "Installing {HOST_BINARY} {VERSION} for {target} from crates.io"
    ));
    tool::capture(
        cargo()
            .args(["install", "--locked", "--quiet", "hyperpane-shell"])
            .args(["--version", &format!("={VERSION}")])
            .args(["--bin", HOST_BINARY, "--target", target, "--root"])
            .arg(&root),
        ErrorKind::Toolchain,
        HOST_REMEDY,
    )?;
    installing.done(&format!("Installed {HOST_BINARY}"));
    Ok(executable)
}

/// Cargo with the iOS deployment target set; builds for macOS ignore it.
fn cargo() -> Command {
    let mut cargo = Command::new("cargo");
    cargo
        .env("IPHONEOS_DEPLOYMENT_TARGET", MINIMUM_IOS)
        .stderr(Stdio::inherit());
    cargo
}
