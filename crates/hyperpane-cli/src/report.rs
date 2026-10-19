//! Everything the CLI prints. Progress goes to standard error as a short log:
//! a banner, `○` when work starts, `✓` when it is done and `✗` for a failure.
//! The host's JSON events are also written to standard output when it is not
//! a terminal, for scripts and the consumer test.
use hyperpane_shell::{Error, HostEvent};
use std::{
    io::{self, IsTerminal, Write},
    sync::OnceLock,
    time::{Duration, Instant},
};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[1;32m";
const CYAN: &str = "\x1b[1;36m";
const RED: &str = "\x1b[1;31m";
const RESET: &str = "\x1b[0m";
/// Wide enough for the longest `doctor` check name.
const CHECK_WIDTH: usize = 26;

/// Work that [`step`] announced; [`Step::done`] reports how long it took.
pub(crate) struct Step {
    started: Instant,
}

impl Step {
    pub(crate) fn done(self, detail: &str) {
        let elapsed = elapsed_text(self.started.elapsed());
        mark(GREEN, "✓", &format!("{detail} in {elapsed}"));
    }
}

/// `▲ hyperpane 0.1.0 · iOS Simulator`
pub(crate) fn banner(mode: &str) {
    let name = paint(BOLD, &format!("▲ hyperpane {}", env!("CARGO_PKG_VERSION")));
    write_line(&format!("{name} · {mode}"));
}

/// `○ Building hyperpane-host for this Mac ...`
pub(crate) fn step(detail: &str) -> Step {
    mark(CYAN, "○", &format!("{detail} ..."));
    Step {
        started: Instant::now(),
    }
}

/// `✓ Booted iPhone 17`, for work with no duration worth showing.
pub(crate) fn done(detail: &str) {
    mark(GREEN, "✓", detail);
}

/// `- Device:   iPhone 17 · iOS 27.0`
pub(crate) fn field(label: &str, value: &str) {
    write_line(&format!("- {:<9} {value}", format!("{label}:")));
}

pub(crate) fn failure(error: &Error) {
    mark(RED, "✗", &error.message);
    write_line(&format!("  {} {}", paint(DIM, "fix:"), error.remedy));
}

pub(crate) fn check(name: &str, outcome: &Result<String, String>, remedy: &str) {
    match outcome {
        Ok(detail) => mark(GREEN, "✓", &format!("{name:<CHECK_WIDTH$} {detail}")),
        Err(problem) => {
            mark(RED, "✗", &format!("{name:<CHECK_WIDTH$} {problem}"));
            write_line(&format!("  {} {remedy}", paint(DIM, "fix:")));
        }
    }
}

/// Shows one line of the host's standard output to a person, and passes it
/// on to standard output unless that is the same terminal.
pub(crate) fn relay(output: &str) {
    if !io::stdout().is_terminal() {
        // A closed standard output must not stop the app; the event is shown
        // on standard error below either way.
        let _ = writeln!(io::stdout().lock(), "{output}");
    }
    match serde_json::from_str(output) {
        Ok(HostEvent::Ready { url, .. }) => done(&format!("Loaded {url}")),
        Ok(HostEvent::Report { report }) => done(&format!("The page reported {report}")),
        Ok(HostEvent::Error(error)) => failure(&error),
        // Not a host event, such as simctl's `<bundle id>: <pid>` line.
        Err(_) => write_line(&paint(DIM, output)),
    }
}

fn mark(color: &str, symbol: &str, detail: &str) {
    write_line(&format!("{} {detail}", paint(color, symbol)));
}

fn paint(color: &str, text: &str) -> String {
    static COLOR: OnceLock<bool> = OnceLock::new();
    let colored =
        COLOR.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && io::stderr().is_terminal());
    if *colored {
        format!("{color}{text}{RESET}")
    } else {
        text.to_owned()
    }
}

fn write_line(text: &str) {
    // Progress is best effort: a closed standard error must not stop the work.
    let _ = writeln!(io::stderr().lock(), "{text}");
}

/// `<1ms`, `45ms`, `4.2s`, `1m 12s`
fn elapsed_text(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs_f64();
    if elapsed < Duration::from_millis(1) {
        return "<1ms".into();
    }
    if seconds < 1.0 {
        return format!("{}ms", elapsed.as_millis());
    }
    if seconds < 60.0 {
        return format!("{seconds:.1}s");
    }
    format!("{}m {}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
}

#[cfg(test)]
mod tests {
    use super::elapsed_text;
    use std::time::Duration;

    #[test]
    fn durations_read_the_way_a_person_writes_them() {
        for (elapsed, text) in [
            (Duration::from_micros(300), "<1ms"),
            (Duration::from_millis(45), "45ms"),
            (Duration::from_millis(4_230), "4.2s"),
            (Duration::from_secs(72), "1m 12s"),
        ] {
            assert_eq!(elapsed_text(elapsed), text);
        }
    }
}
