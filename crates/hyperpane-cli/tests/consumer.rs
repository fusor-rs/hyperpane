//! Builds and runs the fusor application in `tests/consumer` through
//! `hyperpane run` and checks what the injected probe observed. `just consumer`
//! and `just consumer-ios` point `FUSOR_BIN` at the sibling fusor checkout.
use serde_json::{Value, json};
use std::{fs, process::Command};

const MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/consumer/Cargo.toml"
);
const DIST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/consumer/dist");
const PROBE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/consumer/probe.js");
const TIMEOUT_SECONDS: &str = "180";

fn run_consumer(platform_arguments: &[&str]) -> Vec<Value> {
    let output = Command::new(env!("CARGO_BIN_EXE_hyperpane"))
        .args([
            "run",
            "--manifest-path",
            MANIFEST,
            "--script",
            PROBE,
            "--timeout",
            TIMEOUT_SECONDS,
        ])
        .args(platform_arguments)
        .output()
        .expect("hyperpane starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "hyperpane run failed\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{stdout}");
    stdout
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("host events are JSON"))
        .collect()
}

fn assert_consumer_ran(events: &[Value], device: &str) {
    let manifest: Value =
        serde_json::from_slice(&fs::read(format!("{DIST}/.fusor-output.json")).unwrap()).unwrap();
    let event = |name: &str| {
        events
            .iter()
            .find(|event| event["event"] == name)
            .unwrap_or_else(|| panic!("no {name} event in {events:?}"))
    };
    assert_eq!(event("ready")["generation"], manifest["generation"]);
    let report = &event("report")["report"];
    assert_eq!(report["errors"], json!([]));
    assert_eq!(report["href"], "hyperpane://localhost/");
    assert_eq!(report["isSecureContext"], true);
    assert_eq!(report["wasm"], "object");
    assert_eq!(report["appRoot"], true);
    assert_eq!(report["sharedArrayBuffer"], "undefined");
    assert_eq!(
        report["signal"],
        json!({
            "before": { "text": "Clicks: 0", "pressed": "false" },
            "after": { "text": "Clicks: 1", "pressed": "true" },
        })
    );
    assert_eq!(report["worker"], "Worker answered 42");
    assert_eq!(
        report["fetch"],
        json!({ "app": "Fetched: hello from the publication", "status": 200, "missing": 404 })
    );
    assert_eq!(
        report["history"],
        json!({ "path": "/details", "page": "Details", "added": 1 })
    );
    assert_eq!(
        report["reload"],
        json!({ "path": "/details", "page": "Details" })
    );
    let user_agent = report["userAgent"].as_str().unwrap();
    assert!(user_agent.contains(device), "{user_agent}");
}

#[test]
fn consumer_runs_in_a_macos_window() {
    assert_consumer_ran(&run_consumer(&[]), "Macintosh");
}

#[test]
fn consumer_runs_in_the_ios_simulator() {
    let ios = [
        "--ios",
        "--name",
        "Consumer",
        "--bundle-id",
        "build.fusor.hyperpane.consumer",
    ];
    assert_consumer_ran(&run_consumer(&ios), "iPhone");
}
