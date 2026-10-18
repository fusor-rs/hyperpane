use crate::{
    AppOptions,
    host::{self, HOST_BINARY, IOS_SIMULATOR_TARGET, MINIMUM_IOS},
    tool,
};
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

/// Builds the host for the iOS Simulator and assembles `<name>.app` in `out`,
/// replacing an app that hyperpane assembled there before.
pub(crate) fn ios(publication: &Path, app: &AppOptions, out: &Path) -> Result<PathBuf, Error> {
    Publication::open(publication)?;
    validate(app)?;
    let config = RunConfig {
        source: Source::Publication(BUNDLE_PUBLICATION.into()),
        name: app.name.clone(),
        scripts: app
            .scripts
            .iter()
            .map(|path| read_script(path))
            .collect::<Result<_, _>>()?,
    };
    let host = host::locate(app.host_binary.as_deref(), Some(IOS_SIMULATOR_TARGET))?;
    let info = info_plist(app, &simulator_sdk_version()?);
    let bundle = out.join(format!("{}.app", app.name));
    remove_previous(&bundle)?;
    write_bundle(&bundle, &host, &info, &config, publication).map_err(|error| {
        Error::new(
            ErrorKind::Bundle,
            format!("cannot write {}: {error}", bundle.display()),
            "check that the output directory is writable and has free space",
        )
    })?;
    Ok(bundle)
}

fn write_bundle(
    bundle: &Path,
    host: &Path,
    info: &str,
    config: &RunConfig,
    publication: &Path,
) -> io::Result<()> {
    fs::create_dir_all(bundle)?;
    fs::copy(host, bundle.join(HOST_BINARY))?;
    fs::write(bundle.join("Info.plist"), info)?;
    fs::write(bundle.join("PkgInfo"), PACKAGE_TYPE)?;
    fs::write(
        bundle.join(BUNDLE_CONFIG),
        serde_json::to_vec_pretty(config)?,
    )?;
    copy_directory(publication, &bundle.join(BUNDLE_PUBLICATION))
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
fn info_plist(app: &AppOptions, sdk_version: &str) -> String {
    let name = escape_xml(&app.name);
    let bundle_id = &app.bundle_id;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>{bundle_id}</string>
  <key>CFBundleExecutable</key><string>{HOST_BINARY}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{APP_VERSION}</string>
  <key>CFBundleVersion</key><string>{BUILD_NUMBER}</string>
  <key>LSRequiresIPhoneOS</key><true/>
  <key>MinimumOSVersion</key><string>{MINIMUM_IOS}</string>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
  <key>CFBundleSupportedPlatforms</key><array><string>iPhoneSimulator</string></array>
  <key>DTPlatformName</key><string>iphonesimulator</string>
  <key>DTSDKName</key><string>iphonesimulator{sdk_version}</string>
  <key>UILaunchScreen</key><dict/>
</dict>
</plist>
"#
    )
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
