use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{poll, read},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Instant,
};

use access_atlas::{
    app::{App, ThemeId},
    clipboard::osc52_sequence,
    discovery::{
        ConnectionInventory, DiscoveryConfig, DiscoveryMode, DiscoveryService, InventoryCache,
        ProcessRunner, RefreshReport,
    },
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

    #[arg(
        long,
        help = "Discover local CLI connections, write the cache, and print JSON"
    )]
    discover: bool,

    #[arg(
        long,
        requires = "discover",
        help = "Allow discovery to query remote provider APIs"
    )]
    online: bool,

    #[arg(
        long,
        value_name = "PATH",
        help = "Override the generated connection cache path"
    )]
    connections_cache: Option<PathBuf>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Home directory used for local connection discovery"
    )]
    discovery_home: Option<PathBuf>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Explicit Terraform root to inspect; may be repeated"
    )]
    terraform_root: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (discovery_config, inventory_cache) = discovery_setup(&args)?;
    if args.discover {
        let mode = if args.online {
            DiscoveryMode::Online
        } else {
            DiscoveryMode::Local
        };
        let report = DiscoveryService::new(
            ProcessRunner::new(std::time::Duration::from_secs(8)),
            discovery_config,
        )
        .refresh(mode);
        inventory_cache.store(&report.inventory).with_context(|| {
            format!(
                "write connection cache to {}",
                inventory_cache.path().display()
            )
        })?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report.inventory)
                .context("serialize discovered connections")?
        );
        return Ok(());
    }

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

    let cached_inventory = inventory_cache.load_or_default().unwrap_or_else(|error| {
        eprintln!(
            "warning: ignored connection cache {}: {error}",
            inventory_cache.path().display()
        );
        ConnectionInventory::default()
    });
    run_tui(
        App::with_inventory(topology, theme, cached_inventory),
        discovery_config,
        inventory_cache,
    )
}

fn discovery_setup(args: &Args) -> Result<(DiscoveryConfig, InventoryCache)> {
    let home = args
        .discovery_home
        .clone()
        .or_else(|| std::env::var_os("ACCESS_ATLAS_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .context("determine discovery home; use --discovery-home")?;
    let terraform_roots = if args.terraform_root.is_empty() {
        vec![std::env::current_dir().context("determine current Terraform root")?]
    } else {
        args.terraform_root.clone()
    };
    let cache_path = args
        .connections_cache
        .clone()
        .or_else(|| std::env::var_os("ACCESS_ATLAS_CACHE").map(PathBuf::from))
        .unwrap_or_else(|| {
            std::env::var_os("XDG_CACHE_HOME").map_or_else(
                || home.join(".cache/access-atlas/connections.json"),
                |root| PathBuf::from(root).join("access-atlas/connections.json"),
            )
        });
    Ok((
        DiscoveryConfig::new(home, terraform_roots),
        InventoryCache::new(cache_path),
    ))
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

fn run_tui(
    mut app: App,
    discovery_config: DiscoveryConfig,
    inventory_cache: InventoryCache,
) -> Result<()> {
    enable_raw_mode().context("enable raw terminal mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), Hide)
        .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let result = event_loop(&mut terminal, &mut app, discovery_config, inventory_cache);

    disable_raw_mode().context("disable raw terminal mode")?;
    execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
        .context("leave alternate screen")?;
    result
}

type RefreshResult = std::result::Result<RefreshReport, String>;

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    discovery_config: DiscoveryConfig,
    inventory_cache: InventoryCache,
) -> Result<()> {
    let (refresh_tx, refresh_rx) = mpsc::channel();
    start_refresh(
        DiscoveryMode::Local,
        discovery_config.clone(),
        refresh_tx.clone(),
    );
    app.mark_refresh_started();
    let mut last_tick = Instant::now();
    while !app.should_quit() {
        receive_refresh(app, &inventory_cache, &refresh_rx);
        if app.needs_render() {
            terminal.draw(|frame| render::render(frame, app))?;
            app.mark_rendered();
        }
        let poll_timeout = if app.is_animating() {
            std::time::Duration::from_millis(40)
        } else {
            std::time::Duration::from_millis(80)
        };
        if poll(poll_timeout)? {
            app.handle_event(read()?);
        }
        if app.take_refresh_request() {
            app.mark_refresh_started();
            start_refresh(
                DiscoveryMode::Online,
                discovery_config.clone(),
                refresh_tx.clone(),
            );
        }
        if let Some(command) = app.take_copy_request() {
            terminal
                .backend_mut()
                .write_all(osc52_sequence(&command).as_bytes())
                .context("copy command through terminal clipboard")?;
            terminal.backend_mut().flush().context("flush clipboard")?;
        }
        let now = Instant::now();
        app.tick(now.duration_since(last_tick));
        last_tick = now;
    }
    Ok(())
}

fn start_refresh(mode: DiscoveryMode, config: DiscoveryConfig, sender: Sender<RefreshResult>) {
    thread::spawn(move || {
        let report = DiscoveryService::new(
            ProcessRunner::new(std::time::Duration::from_secs(8)),
            config,
        )
        .refresh(mode);
        let _ = sender.send(Ok(report));
    });
}

fn receive_refresh(app: &mut App, cache: &InventoryCache, receiver: &Receiver<RefreshResult>) {
    let Ok(result) = receiver.try_recv() else {
        return;
    };
    match result {
        Ok(mut report) => {
            report.inventory =
                ConnectionInventory::merge(app.inventory().clone(), report.inventory);
            if let Err(error) = cache.store(&report.inventory) {
                app.mark_refresh_failed(format!("cache write failed: {error}"));
            } else {
                app.apply_refresh(report);
            }
        }
        Err(error) => app.mark_refresh_failed(error),
    }
}
