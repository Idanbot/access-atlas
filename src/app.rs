use crate::model::{AccessOption, DetailRow, NetworkType, Target, Topology};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{
    f64::consts::{PI, TAU},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeId {
    CyberOrbital,
    #[default]
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
            Self::CyberOrbital => "Cyber Orbital",
            Self::TacticalRadar => "Tactical Radar (P31)",
            Self::MinimalAtlas => "Minimal Slate Atlas",
            Self::AmberCrt => "Amber CRT Phosphor",
            Self::DeepSpace => "Deep Space Nebula",
        }
    }
}

pub struct App {
    topology: Topology,
    target_index: usize,
    network_type_index: usize,
    access_option_index: usize,
    detail_index: usize,
    elapsed: Duration,
    continuous_time: Duration,
    route_progress: f32,
    rotation: f64,
    focus_rotation: f64,
    pitch: f64,
    focus_pitch: f64,
    zoom: f64,
    focus_zoom: f64,
    paused: bool,
    theme: ThemeId,
    dirty: bool,
    quit: bool,
}

impl App {
    pub fn new(topology: Topology) -> Self {
        Self::with_theme(topology, ThemeId::TacticalRadar)
    }

    pub fn with_theme(topology: Topology, theme: ThemeId) -> Self {
        let focus_rotation = target_focus_rotation(&topology.targets[0]);
        let focus_pitch = target_focus_pitch(&topology.targets[0]);
        let focus_zoom = target_focus_zoom(&topology.targets[0]);
        Self {
            topology,
            target_index: 0,
            network_type_index: 0,
            access_option_index: 0,
            detail_index: 0,
            elapsed: Duration::ZERO,
            continuous_time: Duration::ZERO,
            route_progress: 0.0,
            rotation: focus_rotation,
            focus_rotation,
            pitch: focus_pitch,
            focus_pitch,
            zoom: focus_zoom,
            focus_zoom,
            paused: true,
            theme,
            dirty: true,
            quit: false,
        }
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
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
        (PI / 2.0 - self.rotation).to_degrees().rem_euclid(360.0)
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
            || shortest_angle(self.focus_rotation - self.rotation).abs() > 0.001
            || (self.focus_pitch - self.pitch).abs() > 0.001
            || (self.focus_zoom - self.zoom).abs() > 0.001
    }

    pub fn tick(&mut self, delta: Duration) {
        let was_animating = self.is_animating();
        self.continuous_time += delta;
        if !self.paused {
            self.elapsed += delta;
            self.route_progress = (self.elapsed.as_secs_f32() / 2.5).min(1.0);
            if self.elapsed >= Duration::from_secs(6) {
                self.next_target();
            }
        }
        self.rotation = approach_angle(
            self.rotation,
            self.focus_rotation,
            delta.as_secs_f64() * 2.4,
        );
        self.pitch = approach_angle(self.pitch, self.focus_pitch, delta.as_secs_f64() * 2.4);
        self.zoom = approach_value(self.zoom, self.focus_zoom, delta.as_secs_f64() * 0.8);

        if was_animating || self.is_animating() {
            self.dirty = true;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.zoom_in(),
            KeyCode::Char('-') | KeyCode::Char('_') => self.zoom_out(),
            KeyCode::Char('h') | KeyCode::Char('a') => self.manual_pan(-0.1, 0.0),
            KeyCode::Char('l') | KeyCode::Char('d') => self.manual_pan(0.1, 0.0),
            KeyCode::Char('k') | KeyCode::Char('w') => self.manual_pan(0.0, 0.08),
            KeyCode::Char('j') => self.manual_pan(0.0, -0.08),
            KeyCode::Char('r') => self.reset_camera(),
            KeyCode::Left => self.previous_target(),
            KeyCode::Right => self.next_target(),
            KeyCode::Up => self.previous_detail(),
            KeyCode::Down => self.next_detail(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.previous_access_option()
            }
            KeyCode::Tab => self.next_access_option(),
            KeyCode::BackTab => self.previous_access_option(),
            KeyCode::Enter => {}
            _ => {}
        }
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
        self.focus_zoom = (self.focus_zoom * 1.18).min(3.5);
        self.dirty = true;
    }

    pub fn zoom_out(&mut self) {
        self.focus_zoom = (self.focus_zoom / 1.18).max(0.6);
        self.dirty = true;
    }

    pub fn manual_pan(&mut self, delta_rot: f64, delta_pitch: f64) {
        self.focus_rotation = (self.focus_rotation + delta_rot).rem_euclid(TAU);
        self.focus_pitch =
            (self.focus_pitch + delta_pitch).clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
        self.dirty = true;
    }

    pub fn reset_camera(&mut self) {
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
        self.route_progress = 0.0;
        self.dirty = true;
    }

    fn update_camera_focus(&mut self) {
        self.focus_rotation = target_focus_rotation(self.target());
        self.focus_pitch = target_focus_pitch(self.target());
        self.focus_zoom = target_focus_zoom(self.target());
    }
}

fn target_focus_rotation(target: &Target) -> f64 {
    PI / 2.0 - target.location.longitude.to_radians()
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
    if delta.abs() <= max_step {
        target
    } else {
        current + delta.signum() * max_step
    }
}

fn approach_value(current: f64, target: f64, max_step: f64) -> f64 {
    let delta = target - current;
    if delta.abs() <= max_step {
        target
    } else {
        current + delta.signum() * max_step
    }
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
    fn steady_state_stops_animation_and_redraws_only_after_changes() {
        let mut app = app();
        app.toggle_pause(); // Unpause for animation test
        assert!(app.needs_render());
        assert!(app.is_animating());

        app.mark_rendered();
        app.tick(Duration::from_secs(3));
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
    fn theme_and_interactive_controls_work() {
        let mut app = app();
        assert_eq!(app.theme(), ThemeId::TacticalRadar);
        app.handle_key(key(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.theme(), ThemeId::MinimalAtlas);
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
