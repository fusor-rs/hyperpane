use crate::{report, tool};
use clap::Args;
use hyperpane_shell::{Error, ErrorKind};
use semver::Version;
use serde::Deserialize;
use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The release workflow attaches it after everything else is published.
const READY_ASSET: &str = "install.sh";
const INSTALLER: &str = include_str!("../../install.sh");
/// Points the release check at another URL; the upgrade test serves a file.
const RELEASE_URL: &str = "HYPERPANE_RELEASE_URL";
const CHECK_TIMEOUT_SECONDS: &str = "30";
const UPGRADE_REMEDY: &str = "check your network connection and try again";

#[derive(Args)]
pub(crate) struct Upgrade;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
}

/// How this executable was installed, and so how to replace it.
enum Installation {
    /// By `install.sh`, into this directory.
    Release(PathBuf),
    /// By `cargo install`, into this root.
    Cargo(PathBuf),
}

impl Upgrade {
    pub(super) fn run(self) -> Result<(), Error> {
        report::banner("upgrade");
        let current = parse(CURRENT_VERSION)?;
        let installation = installation()?;
        let checking = report::step("Checking for a newer release");
        let Some(version) = latest(&current)? else {
            report::done(&format!("hyperpane {current} is the latest release"));
            return Ok(());
        };
        checking.done(&format!("Found hyperpane {version}"));
        let upgrading = report::step(&format!("Upgrading hyperpane {current} to {version}"));
        let mut command = match &installation {
            Installation::Release(root) => {
                let mut installer = installer();
                installer
                    .arg(version.to_string())
                    .env("HYPERPANE_INSTALL", root);
                installer
            }
            Installation::Cargo(root) => {
                let mut cargo = Command::new("cargo");
                cargo
                    .args([
                        "install",
                        "--locked",
                        "--quiet",
                        "hyperpane-cli",
                        "--version",
                    ])
                    .arg(format!("={version}"))
                    .arg("--root")
                    .arg(root);
                cargo
            }
        };
        tool::capture(
            command.stderr(Stdio::inherit()),
            ErrorKind::Upgrade,
            UPGRADE_REMEDY,
        )?;
        upgrading.done(&format!("Upgraded to hyperpane {version}"));
        Ok(())
    }
}

/// `install.sh` installs into `<root>/bin` beside `<root>/hosts`; `cargo
/// install` into `<root>/bin` beside `<root>/.crates.toml`.
fn installation() -> Result<Installation, Error> {
    let executable = env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| {
            Error::new(
                ErrorKind::Upgrade,
                format!("cannot locate the hyperpane executable: {error}"),
                "reinstall hyperpane with install.sh",
            )
        })?;
    let root = executable
        .parent()
        .filter(|directory| directory.ends_with("bin"))
        .and_then(|bin| bin.parent());
    if let Some(root) = root {
        if root.join(".crates.toml").is_file() {
            return Ok(Installation::Cargo(root.to_path_buf()));
        }
        if root.join("hosts").is_dir() {
            return Ok(Installation::Release(root.to_path_buf()));
        }
    }
    Err(Error::new(
        ErrorKind::Upgrade,
        format!(
            "{} was not installed by install.sh or cargo install",
            executable.display()
        ),
        "update the checkout it was built from, or install a release with install.sh",
    ))
}

/// The newest stable release, if it is newer than `current` and every asset
/// this Mac needs is attached.
fn latest(current: &Version) -> Result<Option<Version>, Error> {
    let repository = env!("CARGO_PKG_REPOSITORY")
        .strip_prefix("https://github.com/")
        .expect("the workspace repository is hosted on GitHub");
    // Unset, the usual case, means GitHub's latest release.
    let url = env::var(RELEASE_URL)
        .unwrap_or_else(|_| format!("https://api.github.com/repos/{repository}/releases/latest"));
    let body = tool::capture(
        Command::new("curl")
            .args(["--fail", "--silent", "--show-error", "--location"])
            .args(["--max-time", CHECK_TIMEOUT_SECONDS])
            .arg(&url),
        ErrorKind::Upgrade,
        UPGRADE_REMEDY,
    )?;
    let release: Release = serde_json::from_str(&body).map_err(|error| {
        Error::new(
            ErrorKind::Upgrade,
            format!("cannot read the release from {url}: {error}"),
            UPGRADE_REMEDY,
        )
    })?;
    let version = parse(release.tag_name.trim_start_matches('v'))?;
    if release.draft || release.prerelease || !version.pre.is_empty() {
        return Err(Error::new(
            ErrorKind::Upgrade,
            format!("the latest release, {version}, is not a stable release"),
            "try again after it is published",
        ));
    }
    if !version.cmp_precedence(current).is_gt() {
        return Ok(None);
    }
    let archive = tool::capture(
        installer().arg("--archive").arg(version.to_string()),
        ErrorKind::Upgrade,
        UPGRADE_REMEDY,
    )?;
    let archive = archive.trim();
    let checksum = format!("{archive}.sha256");
    let complete = [READY_ASSET, archive, &checksum]
        .iter()
        .all(|required| release.assets.iter().any(|asset| asset.name == *required));
    if !complete {
        return Err(Error::new(
            ErrorKind::Upgrade,
            format!("release {version} is still being published"),
            "try again in a few minutes",
        ));
    }
    Ok(Some(version))
}

/// The `install.sh` this executable was released with.
fn installer() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", INSTALLER, "install.sh"]);
    command
}

fn parse(version: &str) -> Result<Version, Error> {
    Version::parse(version).map_err(|error| {
        Error::new(
            ErrorKind::Upgrade,
            format!("{version:?} is not a release version: {error}"),
            UPGRADE_REMEDY,
        )
    })
}
