use access_atlas::discovery::{
    DiscoveryConfig, DiscoveryMode, DiscoveryService, ProcessRunner, Provider,
};
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

#[test]
fn process_runner_discovers_from_an_isolated_mock_path() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let bin = sandbox.path().join("bin");
    fs::create_dir_all(&bin).expect("mock bin should exist");
    let kubectl = bin.join("kubectl");
    fs::write(
        &kubectl,
        "#!/bin/sh\nprintf '%s\\n' '{\"contexts\":[{\"name\":\"fresh-install\",\"context\":{\"cluster\":\"mock\"}}],\"clusters\":[]}'\n",
    )
    .expect("mock kubectl should be written");
    let mut permissions = fs::metadata(&kubectl)
        .expect("mock metadata should load")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&kubectl, permissions).expect("mock should be executable");

    let runner = ProcessRunner::new(Duration::from_secs(2)).with_search_path(vec![bin]);
    let report = DiscoveryService::new(
        runner,
        DiscoveryConfig::new(sandbox.path().to_owned(), Vec::new()),
    )
    .refresh(DiscoveryMode::Local);

    assert!(report.inventory.connections.iter().any(|connection| {
        connection.provider == Provider::Kubernetes && connection.label == "fresh-install"
    }));
    assert_eq!(report.sources.len(), 9);
}
