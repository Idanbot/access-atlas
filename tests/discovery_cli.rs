use access_atlas::discovery::{ConnectionInventory, InventoryCache, Provider};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

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

fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("mock executable should be written");
    let mut permissions = fs::metadata(path)
        .expect("mock metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("mock should be executable");
}
