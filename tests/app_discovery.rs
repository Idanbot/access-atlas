use access_atlas::{
    app::{App, RefreshState, ThemeId},
    discovery::{
        CommandRequest, CommandResult, CommandRunner, ConnectionInventory, DiscoveredConnection,
        DiscoveryConfig, DiscoveryEvent, DiscoveryMode, DiscoveryService, Provider, SourceReport,
        SourceState,
    },
    model::Topology,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{collections::BTreeMap, io, path::PathBuf};
const FIXTURE: &str = include_str!("../data/demo-topology.json");

#[derive(Clone)]
struct KubectlFixture;

impl CommandRunner for KubectlFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        if request.program == "kubectl" {
            return Ok(CommandResult::success(
                r#"{"contexts":[{"name":"prod-eu","context":{"cluster":"prod","namespace":"payments"}}],"clusters":[]}"#,
            ));
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "not installed"))
    }
}

#[test]
fn discovered_connection_uses_primary_commands_as_tab_actions() {
    let inventory = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    let topology = Topology::from_json(FIXTURE).expect("fixture should load");
    let mut app = App::with_inventory(topology, ThemeId::CyberOrbital, inventory);

    app.previous_target();
    assert_eq!(app.target().label, "prod-eu");
    assert_eq!(app.target().provider, "kubernetes");
    assert_eq!(app.target().location.city, "No location");
    assert_eq!(app.target().location.label, "No location · source unknown");
    assert_eq!(app.current_network_type().access_options.len(), 3);
    assert_eq!(app.current_access_option().label, "Cluster info");
    app.next_access_option();
    assert_eq!(app.current_access_option().label, "Port-forward service");
    app.next_access_option();
    assert_eq!(app.current_access_option().label, "Debug workload");
    app.next_access_option();
    assert_eq!(app.current_access_option().label, "Cluster info");
    app.previous_access_option();
    assert_eq!(app.current_access_option().label, "Debug workload");
}

fn target_from_metadata(metadata: BTreeMap<String, String>) -> access_atlas::model::Target {
    let inventory = ConnectionInventory {
        schema_version: 1,
        generated_at_unix: 1,
        connections: vec![DiscoveredConnection {
            id: "aws:ec2:probe".to_owned(),
            label: "probe".to_owned(),
            provider: Provider::Aws,
            kind: "ec2-instance".to_owned(),
            metadata,
            commands: Vec::new(),
        }],
    };
    let topology = Topology::from_json(FIXTURE).expect("fixture should load");
    let mut app = App::with_inventory(topology, ThemeId::CyberOrbital, inventory);
    app.previous_target();
    app.target().clone()
}

#[test]
fn mapped_provider_regions_locate_and_unknown_regions_stay_unlocated() {
    let east = target_from_metadata(BTreeMap::from([(
        "region".to_owned(),
        "us-east-1".to_owned(),
    )]));
    assert_eq!(east.location.city, "Ashburn");
    assert_eq!(east.location.precision, "estimated-region");
    assert!((east.location.latitude - 39.04).abs() < 0.01);
    assert!((east.location.longitude - -77.49).abs() < 0.01);

    let zone = target_from_metadata(BTreeMap::from([(
        "zone".to_owned(),
        "europe-west4-a".to_owned(),
    )]));
    assert_eq!(zone.location.city, "Amsterdam");

    let west = target_from_metadata(BTreeMap::from([(
        "region".to_owned(),
        "us-west-2".to_owned(),
    )]));
    assert_eq!(west.location.city, "Oregon");
    assert_eq!(west.location.precision, "estimated-region");
    assert_eq!(zone.location.precision, "estimated-region");

    let unknown = target_from_metadata(BTreeMap::from([(
        "region".to_owned(),
        "not-a-region".to_owned(),
    )]));
    assert_eq!(unknown.location.city, "No location");
    assert_eq!(unknown.location.precision, "none");
    assert_eq!(unknown.location.label, "No location · source unknown");
    assert_eq!(unknown.location.latitude, 0.0);
    assert_eq!(unknown.location.longitude, 0.0);
}

fn discovered_app() -> App {
    let inventory = DiscoveryService::new(
        KubectlFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    let topology = Topology::from_json(FIXTURE).expect("fixture should load");
    let mut app = App::with_inventory(topology, ThemeId::CyberOrbital, inventory);
    app.previous_target();
    app
}

#[test]
fn enter_opens_searchable_top_ten_and_y_copies_without_executing() {
    let mut app = discovered_app();

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.command_library_open());
    assert_eq!(app.extended_commands().len(), 10);
    assert_eq!(
        app.selected_extended_command().unwrap().label,
        "Cluster info"
    );

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(
        app.selected_extended_command().unwrap().label,
        "Port-forward service"
    );
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert!(app.take_copy_request().unwrap().contains("port-forward"));
    assert!(app.take_copy_request().is_none());

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.command_library_open());
}

#[test]
fn slash_filters_the_command_library_without_changing_the_top_ten() {
    let mut app = discovered_app();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for character in "logs".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }

    assert_eq!(app.command_filter(), "logs");
    assert!(!app.visible_extended_commands().is_empty());
    assert!(app.visible_extended_commands().iter().all(|(_, command)| {
        format!("{} {}", command.label, command.command)
            .to_lowercase()
            .contains("logs")
    }));
    assert_eq!(app.extended_commands().len(), 10);

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.command_library_open());
    assert!(app.command_filter().is_empty());
}

#[test]
fn uppercase_r_requests_online_refresh_once() {
    let mut app = discovered_app();

    app.handle_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));
    let scope = app.take_online_scope().expect("scoped online refresh");
    assert_eq!(scope.providers, Some(vec![Provider::Kubernetes]));
    assert!(app.take_online_scope().is_none());
    app.mark_refresh_started();
    assert_eq!(app.refresh_state(), &RefreshState::Running);
}

#[test]
fn refresh_progress_and_cancel_request_are_visible_to_the_event_loop() {
    let mut app = discovered_app();
    app.mark_refresh_started();
    app.apply_discovery_event(DiscoveryEvent::Started { total: 9 });
    app.apply_discovery_event(DiscoveryEvent::Source(SourceReport {
        provider: Provider::Kubernetes,
        state: SourceState::Loaded,
        connections: 1,
        message: "context loaded".to_owned(),
    }));

    assert_eq!(app.refresh_progress(), (1, 9));
    assert_eq!(app.source_reports()[0].provider, Provider::Kubernetes);

    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert!(app.connection_browser_open());
    app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT));
    assert!(app.take_cancel_request());
    assert_eq!(app.refresh_state(), &RefreshState::Cancelling);
    assert!(app.connection_browser_open());
}

#[test]
fn applying_inventory_replaces_generated_targets_and_retains_selection() {
    let mut app = discovered_app();
    let inventory = app.inventory().clone();
    let static_targets = app.topology().targets.len() - inventory.connections.len();

    app.apply_inventory(inventory.clone());

    assert_eq!(
        app.topology().targets.len(),
        static_targets + inventory.connections.len()
    );
    assert_eq!(app.target().label, "prod-eu");
}
