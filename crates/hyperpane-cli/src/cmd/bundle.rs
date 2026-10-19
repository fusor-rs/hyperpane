use super::AppOptions;
use crate::{
    host::{self, HOST_BINARY, IOS_SIMULATOR_TARGET, MINIMUM_IOS},
    project::Project,
    report, tool,
};
use clap::Args;
use hyperpane_shell::{
    BUNDLE_CONFIG, Error, ErrorKind, Publication, RunConfig, Source, read_script,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// Where the host finds the publication, relative to the bundle.
const BUNDLE_PUBLICATION: &str = "publication";
const PACKAGE_TYPE: &str = "APPL????";
const APP_VERSION: &str = "1.0";
const BUILD_NUMBER: &str = "1";

#[derive(Args)]
pub(crate) struct Bundle {
    #[command(flatten)]
    project: Project,
    /// Bundle for the iOS Simulator
    #[arg(long, required = true)]
    ios: bool,
    /// The directory that receives <name>.app
    #[arg(long)]
    out: PathBuf,
    #[command(flatten)]
    app: AppOptions,
}

impl Bundle {
    pub(super) fn run(self) -> Result<(), Error> {
        report::banner("iOS Simulator bundle");
        let bundle = ios(&self.project.built()?, &self.app, &self.out)?;
        report::field("Bundle", &bundle.display().to_string());
        Ok(())
    }
}

/// Builds the host for the iOS Simulator and assembles `<name>.app` in `out`
/// from `dist`, the directory `fusor build` wrote, replacing an app that
/// hyperpane assembled there before.
pub(super) fn ios(dist: &Path, app: &AppOptions, out: &Path) -> Result<PathBuf, Error> {
    Publication::open(dist)?;
    validate(app)?;
    validate_out(dist, out)?;
    let config = RunConfig {
        source: Source::Publication(BUNDLE_PUBLICATION.into()),
        name: app.name.clone(),
        scripts: app
            .scripts
            .iter()
            .map(|path| read_script(path))
            .collect::<Result<_, _>>()?,
    };
    let host = host::locate(app.host_binary.as_deref(), IOS_SIMULATOR_TARGET)?;
    let info = info_plist(app, &simulator_sdk_version()?);
    let bundle = out.join(format!("{}.app", app.name));
    let assembling = report::step(&format!("Assembling {}.app", app.name));
    remove_previous(&bundle)?;
    write_bundle(&bundle, &host, &info, &config, dist).map_err(|error| {
        Error::new(
            ErrorKind::Bundle,
            format!("cannot write {}: {error}", bundle.display()),
            "check that the output directory is writable and has free space",
        )
    })?;
    assembling.done(&format!("Assembled {}.app", app.name));
    Ok(bundle)
}

fn write_bundle(
    bundle: &Path,
    host: &Path,
    info: &str,
    config: &RunConfig,
    dist: &Path,
) -> io::Result<()> {
    fs::create_dir_all(bundle)?;
    fs::copy(host, bundle.join(HOST_BINARY))?;
    fs::write(bundle.join("Info.plist"), info)?;
    fs::write(bundle.join("PkgInfo"), PACKAGE_TYPE)?;
    fs::write(
        bundle.join(BUNDLE_CONFIG),
        serde_json::to_vec_pretty(config)?,
    )?;
    copy_directory(dist, &bundle.join(BUNDLE_PUBLICATION))
}

/// The name becomes the bundle's directory name. Apple allows letters,
/// digits, hyphens and periods in a bundle identifier.
fn validate(app: &AppOptions) -> Result<(), Error> {
    if app.name.is_empty() || app.name.contains('/') {
        return Err(Error::new(
            ErrorKind::Bundle,
            format!("{:?} cannot name an app bundle", app.name),
            "pass a --name without slashes",
        ));
    }
    let bundle_id_valid = !app.bundle_id.is_empty()
        && app
            .bundle_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'));
    if bundle_id_valid {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Bundle,
        format!("{:?} is not a valid bundle identifier", app.bundle_id),
        "use reverse-DNS letters, digits, hyphens and periods, such as build.fusor.example",
    ))
}

/// The bundle copies `dist`, so a bundle written inside it would copy itself
/// without end.
fn validate_out(dist: &Path, out: &Path) -> Result<(), Error> {
    let resolve = |path: &Path| {
        fs::create_dir_all(path)
            .and_then(|()| path.canonicalize())
            .map_err(|error| {
                Error::new(
                    ErrorKind::Bundle,
                    format!("cannot use {}: {error}", path.display()),
                    "pass a writable --out directory",
                )
            })
    };
    if !resolve(out)?.starts_with(resolve(dist)?) {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Bundle,
        format!(
            "{} is inside {}, which the bundle copies",
            out.display(),
            dist.display()
        ),
        "pass an --out directory outside the directory that `fusor build` wrote",
    ))
}

fn remove_previous(bundle: &Path) -> Result<(), Error> {
    if !bundle.exists() {
        return Ok(());
    }
    if !bundle.join(BUNDLE_CONFIG).is_file() {
        return Err(Error::new(
            ErrorKind::Bundle,
            format!(
                "{} exists and was not assembled by hyperpane",
                bundle.display()
            ),
            "choose another --out directory or --name",
        ));
    }
    fs::remove_dir_all(bundle).map_err(|error| {
        Error::new(
            ErrorKind::Bundle,
            format!("cannot replace {}: {error}", bundle.display()),
            "remove it and try again",
        )
    })
}

fn simulator_sdk_version() -> Result<String, Error> {
    let version = tool::capture(
        Command::new("xcrun").args(["--sdk", "iphonesimulator", "--show-sdk-version"]),
        ErrorKind::Toolchain,
        "install Xcode with the iOS platform; `hyperpane doctor` checks the setup",
    )?;
    Ok(version.trim().to_owned())
}

/// `UILaunchScreen` must be present, even empty, or iOS shows the app in a
/// 320 by 480 point compatibility window.
const INFO_PLIST: &str = include_str!("bundle/Info.plist");

fn info_plist(app: &AppOptions, sdk_version: &str) -> String {
    let name = escape_xml(&app.name);
    let values = [
        ("{{bundle_id}}", app.bundle_id.as_str()),
        ("{{executable}}", HOST_BINARY),
        ("{{name}}", &name),
        ("{{app_version}}", APP_VERSION),
        ("{{build_number}}", BUILD_NUMBER),
        ("{{minimum_ios}}", MINIMUM_IOS),
        ("{{sdk_version}}", sdk_version),
    ];
    values
        .iter()
        .fold(INFO_PLIST.to_owned(), |plist, (placeholder, value)| {
            plist.replace(placeholder, value)
        })
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn copy_directory(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
