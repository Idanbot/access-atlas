use access_atlas::discovery::{
    CommandRequest, CommandResult, CommandRunner, ConnectionInventory, DiscoveryConfig,
    DiscoveryMode, DiscoveryService, InventoryCache,
};
use std::{io, path::PathBuf};

#[derive(Clone)]
struct KubectlFixture;

impl CommandRunner for KubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(
                r#"{"contexts":[{"name":"cached","context":{"cluster":"demo"}}],"clusters":[]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn fresh_inventory_replaces_matching_cache_entries_and_retains_remote_only_entries() {
    let mut cached = inventory("cached");
    cached.connections[0]
        .metadata
        .insert("version".into(), "old".into());
    let mut fresh = inventory("cached");
    fresh.connections[0]
        .metadata
        .insert("version".into(), "new".into());
    let remote = inventory("remote").connections.remove(0);
    cached.connections.push(remote);

    let merged = ConnectionInventory::merge(cached, fresh);

    assert_eq!(merged.connections.len(), 2);
    assert_eq!(merged.connections[0].metadata["version"], "new");
    assert!(
        merged
            .connections
            .iter()
            .any(|connection| connection.label == "remote")
    );
}

fn inventory(name: &str) -> ConnectionInventory {
    let mut inventory = DiscoveryService::new(
        NamedKubectlFixture(name.to_owned()),
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    inventory.generated_at_unix = if name == "cached" { 1 } else { 2 };
    inventory
}

#[derive(Clone)]
struct NamedKubectlFixture(String);

impl CommandRunner for NamedKubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(format!(
                r#"{{"contexts":[{{"name":"{}","context":{{"cluster":"demo"}}}}],"clusters":[]}}"#,
                self.0
            )));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn generated_inventory_cache_is_atomic_and_independent() {
    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let path = sandbox.path().join("cache/connections.json");
    let cache = InventoryCache::new(path.clone());
    assert!(
        cache
            .load_or_default()
            .expect("missing cache is valid")
            .connections
            .is_empty()
    );

    let inventory = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    cache.store(&inventory).expect("cache should be stored");
    let loaded = cache.load_or_default().expect("cache should load");

    assert_eq!(loaded, inventory);
    assert!(path.exists());
    assert!(!sandbox.path().join("data/demo-topology.json").exists());
    assert!(
        std::fs::read_dir(path.parent().expect("cache has parent"))
            .expect("cache directory should be readable")
            .all(|entry| !entry
                .expect("entry should be valid")
                .file_name()
                .to_string_lossy()
                .contains(".tmp"))
    );
}

#[test]
fn inventory_cache_is_owner_readable_only() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = tempfile::tempdir().expect("sandbox should exist");
    let path = sandbox.path().join("cache/connections.json");
    let cache = InventoryCache::new(path.clone());
    let inventory = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;

    cache.store(&inventory).expect("cache should be stored");

    let mode = std::fs::metadata(&path)
        .expect("cache should exist")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "fresh cache must not be group/world readable");

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("existing cache can be world-readable before upgrade");
    cache.load_or_default().expect("readable cache should load");
    let tightened = std::fs::metadata(&path)
        .expect("cache should exist")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        tightened, 0o600,
        "loading an existing cache should drop group/world bits"
    );
}
