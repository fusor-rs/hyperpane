use std::{fs, process::Command};

#[test]
fn refuses_an_output_directory_inside_the_bundled_one() {
    let dist = tempfile::tempdir().unwrap();
    fs::write(
        dist.path().join(".fusor-output.json"),
        r#"{"version":1,"generation":"g-1","base_path":"/"}"#,
    )
    .unwrap();
    fs::write(dist.path().join("index.html"), "home").unwrap();
    let out = dist.path().join("ios");
    let output = Command::new(env!("CARGO_BIN_EXE_hyperpane"))
        .args(["bundle", "--ios", "--name", "Nested", "--dist"])
        .arg(dist.path())
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("which the bundle copies"), "{stderr}");
    assert!(!out.join("Nested.app").exists());
}
