use crate::{
    discovery::{
        CommandTemplate, ConnectionInventory, DiscoveredConnection, DiscoveryEvent, Provider,
        RefreshReport, SourceReport, SourceState,
    },
    model::{AccessOption, DetailRow, Health, Location, NetworkType, Target, Topology},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::{PI, TAU},
    time::Duration,
};

const AMBIENT_FRAME_MS: u128 = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeId {
    #[default]
    CyberOrbital,
    TacticalRadar,
    MinimalAtlas,
    AmberCrt,
    DeepSpace,
}

impl ThemeId {
    pub fn next(self) -> Self {
        match self {
            Self::CyberOrbital => Self::TacticalRadar,
            Self::TacticalRadar => Self::MinimalAtlas,
            Self::MinimalAtlas => Self::AmberCrt,
            Self::AmberCrt => Self::DeepSpace,
            Self::DeepSpace => Self::CyberOrbital,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::CyberOrbital => "Orbital Ice",
            Self::TacticalRadar => "Tactical Radar (P31)",
            Self::MinimalAtlas => "Minimal Slate Atlas",
            Self::AmberCrt => "Amber CRT Phosphor",
            Self::DeepSpace => "Deep Space Nebula",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CameraTransition {
    pub start_rotation: f64,
    pub target_rotation: f64,
    pub start_pitch: f64,
    pub target_pitch: f64,
    pub start_zoom: f64,
    pub target_zoom: f64,
    pub elapsed: Duration,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RefreshState {
    #[default]
    Idle,
    Running,
    Cancelling,
    Cancelled {
        completed: usize,
    },
    Complete {
        loaded: usize,
        failed: usize,
    },
    Failed(String),
}

pub struct App {
    topology: Topology,
    base_target_count: usize,
    inventory: ConnectionInventory,
    source_reports: Vec<SourceReport>,
    target_index: usize,
    network_type_index: usize,
    access_option_index: usize,
    detail_index: usize,
    elapsed: Duration,
    continuous_time: Duration,
    route_elapsed: Duration,
    route_progress: f32,
    rotation: f64,
    focus_rotation: f64,
    pitch: f64,
    focus_pitch: f64,
    zoom: f64,
    focus_zoom: f64,
    transition: Option<CameraTransition>,
    paused: bool,
    theme: ThemeId,
    dirty: bool,
    quit: bool,
    command_library_open: bool,
    command_library_index: usize,
    command_filter: String,
    command_search_active: bool,
    connection_browser_open: bool,
    connection_browser_index: usize,
    connection_provider_filter: Option<Provider>,
    connection_query: String,
    connection_search_active: bool,
    discovery_enabled: bool,
    refresh_requested: bool,
    cancel_requested: bool,
    refresh_state: RefreshState,
    refresh_completed: usize,
    refresh_total: usize,
    refresh_notices: Vec<String>,
    copy_request: Option<String>,
}

impl App {
    pub fn new(topology: Topology) -> Self {
        Self::with_theme(topology, ThemeId::CyberOrbital)
    }

    pub fn with_theme(topology: Topology, theme: ThemeId) -> Self {
        Self::with_inventory(topology, theme, ConnectionInventory::default())
    }

    pub fn with_inventory(
        mut topology: Topology,
        theme: ThemeId,
        mut inventory: ConnectionInventory,
    ) -> Self {
        inventory.deduplicate();
        let base_target_count = topology.targets.len();
        let fallback_location = topology.origin.location.clone();
        topology
            .targets
            .extend(inventory.connections.iter().map(|connection| {
                connection_target(connection, &fallback_location, inventory.generated_at_unix)
            }));
        let focus_rotation = target_focus_rotation(&topology.targets[0]);
        let focus_pitch = target_focus_pitch(&topology.targets[0]);
        let focus_zoom = target_focus_zoom(&topology.targets[0]);
        Self {
            topology,
            base_target_count,
            inventory,
            source_reports: Vec::new(),
            target_index: 0,
            network_type_index: 0,
            access_option_index: 0,
            detail_index: 0,
            elapsed: Duration::ZERO,
            continuous_time: Duration::ZERO,
            route_elapsed: Duration::ZERO,
            route_progress: 0.0,
            rotation: focus_rotation,
            focus_rotation,
            pitch: focus_pitch,
            focus_pitch,
            zoom: focus_zoom,
            focus_zoom,
            transition: None,
            paused: true,
            theme,
            dirty: true,
            quit: false,
            command_library_open: false,
            command_library_index: 0,
            command_filter: String::new(),
            command_search_active: false,
            connection_browser_open: false,
            connection_browser_index: 0,
            connection_provider_filter: None,
            connection_query: String::new(),
            connection_search_active: false,
            discovery_enabled: true,
            refresh_requested: false,
            cancel_requested: false,
            refresh_state: RefreshState::Idle,
            refresh_completed: 0,
            refresh_total: 9,
            refresh_notices: Vec::new(),
            copy_request: None,
        }
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    pub fn inventory(&self) -> &ConnectionInventory {
        &self.inventory
    }

    pub fn with_discovery_enabled(mut self, enabled: bool) -> Self {
        self.discovery_enabled = enabled;
        self.dirty = true;
        self
    }

    pub fn discovery_enabled(&self) -> bool {
        self.discovery_enabled
    }

    pub fn source_reports(&self) -> &[SourceReport] {
        &self.source_reports
    }

    pub fn current_source_report(&self) -> Option<&SourceReport> {
        let provider = self.current_connection()?.provider;
        self.source_reports
            .iter()
            .find(|source| source.provider == provider)
    }

    pub fn current_connection(&self) -> Option<&DiscoveredConnection> {
        let id = self.target().id.strip_prefix("discovered:")?;
        self.inventory
            .connections
            .iter()
            .find(|connection| connection.id == id)
    }

    pub fn extended_commands(&self) -> &[CommandTemplate] {
        self.current_connection()
            .map_or(&[], |connection| connection.commands.as_slice())
    }

    pub fn selected_extended_command(&self) -> Option<&CommandTemplate> {
        self.extended_commands().get(self.command_library_index)
    }

    pub fn command_filter(&self) -> &str {
        &self.command_filter
    }

    pub fn command_search_active(&self) -> bool {
        self.command_search_active
    }

    pub fn visible_extended_commands(&self) -> Vec<(usize, &CommandTemplate)> {
        let needle = self.command_filter.to_lowercase();
        self.extended_commands()
            .iter()
            .enumerate()
            .filter(|(_, command)| {
                needle.is_empty()
                    || format!(
                        "{} {:?} {} {}",
                        command.label, command.kind, command.command, command.description
                    )
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect()
    }

    pub fn command_library_open(&self) -> bool {
        self.command_library_open
    }

    pub fn command_library_index(&self) -> usize {
        self.command_library_index
    }

    pub fn connection_browser_open(&self) -> bool {
        self.connection_browser_open
    }

    pub fn connection_browser_index(&self) -> usize {
        self.connection_browser_index
    }

    pub fn connection_provider_filter(&self) -> Option<Provider> {
        self.connection_provider_filter
    }

    pub fn connection_query(&self) -> &str {
        &self.connection_query
    }

    pub fn connection_search_active(&self) -> bool {
        self.connection_search_active
    }

    pub fn visible_connections(&self) -> Vec<&DiscoveredConnection> {
        let needle = self.connection_query.to_lowercase();
        let mut connections = self
            .inventory
            .connections
            .iter()
            .filter(|connection| {
                self.connection_provider_filter
                    .is_none_or(|provider| connection.provider == provider)
            })
            .filter(|connection| {
                needle.is_empty()
                    || format!(
                        "{} {} {} {}",
                        connection.label,
                        connection.provider.as_str(),
                        connection.kind,
                        connection
                            .metadata
                            .values()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect::<Vec<_>>();
        connections.sort_by(|left, right| {
            (
                !self.connection_is_located(left),
                left.provider,
                &left.label,
            )
                .cmp(&(
                    !self.connection_is_located(right),
                    right.provider,
                    &right.label,
                ))
        });
        connections
    }

    pub fn connection_is_located(&self, connection: &DiscoveredConnection) -> bool {
        let target_id = format!("discovered:{}", connection.id);
        self.topology
            .targets
            .iter()
            .find(|target| target.id == target_id)
            .is_some_and(|target| target.location.precision != "unknown")
    }

    pub fn refresh_state(&self) -> &RefreshState {
        &self.refresh_state
    }

    pub fn refresh_progress(&self) -> (usize, usize) {
        (self.refresh_completed, self.refresh_total)
    }

    pub fn refresh_notices(&self) -> &[String] {
        &self.refresh_notices
    }

    pub fn take_refresh_request(&mut self) -> bool {
        std::mem::take(&mut self.refresh_requested)
    }

    pub fn mark_refresh_started(&mut self) {
        self.refresh_state = RefreshState::Running;
        self.refresh_completed = 0;
        self.refresh_total = 9;
        self.source_reports.clear();
        self.refresh_notices.clear();
        self.dirty = true;
    }

    pub fn apply_discovery_event(&mut self, event: DiscoveryEvent) {
        match event {
            DiscoveryEvent::Started { total } => {
                self.refresh_total = total;
                self.refresh_completed = 0;
            }
            DiscoveryEvent::Source(source) => {
                if let Some(existing) = self
                    .source_reports
                    .iter_mut()
                    .find(|existing| existing.provider == source.provider)
                {
                    *existing = source;
                } else {
                    self.source_reports.push(source);
                }
                self.refresh_completed = self.source_reports.len();
            }
            DiscoveryEvent::Finished {
                completed,
                cancelled,
            } => {
                self.refresh_completed = completed;
                if cancelled {
                    self.refresh_state = RefreshState::Cancelled { completed };
                }
            }
        }
        self.dirty = true;
    }

    pub fn mark_refresh_failed(&mut self, message: impl Into<String>) {
        self.refresh_state = RefreshState::Failed(message.into());
        self.dirty = true;
    }

    pub fn take_copy_request(&mut self) -> Option<String> {
        self.copy_request.take()
    }

    pub fn take_cancel_request(&mut self) -> bool {
        std::mem::take(&mut self.cancel_requested)
    }

    pub fn apply_refresh(&mut self, report: RefreshReport) {
        let loaded = report
            .sources
            .iter()
            .filter(|source| source.state == SourceState::Loaded)
            .count();
        let failed = report
            .sources
            .iter()
            .filter(|source| source.state == SourceState::Failed)
            .count();
        self.source_reports = report.sources;
        self.refresh_notices = report.notices;
        self.apply_inventory(report.inventory);
        self.refresh_state = if report.cancelled {
            RefreshState::Cancelled {
                completed: self.refresh_completed,
            }
        } else {
            RefreshState::Complete { loaded, failed }
        };
        self.dirty = true;
    }

    pub fn apply_inventory(&mut self, mut inventory: ConnectionInventory) {
        inventory.deduplicate();
        let selected_id = self.target().id.clone();
        let fallback_location = self.topology.origin.location.clone();
        self.topology.targets.truncate(self.base_target_count);
        self.topology
            .targets
            .extend(inventory.connections.iter().map(|connection| {
                connection_target(connection, &fallback_location, inventory.generated_at_unix)
            }));
        self.inventory = inventory;
        self.target_index = self
            .topology
            .targets
            .iter()
            .position(|target| target.id == selected_id)
            .unwrap_or(0);
        self.command_library_open = false;
        self.command_library_index = 0;
        self.command_filter.clear();
        self.command_search_active = false;
        self.connection_browser_index = self
            .connection_browser_index
            .min(self.visible_connections().len().saturating_sub(1));
        self.reset_target_animation();
        self.update_camera_focus();
    }

    pub fn target(&self) -> &Target {
        &self.topology.targets[self.target_index]
    }

    pub fn target_index(&self) -> usize {
        self.target_index
    }

    pub fn network_type_index(&self) -> usize {
        self.network_type_index
    }

    pub fn access_option_index(&self) -> usize {
        self.access_option_index
    }

    pub fn current_network_type(&self) -> &NetworkType {
        &self.target().network_types[self.network_type_index]
    }

    pub fn current_access_option(&self) -> &AccessOption {
        &self.current_network_type().access_options[self.access_option_index]
    }

    pub fn detail_index(&self) -> usize {
        self.detail_index
    }

    pub fn detail_rows(&self) -> Vec<DetailRow> {
        let mut rows = self.target().detail_rows();
        let network_type = self.current_network_type();
        let option = self.current_access_option();
        rows.push(DetailRow {
            label: "access.network_type".to_owned(),
            value: network_type.label.clone(),
        });
        rows.push(DetailRow {
            label: "access.binary".to_owned(),
            value: network_type.binary.clone(),
        });
        rows.push(DetailRow {
            label: "access.description".to_owned(),
            value: network_type.description.clone(),
        });
        rows.push(DetailRow {
            label: "access.option".to_owned(),
            value: option.label.clone(),
        });
        rows.push(DetailRow {
            label: "access.command".to_owned(),
            value: option.command.clone(),
        });
        rows.push(DetailRow {
            label: "access.route".to_owned(),
            value: option.route.join(" -> "),
        });
        rows.push(DetailRow {
            label: "access.notes".to_owned(),
            value: option.notes.clone(),
        });
        rows
    }

    pub fn route_progress(&self) -> f32 {
        self.route_progress
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn continuous_time(&self) -> Duration {
        self.continuous_time
    }

    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    pub fn focus_rotation(&self) -> f64 {
        self.focus_rotation
    }

    pub fn pitch(&self) -> f64 {
        self.pitch
    }

    pub fn focus_pitch(&self) -> f64 {
        self.focus_pitch
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn focus_zoom(&self) -> f64 {
        self.focus_zoom
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn theme(&self) -> ThemeId {
        self.theme
    }

    pub fn camera_azimuth_deg(&self) -> f64 {
        self.rotation.to_degrees().rem_euclid(360.0)
    }

    pub fn camera_pitch_deg(&self) -> f64 {
        self.pitch.to_degrees()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn needs_render(&self) -> bool {
        self.dirty
    }

    pub fn mark_rendered(&mut self) {
        self.dirty = false;
    }

    pub fn is_animating(&self) -> bool {
        self.route_progress < 1.0
            || self.transition.is_some()
            || shortest_angle(self.focus_rotation - self.rotation).abs() > 0.001
            || (self.focus_pitch - self.pitch).abs() > 0.001
            || (self.focus_zoom - self.zoom).abs() > 0.001
    }

    pub fn tick(&mut self, delta: Duration) {
        let was_animating = self.is_animating();
        let ambient_frame = self.continuous_time.as_millis() / AMBIENT_FRAME_MS;
        self.continuous_time += delta;
        if !self.paused {
            self.elapsed += delta;
        }

        self.route_elapsed += delta;
        let route_t = (self.route_elapsed.as_secs_f32() / 1.2).min(1.0);
        self.route_progress = 1.0 - (1.0 - route_t).powi(3);

        if let Some(mut trans) = self.transition {
            trans.elapsed += delta;
            let t = (trans.elapsed.as_secs_f64() / trans.duration.as_secs_f64()).clamp(0.0, 1.0);

            // A fast lock acquisition followed by a gentle settle keeps target changes
            // decisive without introducing a hard camera stop.
            let s = smootherstep(t);
            let rot_delta = shortest_angle(trans.target_rotation - trans.start_rotation);
            self.rotation = trans.start_rotation + rot_delta * s;
            let pitch_delta = trans.target_pitch - trans.start_pitch;
            self.pitch = trans.start_pitch + pitch_delta * s;

            // Pull back quickly to establish the route, coast briefly, then push into
            // the destination as rotation settles. The three phases are continuous.
            let overview_zoom = (trans.start_zoom.min(trans.target_zoom) * 0.68).max(0.68);
            self.zoom = if t < 0.26 {
                let pullback = 1.0 - (1.0 - t / 0.26).powi(3);
                lerp(trans.start_zoom, overview_zoom, pullback)
            } else if t < 0.42 {
                overview_zoom
            } else {
                let push_in = smootherstep((t - 0.42) / 0.58);
                lerp(overview_zoom, trans.target_zoom, push_in)
            };

            if t >= 1.0 {
                self.rotation = trans.target_rotation;
                self.pitch = trans.target_pitch;
                self.zoom = trans.target_zoom;
                self.transition = None;
            } else {
                self.transition = Some(trans);
            }
            self.dirty = true;
        } else {
            self.rotation = approach_angle(
                self.rotation,
                self.focus_rotation,
                delta.as_secs_f64() * 2.4,
            );
            self.pitch = approach_angle(self.pitch, self.focus_pitch, delta.as_secs_f64() * 2.4);
            self.zoom = approach_value(self.zoom, self.focus_zoom, delta.as_secs_f64() * 0.8);
        }

        if !self.paused && self.elapsed >= Duration::from_secs(6) {
            // Begin the next acquisition on the following frame instead of applying
            // this frame's entire delta to a transition that did not exist yet.
            self.next_target();
        }

        if was_animating || self.is_animating() {
            self.dirty = true;
        }
        let next_ambient_frame = self.continuous_time.as_millis() / AMBIENT_FRAME_MS;
        if !self.paused && next_ambient_frame != ambient_frame {
            // Settled live mode only needs a ~6 Hz packet/countdown refresh. Camera
            // and route acquisition continue to use the high-rate animation path.
            self.dirty = true;
        }
    }

    pub fn is_transitioning(&self) -> bool {
        self.transition.is_some()
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(_, _) => self.dirty = true,
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('C') && self.refresh_state == RefreshState::Running {
            self.cancel_requested = true;
            self.refresh_state = RefreshState::Cancelling;
            self.dirty = true;
            return;
        }
        if self.command_library_open {
            match key.code {
                KeyCode::Esc if self.command_search_active || !self.command_filter.is_empty() => {
                    self.command_filter.clear();
                    self.command_search_active = false;
                    self.command_library_index = 0;
                    self.dirty = true;
                }
                KeyCode::Esc | KeyCode::Enter if !self.command_search_active => {
                    self.command_library_open = false;
                    self.dirty = true;
                }
                KeyCode::Enter => {
                    self.command_search_active = false;
                    self.dirty = true;
                }
                KeyCode::Up | KeyCode::BackTab => self.previous_extended_command(),
                KeyCode::Down | KeyCode::Tab => self.next_extended_command(),
                KeyCode::Char('/') if !self.command_search_active => {
                    self.command_filter.clear();
                    self.command_search_active = true;
                    self.dirty = true;
                }
                KeyCode::Backspace if self.command_search_active => {
                    self.command_filter.pop();
                    self.select_first_visible_command();
                }
                KeyCode::Char(character) if self.command_search_active => {
                    self.command_filter.push(character);
                    self.select_first_visible_command();
                }
                KeyCode::Char('y') => self.copy_extended_command(),
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            }
            return;
        }
        if self.connection_browser_open {
            self.handle_connection_browser_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.zoom_in(),
            KeyCode::Char('-') | KeyCode::Char('_') => self.zoom_out(),
            KeyCode::Char('h') | KeyCode::Char('a') => self.manual_pan(-0.1, 0.0),
            KeyCode::Char('l') | KeyCode::Char('d') => self.manual_pan(0.1, 0.0),
            KeyCode::Char('k') | KeyCode::Char('w') => self.manual_pan(0.0, 0.08),
            KeyCode::Char('j') | KeyCode::Char('s') => self.manual_pan(0.0, -0.08),
            KeyCode::Char('r') => self.reset_camera(),
            KeyCode::Char('R')
                if self.discovery_enabled
                    && !matches!(
                        self.refresh_state,
                        RefreshState::Running | RefreshState::Cancelling
                    ) =>
            {
                self.refresh_requested = true;
                self.dirty = true;
            }
            KeyCode::Char('y') => {
                self.copy_request = Some(self.current_access_option().command.clone());
                self.dirty = true;
            }
            KeyCode::Char('g') if !self.inventory.connections.is_empty() => {
                self.connection_browser_open = true;
                self.connection_browser_index = 0;
                self.connection_provider_filter = None;
                self.connection_query.clear();
                self.connection_search_active = false;
                self.dirty = true;
            }
            KeyCode::Left => self.previous_target(),
            KeyCode::Right => self.next_target(),
            KeyCode::Up => self.previous_detail(),
            KeyCode::Down => self.next_detail(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.previous_access_option()
            }
            KeyCode::Tab => self.next_access_option(),
            KeyCode::BackTab => self.previous_access_option(),
            KeyCode::Enter if !self.extended_commands().is_empty() => {
                self.command_library_open = true;
                self.command_library_index = 0;
                self.command_filter.clear();
                self.command_search_active = false;
                self.dirty = true;
            }
            KeyCode::Enter => {}
            _ => {}
        }
    }

    fn handle_connection_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc if self.connection_search_active || !self.connection_query.is_empty() => {
                self.connection_query.clear();
                self.connection_search_active = false;
                self.connection_browser_index = 0;
                self.dirty = true;
            }
            KeyCode::Esc => {
                self.connection_browser_open = false;
                self.dirty = true;
            }
            KeyCode::Enter if self.connection_search_active => {
                self.connection_search_active = false;
                self.dirty = true;
            }
            KeyCode::Enter => self.select_browser_connection(),
            KeyCode::Tab => self.cycle_connection_provider(false),
            KeyCode::BackTab => self.cycle_connection_provider(true),
            KeyCode::Up => self.move_connection_browser(-1),
            KeyCode::Down => self.move_connection_browser(1),
            KeyCode::Char('/') if !self.connection_search_active => {
                self.connection_query.clear();
                self.connection_search_active = true;
                self.dirty = true;
            }
            KeyCode::Backspace if self.connection_search_active => {
                self.connection_query.pop();
                self.connection_browser_index = 0;
                self.dirty = true;
            }
            KeyCode::Char(character) if self.connection_search_active => {
                self.connection_query.push(character);
                self.connection_browser_index = 0;
                self.dirty = true;
            }
            KeyCode::Char('q') => self.quit = true,
            _ => {}
        }
    }

    fn connection_providers(&self) -> Vec<Provider> {
        self.inventory
            .connections
            .iter()
            .map(|connection| connection.provider)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn cycle_connection_provider(&mut self, backwards: bool) {
        let providers = self.connection_providers();
        let current = self
            .connection_provider_filter
            .and_then(|provider| providers.iter().position(|item| *item == provider))
            .map_or(0, |index| index + 1);
        let count = providers.len() + 1;
        let next = if backwards {
            (current + count - 1) % count
        } else {
            (current + 1) % count
        };
        self.connection_provider_filter = if next == 0 {
            None
        } else {
            Some(providers[next - 1])
        };
        self.connection_browser_index = 0;
        self.dirty = true;
    }

    fn move_connection_browser(&mut self, direction: isize) {
        let count = self.visible_connections().len();
        if count > 0 {
            self.connection_browser_index = if direction < 0 {
                (self.connection_browser_index + count - 1) % count
            } else {
                (self.connection_browser_index + 1) % count
            };
            self.dirty = true;
        }
    }

    fn select_browser_connection(&mut self) {
        let Some(connection_id) = self
            .visible_connections()
            .get(self.connection_browser_index)
            .map(|connection| connection.id.clone())
        else {
            return;
        };
        let target_id = format!("discovered:{connection_id}");
        if let Some(index) = self
            .topology
            .targets
            .iter()
            .position(|target| target.id == target_id)
        {
            self.target_index = index;
            self.connection_browser_open = false;
            self.update_camera_focus();
            self.reset_target_animation();
        }
    }

    fn next_extended_command(&mut self) {
        let indices = self
            .visible_extended_commands()
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if !indices.is_empty() {
            let position = indices
                .iter()
                .position(|index| *index == self.command_library_index)
                .unwrap_or(0);
            self.command_library_index = indices[(position + 1) % indices.len()];
            self.dirty = true;
        }
    }

    fn previous_extended_command(&mut self) {
        let indices = self
            .visible_extended_commands()
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if !indices.is_empty() {
            let position = indices
                .iter()
                .position(|index| *index == self.command_library_index)
                .unwrap_or(0);
            self.command_library_index = indices[(position + indices.len() - 1) % indices.len()];
            self.dirty = true;
        }
    }

    fn select_first_visible_command(&mut self) {
        if let Some((index, _)) = self.visible_extended_commands().first() {
            self.command_library_index = *index;
        }
        self.dirty = true;
    }

    fn copy_extended_command(&mut self) {
        self.copy_request = self
            .selected_extended_command()
            .map(|command| command.command.clone());
        self.dirty = true;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.dirty = true;
    }

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
        self.dirty = true;
    }

    pub fn set_theme(&mut self, theme: ThemeId) {
        self.theme = theme;
        self.dirty = true;
    }

    pub fn zoom_in(&mut self) {
        if self.is_transitioning() {
            return;
        }
        self.focus_zoom = (self.focus_zoom * 1.18).min(3.5);
        self.zoom = self.focus_zoom;
        self.dirty = true;
    }

    pub fn zoom_out(&mut self) {
        if self.is_transitioning() {
            return;
        }
        self.focus_zoom = (self.focus_zoom / 1.18).max(0.6);
        self.zoom = self.focus_zoom;
        self.dirty = true;
    }

    pub fn manual_pan(&mut self, delta_rot: f64, delta_pitch: f64) {
        if self.is_transitioning() {
            return;
        }
        self.focus_rotation = (self.focus_rotation + delta_rot).rem_euclid(TAU);
        self.focus_pitch =
            (self.focus_pitch + delta_pitch).clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
        self.dirty = true;
    }

    pub fn reset_camera(&mut self) {
        if self.is_transitioning() {
            return;
        }
        self.update_camera_focus();
        self.dirty = true;
    }

    pub fn next_target(&mut self) {
        self.target_index = (self.target_index + 1) % self.topology.targets.len();
        self.update_camera_focus();
        self.reset_target_animation();
    }

    pub fn previous_target(&mut self) {
        self.target_index = if self.target_index == 0 {
            self.topology.targets.len() - 1
        } else {
            self.target_index - 1
        };
        self.update_camera_focus();
        self.reset_target_animation();
    }

    pub fn next_access_option(&mut self) {
        let option_count = self.current_network_type().access_options.len();
        if self.access_option_index + 1 < option_count {
            self.access_option_index += 1;
        } else {
            self.network_type_index =
                (self.network_type_index + 1) % self.target().network_types.len();
            self.access_option_index = 0;
        }
        self.detail_index = 0;
        self.dirty = true;
    }

    pub fn previous_access_option(&mut self) {
        if self.access_option_index > 0 {
            self.access_option_index -= 1;
        } else {
            self.network_type_index = if self.network_type_index == 0 {
                self.target().network_types.len() - 1
            } else {
                self.network_type_index - 1
            };
            self.access_option_index = self.current_network_type().access_options.len() - 1;
        }
        self.detail_index = 0;
        self.dirty = true;
    }

    pub fn next_detail(&mut self) {
        let count = self.detail_rows().len();
        self.detail_index = (self.detail_index + 1) % count;
        self.dirty = true;
    }

    pub fn previous_detail(&mut self) {
        let count = self.detail_rows().len();
        self.detail_index = if self.detail_index == 0 {
            count - 1
        } else {
            self.detail_index - 1
        };
        self.dirty = true;
    }

    fn reset_target_animation(&mut self) {
        self.network_type_index = 0;
        self.access_option_index = 0;
        self.detail_index = 0;
        self.elapsed = Duration::ZERO;
        self.route_elapsed = Duration::ZERO;
        self.route_progress = 0.0;
        self.dirty = true;
    }

    fn update_camera_focus(&mut self) {
        let target_rotation = target_focus_rotation(self.target());
        let target_pitch = target_focus_pitch(self.target());
        let target_zoom = target_focus_zoom(self.target());

        self.focus_rotation = target_rotation;
        self.focus_pitch = target_pitch;
        self.focus_zoom = target_zoom;

        self.transition = Some(CameraTransition {
            start_rotation: self.rotation,
            target_rotation,
            start_pitch: self.pitch,
            target_pitch,
            start_zoom: self.zoom,
            target_zoom,
            elapsed: Duration::ZERO,
            duration: Duration::from_millis(1_400),
        });
    }
}

fn connection_target(
    connection: &DiscoveredConnection,
    fallback_location: &Location,
    generated_at_unix: u64,
) -> Target {
    let mut location = inferred_connection_location(connection, fallback_location);
    location.source = format!("{} CLI discovery", connection.provider.as_str());
    let state = connection
        .metadata
        .get("power_state")
        .or_else(|| connection.metadata.get("state"))
        .or_else(|| connection.metadata.get("status"))
        .cloned()
        .unwrap_or_else(|| {
            if connection
                .metadata
                .get("online")
                .is_some_and(|value| value == "true")
            {
                "reachable".to_owned()
            } else {
                "discovered".to_owned()
            }
        });
    let primary = connection.primary_commands();
    let binary = primary
        .first()
        .and_then(|command| command.command.split_whitespace().next())
        .unwrap_or(connection.provider.as_str())
        .to_owned();
    let access_options = primary
        .into_iter()
        .map(|command| AccessOption {
            id: command.id.clone(),
            label: command.label.clone(),
            command: command.command.clone(),
            route: vec![
                "local-workstation".to_owned(),
                connection.provider.as_str().to_owned(),
                connection.label.clone(),
            ],
            notes: command.description.clone(),
        })
        .collect();
    let mut metadata = connection.metadata.clone();
    metadata.insert(
        "discovery.provider".to_owned(),
        connection.provider.as_str().to_owned(),
    );
    metadata.insert("discovery.kind".to_owned(), connection.kind.clone());

    Target {
        id: format!("discovered:{}", connection.id),
        label: connection.label.clone(),
        kind: connection.kind.clone(),
        provider: connection.provider.as_str().to_owned(),
        location,
        status: Health {
            state,
            uptime_seconds: 0,
            latency_ms: 0.0,
            packet_loss_percent: 0.0,
            checked_at: format!("unix:{generated_at_unix}"),
        },
        network: BTreeMap::from([("source".to_owned(), "local-cli".to_owned())]),
        metadata,
        network_types: vec![NetworkType {
            id: "discovered-commands".to_owned(),
            label: format!("{} {}", connection.provider.as_str(), connection.kind),
            binary,
            description: "Read-only command templates generated from discovered metadata."
                .to_owned(),
            access_options,
        }],
    }
}

fn inferred_connection_location(
    connection: &DiscoveredConnection,
    fallback_location: &Location,
) -> Location {
    let region = connection
        .metadata
        .get("region")
        .or_else(|| connection.metadata.get("location"))
        .or_else(|| connection.metadata.get("zone"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let (city, country, latitude, longitude, located) = match region.as_str() {
        value if value.starts_with("europe-west4") => ("Amsterdam", "NL", 52.37, 4.90, true),
        value if value.starts_with("eu-west-1") => ("Dublin", "IE", 53.35, -6.26, true),
        "westeurope" => ("Amsterdam", "NL", 52.37, 4.90, true),
        value if value.starts_with("us-east-1") => ("Ashburn", "US", 39.04, -77.49, true),
        value if value.starts_with("asia-northeast1") => ("Tokyo", "JP", 35.68, 139.65, true),
        _ => (
            "Unlocated",
            "--",
            fallback_location.latitude,
            fallback_location.longitude,
            false,
        ),
    };
    Location {
        label: if located {
            format!("Provider region {region}")
        } else {
            "Unlocated · anchored to origin".to_owned()
        },
        region,
        city: city.to_owned(),
        country: country.to_owned(),
        timezone: "unknown".to_owned(),
        source: String::new(),
        precision: if located {
            "provider-region"
        } else {
            "unknown"
        }
        .to_owned(),
        latitude,
        longitude,
    }
}

fn target_focus_rotation(target: &Target) -> f64 {
    target.location.longitude.to_radians()
}

fn target_focus_pitch(target: &Target) -> f64 {
    target.location.latitude.to_radians()
}

fn target_focus_zoom(target: &Target) -> f64 {
    match target.location.precision.as_str() {
        "city" => 1.30,
        "country" => 1.18,
        "region" => 1.10,
        _ => 1.0,
    }
}

fn approach_angle(current: f64, target: f64, max_step: f64) -> f64 {
    let delta = shortest_angle(target - current);
    if delta.abs() <= 0.0005 {
        target
    } else {
        let ease_rate = (delta * 3.5).clamp(-max_step, max_step);
        let min_rate = (max_step * 0.25).min(delta.abs());
        let step = if ease_rate.abs() < min_rate {
            delta.signum() * min_rate
        } else {
            ease_rate
        };
        current + step
    }
}

fn approach_value(current: f64, target: f64, max_step: f64) -> f64 {
    let delta = target - current;
    if delta.abs() <= 0.0005 {
        target
    } else {
        let ease_rate = (delta * 3.0).clamp(-max_step, max_step);
        let min_rate = (max_step * 0.25).min(delta.abs());
        let step = if ease_rate.abs() < min_rate {
            delta.signum() * min_rate
        } else {
            ease_rate
        };
        current + step
    }
}

fn smootherstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}

fn shortest_angle(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Topology;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    const FIXTURE: &str = include_str!("../data/demo-topology.json");

    fn app() -> App {
        App::new(Topology::from_json(FIXTURE).expect("fixture should parse"))
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn arrows_change_targets_and_wrap() {
        let mut app = app();
        app.handle_key(key(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.target_index(), 7);
        app.handle_key(key(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.target_index(), 0);
    }

    #[test]
    fn camera_focus_updates_for_us_target() {
        let mut app = app();
        let initial_pitch = app.focus_pitch();
        for _ in 0..6 {
            app.next_target();
        }
        assert_eq!(app.target().id, "gcp-us-micro");
        assert!((app.focus_pitch() - 39.04_f64.to_radians()).abs() < 0.001);
        assert!((app.focus_pitch() - initial_pitch).abs() > 0.1);
        assert!((app.focus_zoom() - 1.30).abs() < f64::EPSILON);
    }

    #[test]
    fn tab_cycles_options_then_crosses_network_types() {
        let mut app = app();
        assert_eq!(app.current_network_type().id, "ssh");
        assert_eq!(app.current_access_option().id, "gcp-iap-ssh");

        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_access_option().id, "gcp-os-login");
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_access_option().id, "gcp-proxycommand");
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_network_type().id, "console");
        assert_eq!(app.current_access_option().id, "gcp-serial-console");
    }

    #[test]
    fn shift_tab_and_backtab_reverse_across_network_types() {
        let mut app = app();
        app.handle_key(key(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(app.current_network_type().id, "console");
        assert_eq!(app.current_access_option().id, "gcp-serial-console");
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(app.current_access_option().id, "gcp-proxycommand");
    }

    #[test]
    fn up_and_down_cycle_json_detail_rows() {
        let mut app = app();
        let count = app.detail_rows().len();
        app.handle_key(key(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.detail_index(), count - 1);
        app.handle_key(key(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.detail_index(), 0);
        assert!(
            app.detail_rows()
                .iter()
                .any(|row| { row.label == "access.binary" && row.value == "ssh" })
        );
    }

    #[test]
    fn enter_is_intentionally_a_noop_and_q_exits() {
        let mut app = app();
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.should_quit());
        app.handle_key(key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.should_quit());
    }

    #[test]
    fn resize_events_invalidate_a_settled_frame_in_both_directions() {
        let mut app = app();
        app.tick(Duration::from_secs(2));
        app.mark_rendered();
        app.tick(Duration::from_secs(1));
        assert!(!app.needs_render());

        app.handle_event(Event::Resize(80, 24));
        assert!(app.needs_render());
        app.mark_rendered();

        app.handle_event(Event::Resize(120, 40));
        assert!(app.needs_render());
    }

    #[test]
    fn animation_completes_route_and_locks_target_heading() {
        let mut app = app();
        app.toggle_pause(); // Unpause for auto animation test
        let initial_focus = app.focus_rotation();
        let initial_zoom = app.focus_zoom();
        app.tick(Duration::from_secs(3));
        assert_eq!(app.route_progress(), 1.0);
        assert!(shortest_angle(app.rotation() - initial_focus).abs() < 0.001);
        assert!((app.pitch() - app.focus_pitch()).abs() < 0.001);
        assert!((app.zoom() - initial_zoom).abs() < 0.001);
        app.tick(Duration::from_secs(3));
        assert_eq!(app.target_index(), 1);
        assert_eq!(app.route_progress(), 0.0);
        assert_ne!(app.focus_rotation(), initial_focus);
    }

    #[test]
    fn held_mode_settles_then_stops_redrawing() {
        let mut app = app();
        assert!(app.needs_render());
        assert!(app.is_animating());

        app.mark_rendered();
        app.tick(Duration::from_secs(2));
        assert!(!app.is_animating());
        assert!(app.needs_render());

        app.mark_rendered();
        app.tick(Duration::from_secs(1));
        assert!(!app.needs_render());

        app.next_target();
        assert!(app.needs_render());
        assert!(app.is_animating());
    }

    #[test]
    fn live_mode_caps_ambient_redraws_at_six_hz() {
        let mut app = app();
        app.tick(Duration::from_secs(2));
        app.mark_rendered();
        app.toggle_pause();
        app.mark_rendered();

        app.tick(Duration::from_millis(50));
        assert!(!app.needs_render(), "sub-frame tick should remain clean");
        app.tick(Duration::from_millis(110));
        assert!(app.needs_render(), "160 ms boundary should request a frame");
    }

    #[test]
    fn camera_transition_pulls_back_coasts_then_locks_at_1_4s() {
        let mut app = app();
        app.next_target();
        let target_zoom = app.focus_zoom();
        let initial_error = shortest_angle(app.focus_rotation() - app.rotation()).abs();

        app.tick(Duration::from_millis(400));
        assert!(app.is_animating());
        let overview_zoom = app.zoom();
        let acquisition_error = shortest_angle(app.focus_rotation() - app.rotation()).abs();
        assert!(overview_zoom < target_zoom - 0.30);
        assert!(acquisition_error < initial_error);

        app.tick(Duration::from_millis(400));
        assert!(
            app.zoom() >= overview_zoom,
            "push-in should follow the coast"
        );
        assert!(
            shortest_angle(app.focus_rotation() - app.rotation()).abs() < acquisition_error,
            "lock error should decrease monotonically"
        );

        app.tick(Duration::from_millis(600));
        assert!((app.zoom() - target_zoom).abs() < 0.001);
        assert!(shortest_angle(app.rotation() - app.focus_rotation()).abs() < 0.001);
        assert!((app.pitch() - app.focus_pitch()).abs() < 0.001);
    }

    #[test]
    fn manual_zoom_in_and_out_does_not_bounce_after_ticks() {
        let mut app = app();
        let initial_zoom = app.zoom();

        // Manual zoom in
        app.handle_key(key(KeyCode::Char('+'), KeyModifiers::NONE));
        let zoomed_in = app.zoom();
        assert!(zoomed_in > initial_zoom);

        // Tick multiple frames
        app.tick(Duration::from_millis(50));
        app.tick(Duration::from_millis(100));
        app.tick(Duration::from_millis(500));
        // Zoom must stay exactly at the manually set zoom level without bouncing back
        assert_eq!(app.zoom(), zoomed_in);

        // Manual zoom out twice
        app.handle_key(key(KeyCode::Char('-'), KeyModifiers::NONE));
        app.handle_key(key(KeyCode::Char('-'), KeyModifiers::NONE));
        let zoomed_out = app.zoom();
        assert!(zoomed_out < zoomed_in);

        // Tick multiple frames
        app.tick(Duration::from_millis(100));
        app.tick(Duration::from_millis(800));
        assert_eq!(app.zoom(), zoomed_out);
    }

    #[test]
    fn manual_pan_and_zoom_are_ignored_during_active_transition() {
        let mut app = app();
        app.next_target();
        assert!(app.is_transitioning());

        let transition_rot = app.rotation();
        let transition_zoom = app.zoom();

        // Attempt manual pan and zoom during active transition
        app.handle_key(key(KeyCode::Char('+'), KeyModifiers::NONE));
        app.handle_key(key(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(key(KeyCode::Char('k'), KeyModifiers::NONE));

        // Must remain unchanged during transition
        assert_eq!(app.zoom(), transition_zoom);
        assert_eq!(app.rotation(), transition_rot);
        assert!(app.is_transitioning());

        // Complete the 2s camera swoop transition
        app.tick(Duration::from_secs(2));
        assert!(!app.is_transitioning());

        // After transition ends, manual zoom and pan take effect immediately
        let completed_zoom = app.zoom();
        app.handle_key(key(KeyCode::Char('+'), KeyModifiers::NONE));
        assert!(app.zoom() > completed_zoom);
    }

    #[test]
    fn theme_and_interactive_controls_work() {
        let mut app = app();
        assert_eq!(app.theme(), ThemeId::CyberOrbital);
        app.handle_key(key(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.theme(), ThemeId::TacticalRadar);
        assert!(app.is_paused()); // Default is paused
        app.handle_key(key(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(!app.is_paused()); // Unpaused
        let initial_zoom = app.focus_zoom();
        app.handle_key(key(KeyCode::Char('+'), KeyModifiers::NONE));
        assert!(app.focus_zoom() > initial_zoom);
        app.handle_key(key(KeyCode::Char('-'), KeyModifiers::NONE));
        assert!((app.focus_zoom() - initial_zoom).abs() < 0.001);
    }
}
