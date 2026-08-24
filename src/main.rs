use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{Event, poll, read},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, path::PathBuf, time::Instant};

use access_atlas::{
    app::{App, ThemeId},
    model::Topology,
    render,
};

const EMBEDDED_DEMO: &str = include_str!("../data/demo-topology.json");

#[derive(Debug, Parser)]
#[command(
    name = "access-atlas",
    version,
    about = "Animated read-only access topology demo"
)]
struct Args {
    #[arg(long, default_value = "data/demo-topology.json")]
    data: PathBuf,

    #[arg(long, help = "Parse and validate the topology without opening the TUI")]
    validate: bool,

    #[arg(
        long,
        help = "Color theme: cyber-orbital, tactical-radar, minimal-atlas, amber-crt, deep-space"
    )]
    theme: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let topology = load_topology(&args.data)?;

    if args.validate {
        println!(
            "validated {} targets and {} access options",
            topology.targets.len(),
            topology
                .targets
                .iter()
                .map(|target| {
                    target
                        .network_types
                        .iter()
                        .map(|network_type| network_type.access_options.len())
                        .sum::<usize>()
                })
                .sum::<usize>()
        );
        return Ok(());
    }

    let theme = match args.theme.as_deref() {
        Some("cyber-orbital") | Some("cyber") | Some("orbital") => ThemeId::CyberOrbital,
        Some("tactical-radar") | Some("radar") | Some("tactical") => ThemeId::TacticalRadar,
        Some("minimal-atlas") | Some("atlas") | Some("minimal") | Some("slate") => {
            ThemeId::MinimalAtlas
        }
        Some("amber-crt") | Some("amber") | Some("crt") => ThemeId::AmberCrt,
        Some("deep-space") | Some("space") | Some("nebula") => ThemeId::DeepSpace,
        _ => ThemeId::default(),
    };

    run_tui(App::with_theme(topology, theme))
}

fn load_topology(path: &PathBuf) -> Result<Topology> {
    match Topology::load(path) {
        Ok(topology) => Ok(topology),
        Err(error) if path == &PathBuf::from("data/demo-topology.json") => {
            Topology::from_json(EMBEDDED_DEMO).with_context(|| {
                format!(
                    "load embedded demo after reading {} failed: {error:#}",
                    path.display()
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode().context("enable raw terminal mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), Hide)
        .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode().context("disable raw terminal mode")?;
    execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
        .context("leave alternate screen")?;
    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    while !app.should_quit() {
        if app.needs_render() {
            terminal.draw(|frame| render::render(frame, app))?;
            app.mark_rendered();
        }
        let poll_timeout = if app.is_animating() {
            std::time::Duration::from_millis(40)
        } else {
            std::time::Duration::from_millis(80)
        };
        if poll(poll_timeout)?
            && let Event::Key(key) = read()?
        {
            app.handle_key(key);
        }
        let now = Instant::now();
        app.tick(now.duration_since(last_tick));
        last_tick = now;
    }
    Ok(())
}
