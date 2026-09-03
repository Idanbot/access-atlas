use access_atlas::discovery::{
    CancellationToken, CommandRequest, CommandResult, CommandRunner, ConnectionInventory,
    DiscoveryConfig, DiscoveryEvent, DiscoveryMode, DiscoveryService, Provider,
};
use std::{io, path::PathBuf, time::Duration};

#[derive(Clone)]
struct LocalFixture;

impl CommandRunner for LocalFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(
                r#"{"contexts":[{"name":"progress","context":{"cluster":"mock"}}],"clusters":[]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn refresh_emits_per_provider_progress_in_order() {
    let mut events = Vec::new();
    let report = DiscoveryService::new(
        LocalFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh_with_progress(DiscoveryMode::Local, &CancellationToken::new(), |event| {
        events.push(event)
    });

    assert_eq!(events.first(), Some(&DiscoveryEvent::Started { total: 9 }));
    assert!(matches!(
        events.get(1),
        Some(DiscoveryEvent::Source(source)) if source.provider == Provider::Kubernetes
    ));
    assert_eq!(
        events.last(),
        Some(&DiscoveryEvent::Finished {
            completed: 9,
            cancelled: false
        })
    );
    assert_eq!(report.sources.len(), 9);
    assert!(!report.cancelled);
}

#[test]
fn cancellation_stops_before_the_next_provider() {
    let token = CancellationToken::new();
    let callback_token = token.clone();
    let report = DiscoveryService::new(
        LocalFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh_with_progress(DiscoveryMode::Local, &token, move |event| {
        if matches!(event, DiscoveryEvent::Source(_)) {
            callback_token.cancel();
        }
    });

    assert!(report.cancelled);
    assert_eq!(report.sources.len(), 1);
    assert_eq!(report.sources[0].provider, Provider::Kubernetes);
}

#[test]
fn cache_expiry_and_online_reconciliation_remove_confirmed_stale_resources() {
    let mut cached = DiscoveryService::new(
        LocalFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    cached.generated_at_unix = 100;
    assert!(!cached.is_stale_at(150, Duration::from_secs(60)));
    assert!(cached.is_stale_at(161, Duration::from_secs(60)));

    let mut stale = cached.connections[0].clone();
    stale.id = "kubernetes:context:removed".to_owned();
    stale.label = "removed".to_owned();
    cached.connections.push(stale);
    let fresh_report = DiscoveryService::new(
        LocalFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Online);

    let reconciled = ConnectionInventory::reconcile(
        cached,
        fresh_report.inventory,
        &fresh_report.sources,
        DiscoveryMode::Online,
    );
    assert!(
        !reconciled
            .connections
            .iter()
            .any(|connection| connection.label == "removed")
    );

    let old_timestamp = reconciled.generated_at_unix;
    let failed_only = ConnectionInventory {
        schema_version: 1,
        generated_at_unix: old_timestamp + 100,
        connections: Vec::new(),
    };
    let retained =
        ConnectionInventory::reconcile(reconciled, failed_only, &[], DiscoveryMode::Online);
    assert_eq!(retained.generated_at_unix, old_timestamp);
}

#[derive(Clone)]
struct FailedAwsOnlineFixture;

impl CommandRunner for FailedAwsOnlineFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "aws" && request.args == ["configure", "list-profiles"] {
            return Ok(CommandResult::success("prod\n"));
        }
        if request.program == "aws" {
            return Ok(CommandResult {
                status: 1,
                stdout: String::new(),
                stderr: "access denied by fixture".to_owned(),
            });
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn online_query_failure_is_reported_without_discarding_local_profiles() {
    let report = DiscoveryService::new(
        FailedAwsOnlineFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Online);
    let aws = report
        .sources
        .iter()
        .find(|source| source.provider == Provider::Aws)
        .expect("AWS report");

    assert_eq!(aws.state, access_atlas::discovery::SourceState::Failed);
    assert!(aws.message.contains("access denied by fixture"));
    assert!(
        report
            .inventory
            .connections
            .iter()
            .any(|connection| connection.id == "aws:profile:prod")
    );
}
