use crate::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a page sends with `window.ipc.postMessage(JSON.stringify(message))`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum PageMessage {
    /// A test harness's result. The host prints it and exits successfully.
    Report { report: Value },
}

/// One line of the host's standard output; `docs/consumption.md` describes
/// each event.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum HostEvent {
    Ready {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        generation: Option<String>,
    },
    Report {
        report: Value,
    },
    Error(Error),
}

pub(crate) fn emit(event: &HostEvent) {
    println!(
        "{}",
        serde_json::to_string(event).expect("host events contain only JSON-compatible values")
    );
}

pub(crate) fn receive(message: &str) {
    match serde_json::from_str(message) {
        Ok(PageMessage::Report { report }) => {
            emit(&HostEvent::Report { report });
            std::process::exit(0);
        }
        Err(error) => emit(&HostEvent::Error(Error::new(
            ErrorKind::WebView,
            format!("the page sent an unknown message: {error}"),
            "post messages of the documented shapes to window.ipc",
        ))),
    }
}
