use crate::{report, tool};
use clap::Args;
use hyperpane_shell::{Error, ErrorKind};
use serde::Deserialize;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Names the fusor CLI to build with; `just consumer` points it at the
/// sibling fusor checkout.
const FUSOR_BIN: &str = "FUSOR_BIN";
/// fusor's default for `[package.metadata.fusor] output`; fusor reports no
/// output directory itself: https://github.com/fusor-rs/fusor/issues/68
const DEFAULT_OUTPUT: &str = "dist";
const PROJECT_REMEDY: &str =
    "run hyperpane in a fusor application, or pass --manifest-path or --dist";

/// The fusor application to show: built now, or already built.
#[derive(Args)]
pub(crate) struct Project {
    /// Cargo.toml of the fusor application [default: the one Cargo finds from
    /// this directory]
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Skip the build and use this directory that `fusor build` wrote
    #[arg(long, conflicts_with = "manifest_path")]
    dist: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    metadata: Option<PackageMetadata>,
}

#[derive(Deserialize)]
struct PackageMetadata {
    fusor: Option<FusorMetadata>,
}

#[derive(Deserialize)]
struct FusorMetadata {
    output: Option<PathBuf>,
}

/// The fusor CLI named by `FUSOR_BIN`, otherwise `fusor` on the `PATH`.
pub(crate) fn fusor() -> Command {
    Command::new(std::env::var_os(FUSOR_BIN).unwrap_or_else(|| OsString::from("fusor")))
}

impl Project {
    /// `--dist`, or the output of `fusor build` run on the application now.
    pub(crate) fn built(&self) -> Result<PathBuf, Error> {
        if let Some(dist) = &self.dist {
            return Ok(dist.clone());
        }
        let manifest = self.manifest()?;
        let (name, output) = application(&manifest)?;
        let building = report::step(&format!("Building {name} with fusor"));
        tool::capture(
            fusor()
                .args(["build", "--quiet", "--manifest-path"])
                .arg(&manifest)
                .stderr(Stdio::inherit()),
            ErrorKind::Toolchain,
            "fix the build errors above; `hyperpane doctor` checks the fusor CLI",
        )?;
        building.done(&format!("Built {name}"));
        Ok(output)
    }

    fn manifest(&self) -> Result<PathBuf, Error> {
        let mut locate = Command::new("cargo");
        locate.args(["locate-project", "--message-format", "plain"]);
        if let Some(manifest_path) = &self.manifest_path {
            locate.arg("--manifest-path").arg(manifest_path);
        }
        let manifest = tool::capture(&mut locate, ErrorKind::Configuration, PROJECT_REMEDY)?;
        Ok(PathBuf::from(manifest.trim()))
    }
}

/// The name of the package of `manifest`, and where `fusor build` writes it.
fn application(manifest: &Path) -> Result<(String, PathBuf), Error> {
    let listing = tool::capture(
        Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .arg("--manifest-path")
            .arg(manifest),
        ErrorKind::Configuration,
        PROJECT_REMEDY,
    )?;
    let metadata: Metadata = serde_json::from_str(&listing).map_err(|error| {
        Error::new(
            ErrorKind::Configuration,
            format!("cannot read Cargo's metadata: {error}"),
            PROJECT_REMEDY,
        )
    })?;
    let (name, fusor) = metadata
        .packages
        .into_iter()
        .find(|package| package.manifest_path == manifest)
        .and_then(|package| Some((package.name, package.metadata?.fusor?)))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Configuration,
                format!("{} is not a fusor application", manifest.display()),
                "pass the Cargo.toml that has [package.metadata.fusor] with --manifest-path",
            )
        })?;
    let root = manifest
        .parent()
        .expect("a manifest Cargo located is a file in its package directory");
    let output = root.join(fusor.output.unwrap_or_else(|| DEFAULT_OUTPUT.into()));
    Ok((name, output))
}
