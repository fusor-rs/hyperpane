use crate::{report, tool};
use hyperpane_shell::{Error, ErrorKind};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) const HOST_BINARY: &str = "hyperpane-host";
pub(crate) const IOS_SIMULATOR_TARGET: &str = "aarch64-apple-ios-sim";
/// The oldest iOS a bundle declares; the host is compiled for the same release.
pub(crate) const MINIMUM_IOS: &str = "17.0";
/// The CLI builds the host from the checkout it was built from.
const WORKSPACE_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");

#[derive(Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoMessage {
    CompilerArtifact {
        executable: Option<PathBuf>,
    },
    #[serde(other)]
    Other,
}

/// The given host executable, or one built from this checkout for `target`
/// (this Mac when `None`).
pub(crate) fn locate(given: Option<&Path>, target: Option<&str>) -> Result<PathBuf, Error> {
    if let Some(given) = given {
        return Ok(given.to_path_buf());
    }
    let mut cargo = Command::new("cargo");
    cargo
        .args(["build", "--release", "--locked"])
        .args(["--message-format", "json-render-diagnostics"])
        .args(["--manifest-path", WORKSPACE_MANIFEST])
        .args(["--package", "hyperpane-shell", "--bin", HOST_BINARY])
        .stderr(Stdio::inherit());
    if let Some(target) = target {
        cargo
            .args(["--target", target])
            .env("IPHONEOS_DEPLOYMENT_TARGET", MINIMUM_IOS);
    }
    report::step(&format!(
        "building {HOST_BINARY} for {}",
        target.unwrap_or("this Mac")
    ));
    let output = tool::capture(
        &mut cargo,
        ErrorKind::Toolchain,
        "fix the build errors above, or pass a prebuilt host with --host-binary",
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
    executable.ok_or_else(|| {
        Error::new(
            ErrorKind::Toolchain,
            format!("Cargo built no {HOST_BINARY} executable"),
            "pass a prebuilt host with --host-binary",
        )
    })
}
