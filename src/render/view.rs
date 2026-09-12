use crate::app::App;
use crate::model::{Origin, Target};
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct GlobeView<'a> {
    pub rotation: f64,
    pub pitch: f64,
    pub zoom: f64,
    pub origin: &'a Origin,
    pub targets: &'a [Target],
    pub selected: usize,
    pub route_progress: f32,
    pub continuous_time: Duration,
    pub elapsed: Duration,
    pub paused: bool,
}

impl<'a> GlobeView<'a> {
    pub fn from_app(app: &'a App) -> Self {
        Self {
            rotation: app.rotation(),
            pitch: app.pitch(),
            zoom: app.zoom(),
            origin: &app.topology().origin,
            targets: &app.topology().targets,
            selected: app.target_index(),
            route_progress: app.route_progress(),
            continuous_time: app.continuous_time(),
            elapsed: app.elapsed(),
            paused: app.is_paused(),
        }
    }

    pub fn target(self) -> &'a Target {
        &self.targets[self.selected]
    }

    pub fn azimuth_deg(self) -> f64 {
        self.rotation.to_degrees().rem_euclid(360.0)
    }

    pub fn pitch_deg(self) -> f64 {
        self.pitch.to_degrees()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::Topology;

    const FIXTURE: &str = include_str!("../../data/demo-topology.json");

    #[test]
    fn globe_view_snapshots_camera_without_session_flags() {
        let app = App::new(Topology::from_json(FIXTURE).expect("fixture should parse"));
        let view = GlobeView::from_app(&app);
        assert_eq!(view.selected, app.target_index());
        assert_eq!(view.targets.len(), app.topology().targets.len());
        assert_eq!(view.paused, app.is_paused());
        assert!((view.rotation - app.rotation()).abs() < f64::EPSILON);
    }
}
