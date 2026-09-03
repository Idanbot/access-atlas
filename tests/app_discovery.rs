use access_atlas::{
    app::{App, RefreshState, ThemeId},
    discovery::{
        CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
        DiscoveryService,
    },
    model::Topology,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{io, path::PathBuf};

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
    assert_eq!(app.target().location.city, "Unlocated");
    assert_eq!(
        app.target().location.label,
        "Unlocated · anchored to origin"
    );
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
    assert!(app.take_refresh_request());
    assert!(!app.take_refresh_request());
    app.mark_refresh_started();
    assert_eq!(app.refresh_state(), &RefreshState::Running);
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
