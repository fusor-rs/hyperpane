// Worker builds compile out the page: https://github.com/fusor-rs/fusor/issues/65
#![cfg_attr(
    fusor_worker,
    expect(dead_code, unused_imports, reason = "worker builds exclude the UI")
)]
use fusor::{OwnerHandle, Signal, signal};
use fusor_async::{
    Resource, ResourceState,
    browser::resource,
    fetch::{FetchError, get_text},
};
use fusor_worker::{JobError, NoError, TaskResult};

const QUESTION: u32 = 21;
const GREETING: &str = "/greeting.txt";

#[fusor_worker::task]
pub fn double(value: u32) -> TaskResult<u32> {
    Ok(value * 2)
}

struct Consumer {
    clicks: Signal<u32>,
    doubled: Resource<u32, u32, JobError<NoError>>,
    greeting: Resource<(), String, FetchError>,
}

impl Consumer {
    fn new(owner: &OwnerHandle) -> Self {
        let worker_owner = owner.clone();
        Self {
            clicks: signal(0),
            doubled: resource(
                owner,
                || Some(QUESTION),
                move |value, cancel| double::run(&worker_owner, value).cancel_on(&cancel),
            ),
            greeting: resource(
                owner,
                || Some(()),
                |(), cancel| async move { get_text(GREETING, &cancel).await },
            ),
        }
    }

    fn worker_status(&self) -> String {
        self.doubled.with(|state| match state {
            ResourceState::Ready(data) => format!("Worker answered {}", data.value),
            ResourceState::Error { error, .. } => format!("Worker failed: {error}"),
            _ => "Worker pending".into(),
        })
    }

    fn fetch_status(&self) -> String {
        self.greeting.with(|state| match state {
            ResourceState::Ready(data) => format!("Fetched: {}", data.value.trim()),
            ResourceState::Error { error, .. } => format!("Fetch failed: {error}"),
            _ => "Fetch pending".into(),
        })
    }
}

#[expect(
    clippy::excessive_nesting,
    clippy::redundant_clone,
    clippy::too_many_lines,
    reason = "generated mount code: https://github.com/fusor-rs/fusor/issues/39"
)]
mod view {
    use super::Consumer;
    fusor::template!("web/index.html");
}
