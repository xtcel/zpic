//! End-to-end tests for the `zpic` binary.
//!
//! Spawns the compiled `zpic` binary against a temp directory with a
//! local uploader config. These tests don't talk to the network — they
//! exercise the local filesystem pipeline end to end.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn zpic_bin() -> PathBuf {
    // `CARGO_BIN_EXE_zpic` is set by Cargo's integration-test harness.
    PathBuf::from(env!("CARGO_BIN_EXE_zpic"))
}

fn write_config(dir: &TempDir, target: &std::path::Path) {
    let cfg = format!(
        r#"
default_uploader = "local"
default_format = "markdown"
copy_after_upload = false
history_enabled = false

[rename]
strategy = "date-hash"
path = "images/{{yyyy}}/{{mm}}/{{dd}}/{{hash8}}.{{ext}}"

[format]
markdown = "![{{alt}}]({{url}})"

[uploaders.local]
type = "local"
target_dir = "{}"
public_base_url = "/images"
"#,
        target.display()
    );
    std::fs::write(dir.path().join("config.toml"), cfg).unwrap();
}

fn write_png(path: &std::path::Path) {
    // Smallest valid PNG (1x1 transparent).
    let bytes = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn upload_local_file_with_config() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let png = dir.path().join("cover.png");
    write_png(&png);

    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "upload",
            png.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("zpic binary runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("valid JSON on stdout");
    assert_eq!(json["success"], serde_json::Value::Bool(true));
    let item = &json["items"][0];
    assert_eq!(item["mime"], "image/png");
    assert_eq!(item["uploader"], "local");
    assert!(item["url"].as_str().unwrap().starts_with("/images/"));
    // File should be present on disk.
    let key = item["key"].as_str().unwrap();
    let written = target.join(key);
    assert!(written.exists(), "uploaded file at {}", written.display());
}

#[test]
fn upload_multiple_files() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    write_png(&a);
    write_png(&b);

    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "upload",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("zpic runs");
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
}

#[test]
fn dry_run_does_not_write() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let png = dir.path().join("cover.png");
    write_png(&png);

    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "upload",
            png.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .expect("zpic runs");
    assert!(out.status.success());
    // The target directory either does not exist (zpic never created it)
    // or exists but contains no files.
    if target.exists() {
        let entries: Vec<_> = std::fs::read_dir(&target)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(entries.is_empty(), "dry-run wrote something: {entries:?}");
    }
}

#[test]
fn missing_config_exits_nonzero() {
    let out = Command::new(zpic_bin())
        .args(["--config", "/nonexistent/config.toml", "doctor"])
        .output()
        .expect("zpic runs");
    assert!(!out.status.success());
}

#[test]
fn doctor_json_payload_is_well_formed() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "doctor",
            "--json",
        ])
        .output()
        .expect("zpic runs");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let checks = json["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty());
    for c in checks {
        let status = c["status"].as_str().unwrap();
        assert!(matches!(status, "pass" | "warn" | "fail"));
    }
}

#[test]
fn import_picgo_writes_native_toml() {
    let dir = TempDir::new().unwrap();
    let picgo = dir.path().join("picgo.json");
    std::fs::write(
        &picgo,
        r#"{
            "picBed": {
                "current": "github",
                "github": {
                    "repo": "me/picbed",
                    "branch": "main",
                    "token": "ghp_x",
                    "path": "img/",
                    "customUrl": "https://cdn.example.com"
                }
            }
        }"#,
    )
    .unwrap();
    let dest = dir.path().join("zpic.toml");

    let out = Command::new(zpic_bin())
        .args([
            "config",
            "import-picgo",
            "--from",
            picgo.to_str().unwrap(),
            "--to",
            dest.to_str().unwrap(),
        ])
        .output()
        .expect("zpic runs");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dest.exists());
    let contents = std::fs::read_to_string(&dest).unwrap();
    assert!(contents.contains("[uploaders.github]"));
    assert!(contents.contains("repo = \"me/picbed\""));
    // The original PicGo file must be unchanged.
    let picgo_contents = std::fs::read_to_string(&picgo).unwrap();
    assert!(picgo_contents.contains("ghp_x"));
}

#[test]
fn migrate_dry_run_does_not_rewrite() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let png = dir.path().join("cover.png");
    write_png(&png);
    let md = dir.path().join("README.md");
    std::fs::write(&md, "# Title\n\n![cover](./cover.png)\n").unwrap();

    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "migrate",
            md.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .expect("zpic runs");
    assert!(out.status.success());
    let after = std::fs::read_to_string(&md).unwrap();
    assert!(after.contains("./cover.png"));
}

#[test]
fn clipboard_flag_explains_when_no_image() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("public");
    write_config(&dir, &target);
    let out = Command::new(zpic_bin())
        .args([
            "--config",
            dir.path().join("config.toml").to_str().unwrap(),
            "upload",
            "--clipboard",
        ])
        .output()
        .expect("zpic runs");
    // We don't assert on the exit code (CI may or may not have a clipboard
    // backend); we just want the command to either succeed or produce a
    // clear error message.
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("clipboard") || out.status.success(),
        "unexpected output: {combined}"
    );
}
