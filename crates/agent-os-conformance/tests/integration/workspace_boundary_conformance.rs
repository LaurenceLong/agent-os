use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

#[test]
fn workspace_agent_os_normal_dependencies_match_layer_contract() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(&workspace_root)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: Value = serde_json::from_slice(&output.stdout).expect("parse cargo metadata");
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .expect("workspace_members")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();

    let expected = BTreeMap::from([
        ("agent-os-sys", BTreeSet::new()),
        ("agent-os-store", set(["agent-os-sys"])),
        (
            "agent-os-store-sqlite",
            set(["agent-os-store", "agent-os-sys"]),
        ),
        ("agent-os-kernel", set(["agent-os-store", "agent-os-sys"])),
        ("agent-os-thread", set(["agent-os-kernel", "agent-os-sys"])),
        ("agent-os-config", set(["agent-os-sys"])),
        (
            "agent-os-ecosystem",
            set(["agent-os-config", "agent-os-kernel", "agent-os-sys"]),
        ),
        ("agent-os-distro", set(["agent-os-sys"])),
        ("agent-os-app-server", set(["agent-os-sys"])),
        (
            "agent-os-host",
            set([
                "agent-os-app-server",
                "agent-os-config",
                "agent-os-ecosystem",
                "agent-os-kernel",
                "agent-os-store",
                "agent-os-store-sqlite",
                "agent-os-sys",
                "agent-os-thread",
            ]),
        ),
        (
            "agent-os-cli",
            set([
                "agent-os-app-server",
                "agent-os-config",
                "agent-os-distro",
                "agent-os-sys",
            ]),
        ),
    ]);

    let packages = metadata["packages"].as_array().expect("packages");
    let mut actual_by_package = BTreeMap::new();
    for package in packages {
        let id = package["id"].as_str().expect("package id");
        if !workspace_members.contains(id) {
            continue;
        }
        let name = package["name"].as_str().expect("package name");
        if name == "agent-os-conformance" {
            continue;
        }
        let actual = package["dependencies"]
            .as_array()
            .expect("dependencies")
            .iter()
            .filter(|dependency| dependency["kind"].is_null())
            .filter_map(|dependency| dependency["name"].as_str())
            .filter(|dependency| dependency.starts_with("agent-os-"))
            .collect::<BTreeSet<_>>();
        actual_by_package.insert(name.to_string(), actual);
    }

    let actual_names = actual_by_package
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_names = expected.keys().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        actual_names, expected_names,
        "workspace dependency contract must enumerate every production agent-os crate"
    );

    for (name, allowed) in expected {
        let actual = actual_by_package
            .get(name)
            .unwrap_or_else(|| panic!("missing dependency boundary entry for {name}"));
        assert_eq!(
            actual, &allowed,
            "{name} normal agent-os dependency boundary changed"
        );
    }
}

fn set<const N: usize>(items: [&'static str; N]) -> BTreeSet<&'static str> {
    items.into_iter().collect()
}
