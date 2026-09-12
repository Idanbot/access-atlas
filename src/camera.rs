use crate::model::Target;
use std::f64::consts::{PI, TAU};
use std::time::Duration;

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

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    rotation: f64,
    focus_rotation: f64,
    pitch: f64,
    focus_pitch: f64,
    zoom: f64,
    focus_zoom: f64,
    transition: Option<CameraTransition>,
}

impl Camera {
    pub fn focused_on(target: Option<&Target>) -> Self {
        let focus_rotation = target.map(focus_rotation).unwrap_or(0.0);
        let focus_pitch = target.map(focus_pitch).unwrap_or(0.0);
        let focus_zoom = target.map(focus_zoom).unwrap_or(1.0);
        Self {
            rotation: focus_rotation,
            focus_rotation,
            pitch: focus_pitch,
            focus_pitch,
            zoom: focus_zoom,
            focus_zoom,
            transition: None,
        }
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

    pub fn azimuth_deg(&self) -> f64 {
        self.rotation.to_degrees().rem_euclid(360.0)
    }

    pub fn pitch_deg(&self) -> f64 {
        self.pitch.to_degrees()
    }

    pub fn is_transitioning(&self) -> bool {
        self.transition.is_some()
    }

    pub fn is_settling(&self) -> bool {
        self.transition.is_some()
            || shortest_angle(self.focus_rotation - self.rotation).abs() > 0.001
            || (self.focus_pitch - self.pitch).abs() > 0.001
            || (self.focus_zoom - self.zoom).abs() > 0.001
    }

    pub fn tick(&mut self, delta: Duration) -> bool {
        if let Some(mut trans) = self.transition {
            trans.elapsed += delta;
            let t = (trans.elapsed.as_secs_f64() / trans.duration.as_secs_f64()).clamp(0.0, 1.0);
            let s = smootherstep(t);
            let rot_delta = shortest_angle(trans.target_rotation - trans.start_rotation);
            self.rotation = trans.start_rotation + rot_delta * s;
            let pitch_delta = trans.target_pitch - trans.start_pitch;
            self.pitch = trans.start_pitch + pitch_delta * s;
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
            true
        } else {
            self.rotation = approach_angle(
                self.rotation,
                self.focus_rotation,
                delta.as_secs_f64() * 2.4,
            );
            self.pitch = approach_angle(self.pitch, self.focus_pitch, delta.as_secs_f64() * 2.4);
            self.zoom = approach_value(self.zoom, self.focus_zoom, delta.as_secs_f64() * 0.8);
            false
        }
    }

    pub fn zoom_in(&mut self) {
        if self.is_transitioning() {
            return;
        }
        self.focus_zoom = (self.focus_zoom * 1.18).min(3.5);
        self.zoom = self.focus_zoom;
    }

    pub fn zoom_out(&mut self) {
        if self.is_transitioning() {
            return;
        }
        self.focus_zoom = (self.focus_zoom / 1.18).max(0.6);
        self.zoom = self.focus_zoom;
    }

    pub fn pan(&mut self, delta_rot: f64, delta_pitch: f64) {
        if self.is_transitioning() {
            return;
        }
        self.focus_rotation = (self.focus_rotation + delta_rot).rem_euclid(TAU);
        self.focus_pitch =
            (self.focus_pitch + delta_pitch).clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
    }

    pub fn look_at(&mut self, target: &Target) {
        if !target.location_known() {
            return;
        }
        let target_rotation = focus_rotation(target);
        let target_pitch = focus_pitch(target);
        let target_zoom = focus_zoom(target);
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

pub fn shortest_angle(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn focus_rotation(target: &Target) -> f64 {
    target.location.longitude.to_radians()
}

fn focus_pitch(target: &Target) -> f64 {
    target.location.latitude.to_radians()
}

fn focus_zoom(target: &Target) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Topology;

    const FIXTURE: &str = include_str!("../data/demo-topology.json");

    #[test]
    fn look_at_starts_a_transition_for_a_located_target() {
        let topology = Topology::from_json(FIXTURE).expect("fixture should parse");
        let mut camera = Camera::focused_on(topology.targets.first());
        assert!(!camera.is_transitioning());
        camera.look_at(&topology.targets[1]);
        assert!(camera.is_transitioning());
        assert!(camera.is_settling());
    }
}
