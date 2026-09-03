use access_atlas::{
    app::{App, ThemeId},
    discovery::{
        CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
        DiscoveryService, Provider,
    },
    model::Topology,
    render,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::{io, path::PathBuf};

const FIXTURE: &str = include_str!("../data/demo-topology.json");

#[derive(Clone)]
struct MultiProviderFixture;

impl CommandRunner for MultiProviderFixture {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        match request.program.as_str() {
            "kubectl" => Ok(CommandResult::success(
                r#"{"contexts":[{"name":"prod-eu","context":{"cluster":"prod"}}],"clusters":[]}"#,
            )),
            "aws" => Ok(CommandResult::success("team-prod\n")),
            "docker" => Ok(CommandResult::success(
                r#"{"Name":"desktop-linux","DockerEndpoint":"unix:///tmp/docker.sock","Current":"true"}"#,
            )),
            _ => Err(io::Error::new(io::ErrorKind::NotFound, "not installed")),
        }
    }
}

fn app() -> App {
    let inventory = DiscoveryService::new(
        MultiProviderFixture,
        DiscoveryConfig::new(PathBuf::from("/tmp/missing"), Vec::new()),
    )
    .refresh(DiscoveryMode::Local)
    .inventory;
    App::with_inventory(
        Topology::from_json(FIXTURE).expect("topology"),
        ThemeId::CyberOrbital,
        inventory,
    )
}

#[test]
fn browser_filters_by_provider_searches_and_selects_connections() {
    let mut app = app();
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert!(app.connection_browser_open());
    assert_eq!(app.visible_connections().len(), 3);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.connection_provider_filter(), Some(Provider::Kubernetes));
    assert_eq!(app.visible_connections().len(), 1);

    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for character in "desktop".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    assert_eq!(app.visible_connections().len(), 1);
    assert_eq!(app.visible_connections()[0].label, "desktop-linux");
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.connection_browser_open());
    assert_eq!(app.target().label, "desktop-linux");
}

#[test]
fn inventory_deduplication_keeps_one_stable_connection_id() {
    let mut inventory = app().inventory().clone();
    inventory.connections.push(inventory.connections[0].clone());

    inventory.deduplicate();

    let unique = inventory
        .connections
        .iter()
        .filter(|connection| connection.id == inventory.connections[0].id)
        .count();
    assert_eq!(unique, 1);
}

#[test]
fn browser_render_groups_provider_rows_under_an_unlocated_section() {
    let mut app = app();
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render::render(frame, &app))
        .expect("render");
    let output = buffer_text(terminal.backend().buffer());
    if std::env::var_os("ACCESS_ATLAS_SNAPSHOT").is_some() {
        println!("{output}");
    }

    assert!(output.contains("CONNECTION BROWSER"));
    assert!(output.contains("UNLOCATED · KUBERNETES"));
    assert!(output.contains("UNLOCATED · AWS"));
    assert!(output.contains("desktop-linux"));
}

fn buffer_text(buffer: &Buffer) -> String {
    let mut output = String::new();
    for y in buffer.area.top()..buffer.area.bottom() {
        for x in buffer.area.left()..buffer.area.right() {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}
