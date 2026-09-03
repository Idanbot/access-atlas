use access_atlas::discovery::{AcceptanceReport, ConnectionInventory, InventoryCache, Provider};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn validate_cli_does_not_require_discovery_configuration() {
    let output = Command::new(env!("CARGO_BIN_EXE_access-atlas"))
        .arg("--validate")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("HOME")
        .env_remove("ACCESS_ATLAS_HOME")
        .env_remove("ACCESS_ATLAS_CACHE")
        .env_remove("XDG_CACHE_HOME")
        .output()
        .expect("validation CLI should run without discovery configuration");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("validated 8 targets"));
}

#[test]
fn discover_cli_supports_a_fresh_isolated_install() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let bin = sandbox.path().join("bin");
    fs::create_dir_all(&bin).expect("mock bin should exist");
    write_executable(
        &bin.join("kubectl"),
        "#!/bin/sh\nprintf '%s\\n' '{\"contexts\":[{\"name\":\"docker-mock\",\"context\":{\"cluster\":\"fresh\"}}],\"clusters\":[]}'\n",
    );
    write_executable(
        &bin.join("aws"),
        "#!/bin/sh\nprintf '%s\\n' 'sandbox-profile'\n",
    );
    write_executable(
        &bin.join("docker"),
        "#!/bin/sh\nprintf '%s\\n' '{\"Name\":\"mock-daemon\",\"DockerEndpoint\":\"unix:///tmp/mock.sock\",\"Current\":\"true\"}'\n",
    );
    let cache_path = sandbox.path().join("cache/connections.json");

    let output = Command::new(env!("CARGO_BIN_EXE_access-atlas"))
        .args([
            "--discover",
            "--connections-cache",
            cache_path.to_str().expect("cache path is UTF-8"),
            "--discovery-home",
            sandbox.path().to_str().expect("home path is UTF-8"),
        ])
        .env("PATH", &bin)
        .output()
        .expect("discovery CLI should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let inventory: ConnectionInventory =
        serde_json::from_slice(&output.stdout).expect("stdout should be inventory JSON");
    assert!(inventory.connections.iter().any(|connection| {
        connection.provider == Provider::Kubernetes && connection.label == "docker-mock"
    }));
    assert!(inventory.connections.iter().any(|connection| {
        connection.provider == Provider::Aws && connection.label == "sandbox-profile"
    }));
    assert!(inventory.connections.iter().any(|connection| {
        connection.provider == Provider::Docker && connection.label == "mock-daemon"
    }));
    assert_eq!(
        InventoryCache::new(cache_path)
            .load_or_default()
            .expect("written cache should load"),
        inventory
    );
}

#[test]
fn acceptance_cli_audits_an_isolated_install_without_executing_templates() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let bin = sandbox.path().join("bin");
    fs::create_dir_all(&bin).expect("mock bin should exist");
    write_executable(
        &bin.join("kubectl"),
        "#!/bin/sh\nprintf '%s\\n' '{\"contexts\":[{\"name\":\"acceptance\",\"context\":{\"cluster\":\"fresh\"}}],\"clusters\":[]}'\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_access-atlas"))
        .args([
            "--discover",
            "--audit-connections",
            "--connections-cache",
            sandbox
                .path()
                .join("connections.json")
                .to_str()
                .expect("UTF-8"),
            "--discovery-home",
            sandbox.path().to_str().expect("UTF-8"),
        ])
        .env("PATH", &bin)
        .output()
        .expect("acceptance CLI should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let audit: AcceptanceReport = serde_json::from_slice(&output.stdout).expect("audit JSON");
    assert!(audit.passed);
    assert_eq!(audit.connection_count, 1);
    assert_eq!(audit.command_count, 10);
}

#[test]
fn discover_cli_applies_an_explicit_template_override_file() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let bin = sandbox.path().join("bin");
    fs::create_dir_all(&bin).expect("mock bin should exist");
    write_executable(
        &bin.join("kubectl"),
        "#!/bin/sh\nprintf '%s\\n' '{\"contexts\":[{\"name\":\"overridden\",\"context\":{\"cluster\":\"fresh\",\"namespace\":\"payments\"}}],\"clusters\":[]}'\n",
    );
    let overrides = sandbox.path().join("templates.json");
    fs::write(
        &overrides,
        r#"{"version":1,"overrides":[{"provider":"kubernetes","resource_kind":"context","id":"connect","label":"Workloads","action":"connect","command":"kubectl --context {context} --namespace {namespace} get all","description":"Preview workloads"}]}"#,
    )
    .expect("override file");
    let cache = sandbox.path().join("connections.json");

    let output = Command::new(env!("CARGO_BIN_EXE_access-atlas"))
        .args([
            "--discover",
            "--template-overrides",
            overrides.to_str().expect("UTF-8"),
            "--connections-cache",
            cache.to_str().expect("UTF-8"),
            "--discovery-home",
            sandbox.path().to_str().expect("UTF-8"),
        ])
        .env("PATH", &bin)
        .output()
        .expect("discovery CLI should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let inventory: ConnectionInventory =
        serde_json::from_slice(&output.stdout).expect("inventory JSON");
    let command = &inventory.connections[0].commands[0];
    assert_eq!(command.label, "Workloads");
    assert_eq!(
        command.command,
        "kubectl --context overridden --namespace payments get all"
    );
}

fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("mock executable should be written");
    let mut permissions = fs::metadata(path)
        .expect("mock metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("mock should be executable");
}
