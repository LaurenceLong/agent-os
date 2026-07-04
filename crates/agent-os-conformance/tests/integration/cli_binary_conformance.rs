use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_status_binary_starts_hostd_and_reads_empty_sqlite_projection() {
    let root = isolated_temp_dir("cli-status-binary");
    fs::create_dir_all(&root).unwrap();
    let target_dir = root.join("cargo-target");
    build_cli_and_hostd_binaries(&target_dir);
    let state_db = root.join("state").join("agent-os.sqlite");

    let output = Command::new(binary_path(&target_dir, "agent-os"))
        .arg("status")
        .arg("--state-db")
        .arg(&state_db)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "agent-os status failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["state_db"], state_db.to_string_lossy().to_string());
    assert_eq!(value["threads"].as_array().unwrap().len(), 0);
    assert_eq!(value["stats"]["provider_calls"], 0);
    assert!(state_db.is_file());

    fs::remove_dir_all(root).unwrap();
}

fn build_cli_and_hostd_binaries(target_dir: &Path) {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("build")
        .arg("-p")
        .arg("agent-os-cli")
        .arg("--bin")
        .arg("agent-os")
        .arg("-p")
        .arg("agent-os-host")
        .arg("--bin")
        .arg("agent-os-hostd")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(workspace_root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo build for CLI conformance failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn binary_path(target_dir: &Path, stem: &str) -> PathBuf {
    target_dir.join("debug").join(if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    })
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

fn isolated_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "agent-os-conformance-{label}-{}-{unique}",
        std::process::id()
    ))
}
