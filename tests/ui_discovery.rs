use access_atlas::{
    app::{App, ThemeId},
    discovery::{
        CommandRequest, CommandResult, CommandRunner, DiscoveryConfig, DiscoveryMode,
        DiscoveryService,
    },
    model::Topology,
    render,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
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

fn text(buffer: &Buffer) -> String {
    let mut output = String::new();
    for y in buffer.area.top()..buffer.area.bottom() {
        for x in buffer.area.left()..buffer.area.right() {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}

fn render_app(app: &App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render::render(frame, app))
        .expect("render");
    text(terminal.backend().buffer())
}

#[test]
fn discovered_target_exposes_command_library_and_refresh_controls() {
    let output = render_app(&discovered_app());
    if std::env::var_os("ACCESS_ATLAS_SNAPSHOT").is_some() {
        println!("{output}");
    }

    assert!(output.contains("ENTER 10 COMMANDS"));
    assert!(output.contains("R REFRESH"));
    assert!(output.contains("Y COPY"));
}

#[test]
fn command_library_renders_ten_metadata_specific_templates() {
    let mut app = discovered_app();
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let output = render_app(&app);
    if std::env::var_os("ACCESS_ATLAS_SNAPSHOT").is_some() {
        println!("{output}");
    }

    assert!(output.contains("COMMAND LIBRARY [READ ONLY]"));
    assert!(output.contains("01/10"));
    assert!(output.contains("Cluster info"));
    assert!(output.contains("Debug workload"));
    assert!(output.contains("payments"));
    assert!(output.contains("Y COPY"));
}
