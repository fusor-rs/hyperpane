mod simulator;

use super::{AppOptions, bundle};
use crate::{host, project::Project, report};
use clap::Args;
use hyperpane_shell::{Error, ErrorKind};
use simulator::Simulator;
use std::{
    ffi::OsString,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

const HOST_REMEDY: &str = "read the host's error event above";

#[derive(Args)]
pub(crate) struct Run {
    #[command(flatten)]
    project: Project,
    /// Show a development server in a macOS window instead
    #[arg(long, conflicts_with_all = ["manifest_path", "dist", "ios"])]
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
}

impl Run {
    pub(super) fn run(self) -> Result<(), Error> {
        let timeout = self.timeout.map(Duration::from_secs);
        report::banner(if self.ios {
            "iOS Simulator"
        } else {
            "macOS window"
        });
        if let Some(url) = self.url {
            return mac(&["--url".into(), url.into()], &self.app, timeout);
        }
        let dist = self.project.built()?;
        if self.ios {
            return ios(&dist, &self.app, self.simulator.as_deref(), timeout);
        }
        mac(&[dist.into()], &self.app, timeout)
    }
}

/// Opens the host in a macOS window. `page` is the directory `fusor build`
/// wrote, or `--url` and a development server address.
fn mac(page: &[OsString], app: &AppOptions, timeout: Option<Duration>) -> Result<(), Error> {
    let mut command = Command::new(host::locate(app.host_binary.as_deref(), host::MAC_TARGET)?);
    command.args(page).arg("--name").arg(&app.name);
    for script in &app.scripts {
        command.arg("--script").arg(script);
    }
    let child = command.stdout(Stdio::piped()).spawn().map_err(|error| {
        Error::new(
            ErrorKind::Host,
            format!("cannot start the host: {error}"),
            HOST_REMEDY,
        )
    })?;
    report::done(&format!("Opened {}", app.name));
    relay(child, timeout, || Ok(()))
}

/// Bundles the output, installs it in the iOS Simulator and launches it.
fn ios(
    dist: &Path,
    app: &AppOptions,
    simulator: Option<&str>,
    timeout: Option<Duration>,
) -> Result<(), Error> {
    let bundle = bundle::ios(dist, app, &std::env::temp_dir().join("hyperpane"))?;
    let simulator = Simulator::select(simulator)?;
    let booting = report::step(&format!("Booting {simulator}"));
    simulator.boot()?;
    booting.done(&format!("Booted {simulator}"));
    let installing = report::step(&format!("Installing {}.app", app.name));
    simulator.install(&bundle)?;
    installing.done(&format!("Installed {}.app", app.name));
    let console = simulator.launch(&app.bundle_id)?;
    report::done(&format!("Launched {} on {simulator}", app.name));
    report::field("App", &app.bundle_id);
    report::field("UDID", simulator.udid());
    relay(console, timeout, || simulator.terminate(&app.bundle_id))
}

/// Prints the child's output line by line until it exits. After `timeout`,
/// calls `stop`, kills the child and fails.
fn relay(
    mut child: Child,
    timeout: Option<Duration>,
    stop: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    let output = child
        .stdout
        .take()
        .expect("the child's standard output is piped");
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        // A read error ends the output like an exit does; the exit status
        // below reports the failure.
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = timeout.map(|timeout| Instant::now() + timeout);
    loop {
        let next = match deadline {
            Some(deadline) => {
                lines.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => lines.recv().map_err(RecvTimeoutError::from),
        };
        match next {
            Ok(line) => report::relay(&line),
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                stop()?;
                return Err(timed_out(&mut child));
            }
        }
    }
    let status = child.wait().map_err(|error| {
        Error::new(
            ErrorKind::Host,
            format!("cannot wait for the host: {error}"),
            HOST_REMEDY,
        )
    })?;
    if status.success() {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Host,
        format!("the host exited with {status}"),
        HOST_REMEDY,
    ))
}

fn timed_out(child: &mut Child) -> Error {
    let message = "the page did not report before --timeout passed";
    match child.kill() {
        Ok(()) => Error::new(
            ErrorKind::Host,
            message,
            "raise --timeout, or check the output above",
        ),
        Err(error) => Error::new(
            ErrorKind::Host,
            format!("{message}, and the host could not be stopped: {error}"),
            "stop the host process by hand",
        ),
    }
}
