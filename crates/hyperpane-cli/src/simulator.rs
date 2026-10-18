use crate::{report, tool};
use hyperpane_shell::{Error, ErrorKind};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::Path,
    process::{Child, Command, Stdio},
};

const IOS_RUNTIME_PREFIX: &str = "com.apple.CoreSimulator.SimRuntime.iOS-";
const BOOTED: &str = "Booted";
const IPHONE: &str = "iPhone";
const SIMCTL_REMEDY: &str = "check the simulator setup with `hyperpane doctor`";

#[derive(Deserialize)]
struct DeviceList {
    devices: BTreeMap<String, Vec<Device>>,
}

#[derive(Deserialize)]
struct Device {
    udid: String,
    name: String,
    state: String,
}

/// One iOS Simulator device, identified by its UDID.
pub(crate) struct Simulator {
    udid: String,
}

impl Simulator {
    /// The device named or identified by `requested`; otherwise a booted
    /// iPhone, otherwise the first iPhone of the newest iOS runtime.
    pub(crate) fn select(requested: Option<&str>) -> Result<Self, Error> {
        let listing = simctl(&["list", "devices", "available", "--json"])?;
        let list: DeviceList = serde_json::from_str(&listing).map_err(|error| {
            Error::new(
                ErrorKind::Simulator,
                format!("cannot read the simulator list: {error}"),
                SIMCTL_REMEDY,
            )
        })?;
        let mut runtimes: Vec<(Vec<u32>, Vec<Device>)> = list
            .devices
            .into_iter()
            .filter_map(|(runtime, devices)| Some((ios_version(&runtime)?, devices)))
            .collect();
        runtimes.sort_by(|(newer, _), (older, _)| older.cmp(newer));
        let mut devices: Vec<Device> = runtimes
            .into_iter()
            .flat_map(|(_, devices)| devices)
            .collect();
        let chosen = match requested {
            Some(requested) => devices
                .into_iter()
                .find(|device| device.udid == requested || device.name == requested),
            None => {
                devices.retain(|device| device.name.starts_with(IPHONE));
                let booted = devices.iter().position(|device| device.state == BOOTED);
                devices.into_iter().nth(booted.unwrap_or(0))
            }
        };
        let device = chosen.ok_or_else(|| {
            Error::new(
                ErrorKind::Simulator,
                match requested {
                    Some(requested) => format!("no available iOS simulator is named {requested}"),
                    None => "no iPhone simulator is available".into(),
                },
                "list the devices with `xcrun simctl list devices available`, or install a runtime with `xcodebuild -downloadPlatform iOS`",
            )
        })?;
        report::step(&format!(
            "using the {} simulator ({})",
            device.name, device.udid
        ));
        Ok(Self { udid: device.udid })
    }

    /// Boots the device if it is not running and waits until it is ready.
    pub(crate) fn boot(&self) -> Result<(), Error> {
        simctl(&["bootstatus", &self.udid, "-b"]).map(drop)
    }

    pub(crate) fn install(&self, bundle: &Path) -> Result<(), Error> {
        report::step(&format!("installing {}", bundle.display()));
        let bundle = bundle.to_string_lossy();
        simctl(&["install", &self.udid, &bundle]).map(drop)
    }

    /// Starts the app with its standard output piped to the returned process.
    pub(crate) fn launch(&self, bundle_id: &str) -> Result<Child, Error> {
        let mut launch = Command::new("xcrun");
        launch
            .args([
                "simctl",
                "launch",
                "--console",
                "--terminate-running-process",
            ])
            .args([&self.udid, bundle_id])
            .stdout(Stdio::piped());
        launch.spawn().map_err(|error| {
            Error::new(
                ErrorKind::Simulator,
                format!("cannot run {}: {error}", tool::describe(&launch)),
                SIMCTL_REMEDY,
            )
        })
    }

    pub(crate) fn terminate(&self, bundle_id: &str) -> Result<(), Error> {
        simctl(&["terminate", &self.udid, bundle_id]).map(drop)
    }
}

fn simctl(arguments: &[&str]) -> Result<String, Error> {
    tool::capture(
        Command::new("xcrun").arg("simctl").args(arguments),
        ErrorKind::Simulator,
        SIMCTL_REMEDY,
    )
}

/// `com.apple.CoreSimulator.SimRuntime.iOS-27-1` is iOS 27.1; any other
/// runtime, such as watchOS, is `None`.
fn ios_version(runtime: &str) -> Option<Vec<u32>> {
    runtime
        .strip_prefix(IOS_RUNTIME_PREFIX)?
        .split('-')
        .map(|part| part.parse().ok())
        .collect()
}
