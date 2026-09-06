use access_atlas::{
    app::{App, ThemeId},
    discovery::ConnectionInventory,
    modal::ConfirmChoice,
    model::Topology,
    render,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

const FIXTURE: &str = include_str!("../data/demo-topology.json");

fn prompt_app() -> App {
    App::with_inventory(
        Topology::from_json(FIXTURE).expect("topology"),
        ThemeId::CyberOrbital,
        ConnectionInventory::default(),
    )
    .with_load_prompt()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
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

fn render_app(app: &App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render::render(frame, app))
        .expect("render");
    buffer_text(terminal.backend().buffer())
}

#[test]
fn fresh_open_asks_to_load_or_skip_connections() {
    let mut app = prompt_app();
    assert!(app.load_prompt_open());
    assert_eq!(app.load_prompt_choice(), Some(ConfirmChoice::Approve));

    let output = render_app(&app);
    assert!(output.contains("LOAD CONNECTIONS"));
    assert!(output.contains("Load all connections"));
    assert!(output.contains("Skip for now"));
    assert!(!app.take_refresh_request());

    app.handle_key(key(KeyCode::Tab));
    assert_eq!(app.load_prompt_choice(), Some(ConfirmChoice::Decline));
    app.handle_key(key(KeyCode::Char('g')));
    assert!(app.load_prompt_open());
    assert!(!app.connection_browser_open());
}

#[test]
fn approving_the_prompt_fetches_all_connections() {
    let mut app = prompt_app();
    app.handle_key(key(KeyCode::Enter));

    assert!(!app.load_prompt_open());
    assert!(app.take_refresh_request());
    assert!(!app.take_refresh_request());
}

#[test]
fn skipping_leaves_cached_inventory_unfetched() {
    let mut app = prompt_app();
    app.handle_key(key(KeyCode::Tab));
    app.handle_key(key(KeyCode::Enter));

    assert!(!app.load_prompt_open());
    assert!(!app.take_refresh_request());
}

#[test]
fn load_and_skip_shortcuts_resolve_the_prompt() {
    let mut load = prompt_app();
    load.handle_key(key(KeyCode::Char('l')));
    assert!(!load.load_prompt_open());
    assert!(load.take_refresh_request());

    let mut skip = prompt_app();
    skip.handle_key(key(KeyCode::Char('s')));
    assert!(!skip.load_prompt_open());
    assert!(!skip.take_refresh_request());
}

#[test]
fn prompt_ignores_key_release_so_it_stays_controllable() {
    let mut app = prompt_app();
    app.handle_key(KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    });
    assert_eq!(app.load_prompt_choice(), Some(ConfirmChoice::Approve));
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.load_prompt_open());
    assert!(!app.take_refresh_request());
}
