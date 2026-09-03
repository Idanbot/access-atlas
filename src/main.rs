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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use access_atlas::{
    app::{App, ThemeId},
    clipboard::osc52_sequence,
    discovery::{
        CancellationToken, ConnectionInventory, DiscoveryConfig, DiscoveryEvent, DiscoveryMode,
        DiscoveryService, InventoryCache, ProcessRunner, RefreshReport, audit_refresh,
    },
    model::Topology,
    render,
};

const EMBEDDED_DEMO: &str = include_str!("../data/demo-topology.json");

#[derive(Debug, Parser)]
#[command(
    name = "access-atlas",
    version,
    about = "Local-first infrastructure access map and command catalog"
)]
struct Args {
    #[arg(long, default_value = "data/demo-topology.json")]
    data: PathBuf,

    #[arg(long, help = "Parse and validate the topology without opening the TUI")]
    validate: bool,

    #[arg(
        long,
        conflicts_with_all = [
            "discover",
            "connections_cache",
            "discovery_home",
            "terraform_root",
            "template_overrides"
        ],
        help = "Open the topology without reading cached connections or scanning local tools"
    )]
    demo_only: bool,

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
        requires = "discover",
        help = "Audit discovered metadata and templates without executing generated commands"
    )]
    audit_connections: bool,

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

    #[arg(
        long,
        value_name = "PATH",
        help = "Versioned JSON file that replaces or inserts typed command templates"
    )]
    template_overrides: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = 24,
        value_name = "HOURS",
        help = "Discard generated connection caches older than this; zero disables cache loading"
    )]
    cache_max_age_hours: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.discover {
        let (discovery_config, inventory_cache) = discovery_setup(&args)?;
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
        if args.audit_connections {
            let audit = audit_refresh(&report);
            println!(
                "{}",
                serde_json::to_string_pretty(&audit).context("serialize acceptance audit")?
            );
            if !audit.passed {
                anyhow::bail!("connection acceptance audit failed");
            }
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&report.inventory)
                    .context("serialize discovered connections")?
            );
        }
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

    if args.demo_only {
        return run_tui(
            App::with_inventory(topology, theme, ConnectionInventory::default())
                .with_discovery_enabled(false),
            None,
        );
    }

    let (discovery_config, inventory_cache) = discovery_setup(&args)?;
    let mut cached_inventory = inventory_cache.load_or_default().unwrap_or_else(|error| {
        eprintln!(
            "warning: ignored connection cache {}: {error}",
            inventory_cache.path().display()
        );
        ConnectionInventory::default()
    });
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let cache_age = Duration::from_secs(args.cache_max_age_hours.saturating_mul(3_600));
    if (args.cache_max_age_hours == 0 && !cached_inventory.connections.is_empty())
        || cached_inventory.is_stale_at(now_unix, cache_age)
    {
        eprintln!(
            "warning: ignored stale connection cache {}",
            inventory_cache.path().display()
        );
        cached_inventory = ConnectionInventory::default();
    }
    run_tui(
        App::with_inventory(topology, theme, cached_inventory),
        Some((discovery_config, inventory_cache)),
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
    let template_overrides = args
        .template_overrides
        .clone()
        .unwrap_or_else(|| home.join(".config/access-atlas/templates.json"));
    Ok((
        DiscoveryConfig::new(home, terraform_roots).with_template_overrides(template_overrides),
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

fn run_tui(mut app: App, discovery: Option<(DiscoveryConfig, InventoryCache)>) -> Result<()> {
    enable_raw_mode().context("enable raw terminal mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), Hide)
        .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let result = event_loop(&mut terminal, &mut app, discovery);

    disable_raw_mode().context("disable raw terminal mode")?;
    execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
        .context("leave alternate screen")?;
    result
}

enum RefreshMessage {
    Progress(DiscoveryEvent),
    Complete {
        mode: DiscoveryMode,
        report: RefreshReport,
    },
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    discovery: Option<(DiscoveryConfig, InventoryCache)>,
) -> Result<()> {
    let (refresh_tx, refresh_rx) = mpsc::channel();
    let mut active_cancellation = discovery.as_ref().map(|(config, _)| {
        app.mark_refresh_started();
        start_refresh(DiscoveryMode::Local, config.clone(), refresh_tx.clone())
    });
    let mut last_tick = Instant::now();
    while !app.should_quit() {
        if let Some((_, cache)) = &discovery
            && receive_refresh(app, cache, &refresh_rx)
        {
            active_cancellation = None;
        }
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
            if let Some((config, _)) = &discovery {
                app.mark_refresh_started();
                active_cancellation = Some(start_refresh(
                    DiscoveryMode::Online,
                    config.clone(),
                    refresh_tx.clone(),
                ));
            } else {
                app.mark_refresh_failed("Discovery is disabled by --demo-only");
            }
        }
        if app.take_cancel_request()
            && let Some(cancellation) = &active_cancellation
        {
            cancellation.cancel();
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

fn start_refresh(
    mode: DiscoveryMode,
    config: DiscoveryConfig,
    sender: Sender<RefreshMessage>,
) -> CancellationToken {
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    thread::spawn(move || {
        let report = DiscoveryService::new(
            ProcessRunner::new(std::time::Duration::from_secs(8)),
            config,
        )
        .refresh_with_progress(mode, &worker_cancellation, |event| {
            let _ = sender.send(RefreshMessage::Progress(event));
        });
        let _ = sender.send(RefreshMessage::Complete { mode, report });
    });
    cancellation
}

fn receive_refresh(
    app: &mut App,
    cache: &InventoryCache,
    receiver: &Receiver<RefreshMessage>,
) -> bool {
    let mut completed = false;
    while let Ok(message) = receiver.try_recv() {
        match message {
            RefreshMessage::Progress(event) => app.apply_discovery_event(event),
            RefreshMessage::Complete { mode, mut report } => {
                report.inventory = ConnectionInventory::reconcile(
                    app.inventory().clone(),
                    report.inventory,
                    &report.sources,
                    mode,
                );
                if let Err(error) = cache.store(&report.inventory) {
                    app.apply_refresh(report);
                    app.mark_refresh_failed(format!("cache write failed: {error}"));
                } else {
                    app.apply_refresh(report);
                }
                completed = true;
            }
        }
    }
    completed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_only_arguments_parse() {
        let args = Args::try_parse_from(["access-atlas", "--demo-only"])
            .expect("demo-only arguments should parse");

        assert!(args.demo_only);
        assert!(!args.discover);
    }

    #[test]
    fn demo_only_disables_the_refresh_shortcut() {
        let topology = Topology::from_json(EMBEDDED_DEMO).expect("embedded topology should parse");
        let mut app =
            App::with_inventory(topology, ThemeId::default(), ConnectionInventory::default())
                .with_discovery_enabled(false);
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('R'),
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(!app.discovery_enabled());
        assert!(!app.take_refresh_request());
    }

    #[test]
    fn demo_only_rejects_discovery_configuration() {
        for arguments in [
            vec!["access-atlas", "--demo-only", "--discover"],
            vec![
                "access-atlas",
                "--demo-only",
                "--connections-cache",
                "connections.json",
            ],
        ] {
            assert!(Args::try_parse_from(arguments).is_err());
        }
    }
}
