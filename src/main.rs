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
    collections::BTreeMap,
    io::{self, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use access_atlas::{
    app::{App, ThemeId},
    clipboard::copy_text,
    discovery::{
        CancellationToken, CommandRequest, CommandRunner, ConnectionInventory, DiscoveryConfig,
        DiscoveryEvent, DiscoveryMode, DiscoveryService, InventoryCache, ProcessRunner,
        RefreshReport, RefreshScope, audit_refresh,
    },
    geo::Gazetteer,
    model::{AccessOption, Health, MatchStatus, NetworkType, Origin, Target, Topology},
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
    #[arg(
        long,
        value_name = "PATH",
        help = "Optional authored overlay; default live run is origin plus inventory"
    )]
    data: Option<PathBuf>,

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

    #[arg(
        long,
        help = "Start with the globe visible instead of the inventory pane"
    )]
    globe: bool,
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
        let report =
            DiscoveryService::new(ProcessRunner::new(Duration::from_secs(3)), discovery_config)
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

    if args.validate {
        let topology = load_topology(
            args.data
                .as_ref()
                .unwrap_or(&PathBuf::from("data/demo-topology.json")),
        )?;
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
        let topology = load_topology(
            args.data
                .as_ref()
                .unwrap_or(&PathBuf::from("data/demo-topology.json")),
        )?;
        return run_tui(
            App::with_inventory(topology, theme, ConnectionInventory::default())
                .with_discovery_enabled(false)
                .with_globe_visible(args.globe),
            None,
        );
    }

    let topology = if let Some(path) = &args.data {
        load_topology(path)?
    } else {
        origin_surface()?
    };
    let (discovery_config, inventory_cache) = discovery_setup(&args)?;
    let gazetteer = Gazetteer::load(
        &discovery_config
            .home
            .join(".config/access-atlas/locations.json"),
    );
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
        App::with_inventory(topology, theme, cached_inventory)
            .with_gazetteer(gazetteer)
            .with_globe_visible(args.globe),
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

fn origin_surface() -> Result<Topology> {
    let demo = Topology::from_json(EMBEDDED_DEMO).context("parse embedded origin")?;
    Ok(Topology {
        schema_version: 1,
        name: "Local access surface".to_owned(),
        generated_at: demo.generated_at,
        origin: demo.origin.clone(),
        targets: vec![workstation_target(&demo.origin)],
    })
}

fn workstation_target(origin: &Origin) -> Target {
    Target {
        id: origin.id.clone(),
        label: origin.label.clone(),
        kind: "workstation".to_owned(),
        provider: "local".to_owned(),
        location: origin.location.clone(),
        status: Health {
            state: "source".to_owned(),
            uptime_seconds: 0,
            latency_ms: 0.0,
            packet_loss_percent: 0.0,
            checked_at: "local".to_owned(),
            probed: false,
        },
        network: BTreeMap::new(),
        metadata: BTreeMap::from([("match".to_owned(), "source".to_owned())]),
        network_types: vec![NetworkType {
            id: "local".to_owned(),
            label: "Workstation".to_owned(),
            binary: "local".to_owned(),
            description: "This workstation is the access source.".to_owned(),
            access_options: vec![AccessOption {
                id: "source".to_owned(),
                label: "Source".to_owned(),
                command: "true".to_owned(),
                route: vec![origin.id.clone()],
                notes: "Local origin. Discovered connections attach here.".to_owned(),
            }],
        }],
        match_status: MatchStatus::Source,
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
        start_refresh(
            DiscoveryMode::Local,
            config.clone(),
            RefreshScope::default(),
            refresh_tx.clone(),
        )
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
        if let Some(scope) = app.take_online_scope() {
            if let Some((config, _)) = &discovery {
                app.mark_refresh_started();
                active_cancellation = Some(start_refresh(
                    DiscoveryMode::Online,
                    config.clone(),
                    scope,
                    refresh_tx.clone(),
                ));
            } else {
                app.mark_refresh_failed("Discovery is disabled by --demo-only");
            }
        }
        if app.take_probe_request() {
            apply_probe(app);
        }
        if app.take_cancel_request()
            && let Some(cancellation) = &active_cancellation
        {
            cancellation.cancel();
        }
        if let Some(command) = app.take_copy_request() {
            let copied = copy_text(&command);
            terminal
                .backend_mut()
                .write_all(copied.osc52.as_bytes())
                .context("copy command through terminal clipboard")?;
            terminal.backend_mut().flush().context("flush clipboard")?;
            let chars = command.chars().count();
            app.set_copy_notice(if copied.native {
                format!("copied {chars} chars")
            } else {
                format!("copied {chars} chars · native clipboard unsupported")
            });
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
    scope: RefreshScope,
    sender: Sender<RefreshMessage>,
) -> CancellationToken {
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    thread::spawn(move || {
        let report = DiscoveryService::new(ProcessRunner::new(Duration::from_secs(3)), config)
            .refresh_scoped(mode, &worker_cancellation, scope, |event| {
                let _ = sender.send(RefreshMessage::Progress(event));
            });
        let _ = sender.send(RefreshMessage::Complete { mode, report });
    });
    cancellation
}

fn apply_probe(app: &mut App) {
    let host = app
        .current_connection()
        .and_then(|connection| {
            connection
                .metadata
                .get("public_ip")
                .or_else(|| connection.metadata.get("hostname"))
                .or_else(|| connection.metadata.get("internal_ip"))
                .or_else(|| connection.metadata.get("ip"))
                .cloned()
        })
        .or_else(|| Some(app.target().location.city.clone()));
    let Some(host) = host.filter(|value| value != "No location") else {
        app.mark_probe_failed("no probe host");
        return;
    };
    let runner = ProcessRunner::new(Duration::from_secs(3));
    let request = CommandRequest {
        program: "ping".to_owned(),
        args: vec![
            "-c".to_owned(),
            "1".to_owned(),
            "-W".to_owned(),
            "2".to_owned(),
            host,
        ],
        current_dir: None,
    };
    match runner.run(&request) {
        Ok(result) if result.is_success() => {
            let latency = result
                .stdout
                .split("time=")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.trim_end_matches("ms").parse().ok())
                .unwrap_or(0.0);
            app.apply_probe(latency, 0.0);
        }
        Ok(_) => app.mark_probe_failed("probe lost"),
        Err(error) => app.mark_probe_failed(format!("probe failed: {error}")),
    }
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
