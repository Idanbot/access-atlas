use crate::app::{App, ThemeId};
use glam::DVec3;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};
use std::sync::OnceLock;

const MASK_WIDTH: usize = 1440;
const MASK_HEIGHT: usize = 720;

const GRID_COLS: usize = 72; // 5 degrees per column
const GRID_ROWS: usize = 36; // 5 degrees per row

pub const BRAILLE_DOTS: [(usize, usize, u8); 8] = [
    (0, 0, 0b0000_0001), // dot 1
    (0, 1, 0b0000_0010), // dot 2
    (0, 2, 0b0000_0100), // dot 3
    (1, 0, 0b0000_1000), // dot 4
    (1, 1, 0b0001_0000), // dot 5
    (1, 2, 0b0010_0000), // dot 6
    (0, 3, 0b0100_0000), // dot 7
    (1, 3, 0b1000_0000), // dot 8
];

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub name: &'static str,
    pub background: [u8; 3],
    pub ocean: [u8; 3],
    pub land: [u8; 3],
    pub coast: [u8; 3],
    pub border: [u8; 3],
    pub atmosphere: [u8; 3],
    pub active_target: [u8; 3],
    pub other_target: [u8; 3],
    pub origin: [u8; 3],
    pub route: [u8; 3],
    pub packet: [u8; 3],
    pub hud_accent: [u8; 3],
    pub hud_text: [u8; 3],
    pub border_color: [u8; 3],
}

pub fn get_theme(theme_id: ThemeId) -> ThemePalette {
    match theme_id {
        ThemeId::CyberOrbital => ThemePalette {
            name: "Cyber Orbital",
            background: [4, 8, 16],
            ocean: [12, 36, 60],
            land: [42, 175, 110],
            coast: [75, 230, 245],
            border: [135, 230, 185],
            atmosphere: [65, 175, 255],
            active_target: [255, 225, 90],
            other_target: [80, 195, 240],
            origin: [255, 90, 210],
            route: [255, 135, 55],
            packet: [255, 255, 215],
            hud_accent: [120, 220, 255],
            hud_text: [160, 205, 235],
            border_color: [40, 100, 145],
        },
        ThemeId::TacticalRadar => ThemePalette {
            name: "Tactical Radar (P31)",
            background: [2, 10, 4],
            ocean: [6, 28, 12],
            land: [28, 115, 50],
            coast: [65, 225, 105],
            border: [100, 185, 125],
            atmosphere: [50, 155, 75],
            active_target: [220, 255, 225],
            other_target: [75, 185, 105],
            origin: [160, 255, 180],
            route: [130, 255, 155],
            packet: [245, 255, 245],
            hud_accent: [80, 240, 120],
            hud_text: [120, 200, 140],
            border_color: [30, 120, 55],
        },
        ThemeId::MinimalAtlas => ThemePalette {
            name: "Minimal Slate Atlas",
            background: [14, 18, 24],
            ocean: [22, 38, 48],
            land: [72, 118, 98],
            coast: [220, 200, 130],
            border: [190, 190, 185],
            atmosphere: [90, 130, 155],
            active_target: [255, 198, 75],
            other_target: [125, 175, 205],
            origin: [95, 165, 245],
            route: [240, 95, 80],
            packet: [255, 235, 210],
            hud_accent: [225, 205, 150],
            hud_text: [175, 185, 195],
            border_color: [60, 85, 105],
        },
        ThemeId::AmberCrt => ThemePalette {
            name: "Amber CRT",
            background: [16, 8, 2],
            ocean: [36, 18, 4],
            land: [120, 62, 16],
            coast: [225, 130, 32],
            border: [180, 100, 28],
            atmosphere: [155, 85, 22],
            active_target: [255, 225, 140],
            other_target: [185, 105, 30],
            origin: [255, 140, 40],
            route: [255, 165, 50],
            packet: [255, 245, 210],
            hud_accent: [255, 180, 60],
            hud_text: [210, 150, 80],
            border_color: [120, 65, 20],
        },
        ThemeId::DeepSpace => ThemePalette {
            name: "Deep Space Nebula",
            background: [6, 4, 14],
            ocean: [18, 18, 48],
            land: [64, 46, 115],
            coast: [145, 110, 235],
            border: [115, 85, 195],
            atmosphere: [120, 85, 220],
            active_target: [255, 60, 160],
            other_target: [80, 180, 255],
            origin: [0, 235, 255],
            route: [0, 220, 255],
            packet: [255, 255, 255],
            hud_accent: [190, 120, 255],
            hud_text: [180, 165, 225],
            border_color: [85, 55, 140],
        },
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let theme = get_theme(app.theme());

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(vertical[1]);

    let target = app.target();
    let network_type = app.current_network_type();
    let option = app.current_access_option();
    let target_title = format!(
        " {} | {}, {} | target {}/{} | {} ",
        target.label,
        target.location.city,
        target.location.country,
        app.target_index() + 1,
        app.topology().targets.len(),
        target.status.state
    );

    let status_indicator = if app.is_paused() {
        " [PAUSED] "
    } else {
        " [LIVE] "
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " ACCESS ATLAS ",
                Style::default()
                    .fg(to_color(theme.hud_accent))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Planetary Access Topology Demo",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                status_indicator,
                Style::default()
                    .fg(to_color(theme.active_target))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Theme: {} ", theme.name),
                Style::default().fg(to_color(theme.hud_text)),
            ),
        ])),
        vertical[0],
    );

    let globe_title = format!(" Planetary Access Globe · [{}] ", theme.name);
    let globe_block = Block::bordered()
        .title(globe_title)
        .border_style(Style::default().fg(to_color(theme.border_color)));
    let globe_area = globe_block.inner(main[0]);
    frame.render_widget(globe_block, main[0]);
    frame.render_widget(GlobeWidget { app, theme }, globe_area);

    let details = app.detail_rows();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("network: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(
                    "{}/{} {}",
                    app.network_type_index() + 1,
                    target.network_types.len(),
                    network_type.label
                ),
                Style::default().fg(to_color(theme.active_target)),
            ),
        ]),
        Line::from(vec![
            Span::styled("binary: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &network_type.binary,
                Style::default().fg(to_color(theme.hud_accent)),
            ),
        ]),
        Line::from(vec![
            Span::styled("option: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(
                    "{}/{} {}",
                    app.access_option_index() + 1,
                    network_type.access_options.len(),
                    option.label
                ),
                Style::default().fg(to_color(theme.active_target)),
            ),
        ]),
        Line::from(vec![
            Span::styled("command: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &option.command,
                Style::default().fg(to_color(theme.hud_accent)),
            ),
        ]),
        Line::from(""),
    ];
    for (index, row) in details.iter().enumerate() {
        let style = if index == app.detail_index() {
            Style::default()
                .fg(to_color(theme.hud_accent))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>20} ", row.label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(&row.value, style),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(target_title)
                    .border_style(Style::default().fg(to_color(theme.border_color))),
            )
            .wrap(Wrap { trim: false }),
        main[1],
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(" Space: Pause/Resume   t: Theme   h/j/k/l: Orbit   +/-: Zoom   r: Reset"),
            Line::from(" Tab/Shift+Tab: Option   Left/Right: Target   Up/Down: Detail   q: Exit"),
        ])
        .style(Style::default().fg(Color::DarkGray)),
        vertical[2],
    );
}

struct GlobeWidget<'a> {
    app: &'a App,
    theme: ThemePalette,
}

impl Widget for GlobeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_globe(area, buf, self.app, &self.theme);
    }
}

#[derive(Clone, Copy)]
struct PointMarker {
    x: f64,
    y: f64,
    radius_sq: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct RingMarker {
    x: f64,
    y: f64,
    radius: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct RouteSegment {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct GlobeGeometry {
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    pitch: f64,
    rotation_cos: f64,
    rotation_sin: f64,
    pitch_cos: f64,
    pitch_sin: f64,
}

#[derive(Clone, Copy)]
struct DotSample {
    color: [u8; 3],
    priority: u8,
}

fn render_globe(area: Rect, buf: &mut Buffer, app: &App, theme: &ThemePalette) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let pixel_width = area.width as usize * 2;
    let pixel_height = area.height as usize * 4;
    let geometry = GlobeGeometry {
        center_x: pixel_width as f64 / 2.0,
        center_y: pixel_height as f64 / 2.0,
        radius_x: (pixel_width as f64 * 0.43 * app.zoom()).max(1.0),
        radius_y: (pixel_height as f64 * 0.43 * app.zoom()).max(1.0),
        rotation: app.rotation(),
        pitch: app.pitch(),
        rotation_cos: app.rotation().cos(),
        rotation_sin: app.rotation().sin(),
        pitch_cos: app.pitch().cos(),
        pitch_sin: app.pitch().sin(),
    };
    let (points, rings, segments) = build_overlays(app, &geometry, theme);

    for cell_y in 0..area.height as usize {
        for cell_x in 0..area.width as usize {
            let mut bits = 0_u8;
            let mut best_sample = DotSample {
                color: theme.background,
                priority: 0,
            };
            // Full 8-dot Braille grid (2x4 subpixels) for high resolution
            for (dot_x, dot_y, mask) in BRAILLE_DOTS {
                let x = cell_x * 2 + dot_x;
                let y = cell_y * 4 + dot_y;
                if let Some(sample) = dot_sample(x, y, &geometry, &points, &rings, &segments, theme)
                {
                    bits |= mask;
                    if sample.priority >= best_sample.priority {
                        best_sample = sample;
                    }
                }
            }
            let glyph = braille_glyph(bits);
            let style = if bits == 0 {
                Style::default().bg(to_color(theme.background))
            } else {
                Style::default()
                    .fg(to_color(best_sample.color))
                    .bg(to_color(theme.background))
            };
            buf.set_string(
                area.x + cell_x as u16,
                area.y + cell_y as u16,
                glyph.to_string(),
                style,
            );
        }
    }

    render_city_label(area, buf, app, &geometry, theme);
    render_telemetry_hud(area, buf, app, theme);
}

fn render_city_label(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    geometry: &GlobeGeometry,
    theme: &ThemePalette,
) {
    let target = app.target();
    let point = geo_to_vec(target.location.latitude, target.location.longitude);
    let Some((x, y, _depth)) = project_vec_camera(
        point,
        geometry.rotation,
        geometry.pitch,
        geometry.center_x,
        geometry.center_y,
        geometry.radius_x,
        geometry.radius_y,
    ) else {
        return;
    };

    let label = format!(" ◉ {} ", target.location.city);
    let label_width = label.chars().count() as i32;
    let start_x = (x / 2.0).round() as i32 + 2;
    let start_y = (y / 4.0).round() as i32;
    let min_x = i32::from(area.x);
    let max_x = i32::from(area.x + area.width).saturating_sub(label_width);
    let draw_x = start_x.clamp(min_x, max_x.max(min_x));
    let draw_y = start_y.clamp(
        i32::from(area.y),
        i32::from(area.bottom().saturating_sub(1)),
    );
    buf.set_string(
        draw_x as u16,
        draw_y as u16,
        label,
        Style::default()
            .fg(to_color(theme.active_target))
            .bg(to_color(theme.background))
            .add_modifier(Modifier::BOLD),
    );
}

fn render_telemetry_hud(area: Rect, buf: &mut Buffer, app: &App, theme: &ThemePalette) {
    if area.width < 28 || area.height < 5 {
        return;
    }

    let lat = app.camera_pitch_deg();
    let lat_dir = if lat >= 0.0 { "N" } else { "S" };
    let lon = app.camera_azimuth_deg();
    let lon_dir = if lon <= 180.0 { "E" } else { "W" };
    let lon_val = if lon <= 180.0 { lon } else { 360.0 - lon };

    // Top-Left Telemetry Badge
    let top_left = format!(
        " LAT {:.1}°{} · LON {:.1}°{} ",
        lat.abs(),
        lat_dir,
        lon_val,
        lon_dir
    );
    buf.set_string(
        area.x + 1,
        area.y + 1,
        top_left,
        Style::default()
            .fg(to_color(theme.hud_accent))
            .bg(to_color(theme.background)),
    );

    // Top-Right Telemetry Badge
    let top_right = format!(" ZOOM {:.2}x · {} ", app.zoom(), theme.name);
    let top_right_len = top_right.chars().count() as u16;
    if area.width > top_right_len + 4 {
        buf.set_string(
            area.right().saturating_sub(top_right_len + 1),
            area.y + 1,
            top_right,
            Style::default()
                .fg(to_color(theme.hud_text))
                .bg(to_color(theme.background)),
        );
    }

    // Bottom-Left Target Lock Badge
    let target = app.target();
    let bot_left = format!(
        " ⌖ LOCK: {} [{}] ",
        target.location.city,
        target.provider.to_uppercase()
    );
    if area.height > 2 {
        buf.set_string(
            area.x + 1,
            area.bottom().saturating_sub(2),
            bot_left,
            Style::default()
                .fg(to_color(theme.active_target))
                .bg(to_color(theme.background))
                .add_modifier(Modifier::BOLD),
        );
    }

    // Bottom-Right Mode Badge
    let bot_right = if app.is_paused() {
        " [AUTO-CYCLE: PAUSED (Space to run)] ".to_owned()
    } else {
        format!(
            " AUTO-CYCLE: LIVE ({:.0}s) ",
            (6.0 - app.elapsed().as_secs_f64()).max(0.0)
        )
    };
    let bot_right_len = bot_right.chars().count() as u16;
    if area.width > bot_right_len + 4 && area.height > 2 {
        buf.set_string(
            area.right().saturating_sub(bot_right_len + 1),
            area.bottom().saturating_sub(2),
            bot_right,
            Style::default()
                .fg(to_color(theme.hud_text))
                .bg(to_color(theme.background)),
        );
    }
}

fn build_overlays(
    app: &App,
    geometry: &GlobeGeometry,
    theme: &ThemePalette,
) -> (Vec<PointMarker>, Vec<RingMarker>, Vec<RouteSegment>) {
    let mut points = Vec::with_capacity(32);
    let mut rings = Vec::with_capacity(8);
    let mut segments = Vec::with_capacity(64);

    let target = app.target();
    let origin = geo_to_vec(
        app.topology().origin.location.latitude,
        app.topology().origin.location.longitude,
    );
    let destination = geo_to_vec(target.location.latitude, target.location.longitude);

    // 1. Workstation Origin Marker (Clean compact beacon)
    if let Some((ox, oy, _depth)) = project_vec_camera(
        origin,
        geometry.rotation,
        geometry.pitch,
        geometry.center_x,
        geometry.center_y,
        geometry.radius_x,
        geometry.radius_y,
    ) {
        points.push(PointMarker {
            x: ox,
            y: oy,
            radius_sq: 1.25,
            color: theme.origin,
            priority: 4,
        });
    }

    // 2. 3D Elevated Parabolic Great-Circle Route (Subtle thin 1-dot hairline)
    if app.route_progress() > 0.0 {
        let steps = 60;
        let visible_steps = (steps as f32 * app.route_progress()).ceil() as usize;

        let packet_phase = (app.continuous_time().as_secs_f64() * 0.85).fract();
        let packet_step = (packet_phase * visible_steps.min(steps) as f64).round() as usize;

        let mut prev_pt: Option<(f64, f64)> = None;

        for step in 0..=visible_steps.min(steps) {
            let amount = step as f64 / steps as f64;
            // 3D Parabolic sub-orbital lift (12% altitude apex)
            let altitude = 1.0 + (amount * std::f64::consts::PI).sin() * 0.12;
            let elevated_point = interpolate_arc(origin, destination, amount) * altitude;

            if let Some((x, y, _depth)) = project_vec_camera(
                elevated_point,
                geometry.rotation,
                geometry.pitch,
                geometry.center_x,
                geometry.center_y,
                geometry.radius_x,
                geometry.radius_y,
            ) {
                if let Some((px, py)) = prev_pt {
                    let (color, priority) = if step == packet_step {
                        (theme.packet, 5)
                    } else if step < packet_step && packet_step - step <= 4 {
                        let tail_fade = 1.0 - (packet_step - step) as f64 / 4.0;
                        (scale_color(theme.packet, tail_fade * 0.85 + 0.15), 3)
                    } else {
                        (theme.route, 2)
                    };

                    segments.push(RouteSegment {
                        x1: px,
                        y1: py,
                        x2: x,
                        y2: y,
                        color,
                        priority,
                    });
                }

                // Traveling photon packet head dot
                if step == packet_step {
                    points.push(PointMarker {
                        x,
                        y,
                        radius_sq: 1.25,
                        color: theme.packet,
                        priority: 5,
                    });
                }

                prev_pt = Some((x, y));
            } else {
                prev_pt = None;
            }
        }
    }

    // 3. Targets (Active and Inactive targets)
    for (index, target_item) in app.topology().targets.iter().enumerate() {
        let point = geo_to_vec(
            target_item.location.latitude,
            target_item.location.longitude,
        );
        if let Some((x, y, _depth)) = project_vec_camera(
            point,
            geometry.rotation,
            geometry.pitch,
            geometry.center_x,
            geometry.center_y,
            geometry.radius_x,
            geometry.radius_y,
        ) {
            let active = index == app.target_index();
            if active {
                // Center clean bullseye dot
                points.push(PointMarker {
                    x,
                    y,
                    radius_sq: 1.25,
                    color: theme.active_target,
                    priority: 4,
                });

                // Clean circular reticle ring
                rings.push(RingMarker {
                    x,
                    y,
                    radius: 2.8,
                    color: theme.active_target,
                    priority: 4,
                });

                // Subtle expanding radar pulse
                let ping_phase = (app.continuous_time().as_secs_f64() * 1.3).fract();
                let ping_radius = 2.8 + ping_phase * 3.5;
                let ping_fade = (1.0 - ping_phase).max(0.1);
                rings.push(RingMarker {
                    x,
                    y,
                    radius: ping_radius,
                    color: scale_color(theme.active_target, ping_fade * 0.70),
                    priority: 3,
                });
            } else {
                points.push(PointMarker {
                    x,
                    y,
                    radius_sq: 0.9,
                    color: theme.other_target,
                    priority: 3,
                });
            }
        }
    }

    (points, rings, segments)
}

fn dot_sample(
    x: usize,
    y: usize,
    geometry: &GlobeGeometry,
    points: &[PointMarker],
    rings: &[RingMarker],
    segments: &[RouteSegment],
    theme: &ThemePalette,
) -> Option<DotSample> {
    let screen_x = x as f64 + 0.5;
    let screen_y = y as f64 + 0.5;

    let mut best_overlay: Option<DotSample> = None;

    // Check point markers (center bullseye, photon packet, origin)
    for p in points {
        let dist_sq = (screen_x - p.x).powi(2) + (screen_y - p.y).powi(2);
        if dist_sq <= p.radius_sq {
            let sample = DotSample {
                color: p.color,
                priority: p.priority,
            };
            if best_overlay.is_none_or(|b| sample.priority >= b.priority) {
                best_overlay = Some(sample);
            }
        }
    }

    // Check circular reticle rings
    for r in rings {
        let dist = (screen_x - r.x).hypot(screen_y - r.y);
        if (dist - r.radius).abs() <= 0.46 {
            let sample = DotSample {
                color: r.color,
                priority: r.priority,
            };
            if best_overlay.is_none_or(|b| sample.priority >= b.priority) {
                best_overlay = Some(sample);
            }
        }
    }

    // Check thin 1-dot hairline route line segments
    for seg in segments {
        let d_sq = dist_to_segment_squared(screen_x, screen_y, seg.x1, seg.y1, seg.x2, seg.y2);
        if d_sq <= 0.26 {
            let sample = DotSample {
                color: seg.color,
                priority: seg.priority,
            };
            if best_overlay.is_none_or(|b| sample.priority >= b.priority) {
                best_overlay = Some(sample);
            }
        }
    }

    if let Some(top) = best_overlay
        && top.priority >= 3
    {
        return Some(top);
    }

    let normalized_x = (screen_x - geometry.center_x) / geometry.radius_x;
    let normalized_y = (screen_y - geometry.center_y) / geometry.radius_y;
    let sphere_distance = normalized_x * normalized_x + normalized_y * normalized_y;

    // Inside globe sphere:
    if sphere_distance <= 1.0 {
        let normal_z = (1.0 - sphere_distance).sqrt();
        let yaw_x = normalized_x;
        let yaw_y = -normalized_y * geometry.pitch_cos + normal_z * geometry.pitch_sin;
        let yaw_z = normalized_y * geometry.pitch_sin + normal_z * geometry.pitch_cos;
        let world_normal = DVec3::new(
            yaw_x * geometry.rotation_cos + yaw_z * geometry.rotation_sin,
            yaw_y,
            -yaw_x * geometry.rotation_sin + yaw_z * geometry.rotation_cos,
        );

        let latitude = world_normal.y.asin().to_degrees();
        let longitude = world_normal.z.atan2(world_normal.x).to_degrees();

        let brightness =
            (world_normal.x * -0.437_529 + world_normal.y * 0.340_300 + world_normal.z * 0.893_153)
                .max(0.0)
                .mul_add(0.58, 0.42)
                .min(1.0);

        let map = map_sample(latitude, longitude);

        // High-accuracy world-space continuous dithering (locked to the rotating planet)
        let stipple = world_stipple(latitude, longitude);

        // Clean dark ocean (no random noise dots in the water)
        let (base_color, density) = if map.boundary {
            (theme.border, 0.98)
        } else if map.coast {
            (theme.coast, 0.98)
        } else if map.land {
            (theme.land, 0.85)
        } else {
            // Clean ocean
            (theme.ocean, 0.0)
        };

        if density > 0.0 && stipple <= density {
            let color = scale_color(base_color, brightness);
            let sample = DotSample { color, priority: 1 };
            if let Some(top) = best_overlay
                && top.priority >= sample.priority
            {
                return Some(top);
            }
            return Some(sample);
        }

        if let Some(top) = best_overlay {
            return Some(top);
        }

        return None;
    }

    // Atmospheric limb glow (1.0 < sphere_distance <= 1.05)
    if sphere_distance <= 1.05 {
        let limb_intensity = (1.05 - sphere_distance) / 0.05;
        let halo_stipple = ((x.wrapping_mul(41) + y.wrapping_mul(19)) % 100) as f64 / 100.0;
        if halo_stipple < limb_intensity * 0.55 {
            return Some(DotSample {
                color: scale_color(theme.atmosphere, limb_intensity * 0.85),
                priority: 1,
            });
        }
    }

    best_overlay
}

fn dist_to_segment_squared(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return (px - x1).powi(2) + (py - y1).powi(2);
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    (px - proj_x).powi(2) + (py - proj_y).powi(2)
}

pub fn world_stipple(latitude: f64, longitude: f64) -> f64 {
    let lat_term = ((latitude + 90.0) * 12.9898).sin();
    let lon_term = ((longitude + 180.0) * 78.233).cos();
    ((lat_term * 43758.5453 + lon_term * 24634.63) % 1.0).abs()
}

type LandGeometry = Vec<Vec<Vec<(f64, f64)>>>;

struct LandShape {
    rings: Vec<Vec<(f64, f64)>>,
    min_latitude: f64,
    max_latitude: f64,
    min_longitude: f64,
    max_longitude: f64,
}

#[derive(Clone, Copy)]
struct MapSample {
    land: bool,
    coast: bool,
    boundary: bool,
}

struct GlobeMasks {
    land: Vec<u8>,
    coast: Vec<u8>,
    boundary: Vec<u8>,
}

pub fn geo_to_vec(latitude: f64, longitude: f64) -> DVec3 {
    let lat = latitude.to_radians();
    let lon = longitude.to_radians();
    DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin())
}

fn project_vec_camera(
    point: DVec3,
    rotation: f64,
    pitch: f64,
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
) -> Option<(f64, f64, f64)> {
    let yaw_x = point.x * rotation.cos() - point.z * rotation.sin();
    let yaw_z = point.x * rotation.sin() + point.z * rotation.cos();
    let rotated_x = yaw_x;
    let rotated_y = point.y * pitch.cos() - yaw_z * pitch.sin();
    let rotated_z = point.y * pitch.sin() + yaw_z * pitch.cos();
    if rotated_z < 0.0 {
        return None;
    }
    Some((
        center_x + rotated_x * radius_x,
        center_y - rotated_y * radius_y,
        rotated_z,
    ))
}

pub fn land_mask(latitude: f64, longitude: f64) -> bool {
    map_sample(latitude, longitude).land
}

fn map_sample(latitude: f64, longitude: f64) -> MapSample {
    let masks = globe_masks();
    let index = mask_index(latitude, longitude);
    MapSample {
        land: masks.land[index] != 0,
        coast: masks.coast[index] != 0,
        boundary: masks.boundary[index] != 0,
    }
}

fn globe_masks() -> &'static GlobeMasks {
    static MASKS: OnceLock<GlobeMasks> = OnceLock::new();
    MASKS.get_or_init(build_globe_masks)
}

struct SpatialGrid {
    shapes: Vec<LandShape>,
    cells: Vec<Vec<usize>>,
}

fn spatial_grid() -> &'static SpatialGrid {
    static GRID: OnceLock<SpatialGrid> = OnceLock::new();
    GRID.get_or_init(|| {
        let geometry: LandGeometry = serde_json::from_str(include_str!("../data/ne_50m_land.json"))
            .expect("embedded Natural Earth 50m land geometry must be valid JSON");
        let shapes: Vec<LandShape> = geometry
            .into_iter()
            .filter_map(|rings| {
                let exterior = rings.first()?;
                let (min_longitude, max_longitude) = exterior
                    .iter()
                    .map(|(longitude, _)| *longitude)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                        (min.min(value), max.max(value))
                    });
                let (min_latitude, max_latitude) = exterior
                    .iter()
                    .map(|(_, latitude)| *latitude)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                        (min.min(value), max.max(value))
                    });
                Some(LandShape {
                    rings,
                    min_latitude,
                    max_latitude,
                    min_longitude,
                    max_longitude,
                })
            })
            .collect();

        let mut cells = vec![Vec::new(); GRID_ROWS * GRID_COLS];
        for (shape_idx, shape) in shapes.iter().enumerate() {
            let row_start = (((90.0 - shape.max_latitude) / 180.0) * GRID_ROWS as f64)
                .floor()
                .clamp(0.0, (GRID_ROWS - 1) as f64) as usize;
            let row_end = (((90.0 - shape.min_latitude) / 180.0) * GRID_ROWS as f64)
                .floor()
                .clamp(0.0, (GRID_ROWS - 1) as f64) as usize;

            let span_lon = shape.max_longitude - shape.min_longitude;
            if span_lon >= 360.0 {
                for r in row_start..=row_end {
                    for c in 0..GRID_COLS {
                        cells[r * GRID_COLS + c].push(shape_idx);
                    }
                }
            } else {
                let col_start = (((shape.min_longitude + 180.0).rem_euclid(360.0) / 360.0)
                    * GRID_COLS as f64)
                    .floor()
                    .min((GRID_COLS - 1) as f64) as usize;
                let col_end = (((shape.max_longitude + 180.0).rem_euclid(360.0) / 360.0)
                    * GRID_COLS as f64)
                    .floor()
                    .min((GRID_COLS - 1) as f64) as usize;

                for r in row_start..=row_end {
                    if col_start <= col_end {
                        for c in col_start..=col_end {
                            cells[r * GRID_COLS + c].push(shape_idx);
                        }
                    } else {
                        for c in col_start..GRID_COLS {
                            cells[r * GRID_COLS + c].push(shape_idx);
                        }
                        for c in 0..=col_end {
                            cells[r * GRID_COLS + c].push(shape_idx);
                        }
                    }
                }
            }
        }

        SpatialGrid { shapes, cells }
    })
}

fn boundary_geometry() -> &'static [Vec<(f64, f64)>] {
    static GEOMETRY: OnceLock<Vec<Vec<(f64, f64)>>> = OnceLock::new();
    GEOMETRY
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/ne_50m_boundaries.json"))
                .expect("embedded Natural Earth 50m boundary geometry must be valid JSON")
        })
        .as_slice()
}

fn build_globe_masks() -> GlobeMasks {
    let mut land = vec![0_u8; MASK_WIDTH * MASK_HEIGHT];
    for y in 0..MASK_HEIGHT {
        let latitude = 90.0 - (y as f64 + 0.5) * 180.0 / MASK_HEIGHT as f64;
        for x in 0..MASK_WIDTH {
            let longitude = -180.0 + (x as f64 + 0.5) * 360.0 / MASK_WIDTH as f64;
            if point_in_land(latitude, longitude) {
                land[y * MASK_WIDTH + x] = 1;
            }
        }
    }

    let mut coast = vec![0_u8; MASK_WIDTH * MASK_HEIGHT];
    for y in 0..MASK_HEIGHT {
        for x in 0..MASK_WIDTH {
            let index = y * MASK_WIDTH + x;
            if land[index] == 0 {
                continue;
            }
            let left = y * MASK_WIDTH + (x + MASK_WIDTH - 1) % MASK_WIDTH;
            let right = y * MASK_WIDTH + (x + 1) % MASK_WIDTH;
            let above = y.saturating_sub(1) * MASK_WIDTH + x;
            let below = (y + 1).min(MASK_HEIGHT - 1) * MASK_WIDTH + x;
            if land[left] == 0 || land[right] == 0 || land[above] == 0 || land[below] == 0 {
                coast[index] = 1;
            }
        }
    }

    let mut boundary = vec![0_u8; MASK_WIDTH * MASK_HEIGHT];
    for line in boundary_geometry() {
        for segment in line.windows(2) {
            rasterize_boundary_segment(&mut boundary, segment[0], segment[1]);
        }
    }

    GlobeMasks {
        land,
        coast,
        boundary,
    }
}

fn point_in_land(latitude: f64, longitude: f64) -> bool {
    let grid = spatial_grid();
    let row = (((90.0 - latitude.clamp(-90.0, 90.0)) / 180.0) * GRID_ROWS as f64)
        .floor()
        .clamp(0.0, (GRID_ROWS - 1) as f64) as usize;
    let col = (((longitude + 180.0).rem_euclid(360.0) / 360.0) * GRID_COLS as f64)
        .floor()
        .min((GRID_COLS - 1) as f64) as usize;

    let candidate_indices = &grid.cells[row * GRID_COLS + col];
    for &idx in candidate_indices {
        let shape = &grid.shapes[idx];
        let longitude_span = shape.max_longitude - shape.min_longitude;
        let longitude_matches = longitude_span > 180.0
            || (shape.min_longitude..=shape.max_longitude).contains(&longitude);
        if !(shape.min_latitude..=shape.max_latitude).contains(&latitude) || !longitude_matches {
            continue;
        }
        if let Some(exterior) = shape.rings.first()
            && point_in_polygon(longitude, latitude, exterior)
            && !shape
                .rings
                .iter()
                .skip(1)
                .any(|hole| point_in_polygon(longitude, latitude, hole))
        {
            return true;
        }
    }
    false
}

fn mask_index(latitude: f64, longitude: f64) -> usize {
    let wrapped_longitude = (longitude + 180.0).rem_euclid(360.0);
    let x = ((wrapped_longitude / 360.0) * MASK_WIDTH as f64)
        .floor()
        .min((MASK_WIDTH - 1) as f64) as usize;
    let y = (((90.0 - latitude.clamp(-90.0, 90.0)) / 180.0) * MASK_HEIGHT as f64)
        .floor()
        .min((MASK_HEIGHT - 1) as f64) as usize;
    y * MASK_WIDTH + x
}

fn rasterize_boundary_segment(mask: &mut [u8], start: (f64, f64), end: (f64, f64)) {
    let (start_longitude, start_latitude) = start;
    let (end_longitude, end_latitude) = end;
    let mut longitude_delta = end_longitude - start_longitude;
    if longitude_delta > 180.0 {
        longitude_delta -= 360.0;
    } else if longitude_delta < -180.0 {
        longitude_delta += 360.0;
    }
    let latitude_delta = end_latitude - start_latitude;
    let steps = (longitude_delta.abs() * MASK_WIDTH as f64 / 360.0)
        .max(latitude_delta.abs() * MASK_HEIGHT as f64 / 180.0)
        .ceil()
        .max(1.0) as usize;

    for step in 0..=steps {
        let amount = step as f64 / steps as f64;
        let longitude = start_longitude + longitude_delta * amount;
        let latitude = start_latitude + latitude_delta * amount;
        mark_boundary_point(mask, latitude, longitude);
    }
}

fn mark_boundary_point(mask: &mut [u8], latitude: f64, longitude: f64) {
    let wrapped_longitude = (longitude + 180.0).rem_euclid(360.0);
    let center_x = ((wrapped_longitude / 360.0) * MASK_WIDTH as f64).floor() as isize;
    let center_y =
        (((90.0 - latitude.clamp(-90.0, 90.0)) / 180.0) * MASK_HEIGHT as f64).floor() as isize;
    for y_offset in -1_isize..=1_isize {
        for x_offset in -1_isize..=1_isize {
            if x_offset.abs() + y_offset.abs() > 1 {
                continue;
            }
            let x = (center_x + x_offset).rem_euclid(MASK_WIDTH as isize) as usize;
            let y = (center_y + y_offset).clamp(0, MASK_HEIGHT as isize - 1) as usize;
            mask[y * MASK_WIDTH + x] = 1;
        }
    }
}

fn point_in_polygon(x: f64, y: f64, polygon: &[(f64, f64)]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let (current_x, current_y) = polygon[current];
        let (previous_x, previous_y) = polygon[previous];
        let crosses = (current_y > y) != (previous_y > y);
        if crosses {
            let intersection =
                (previous_x - current_x) * (y - current_y) / (previous_y - current_y) + current_x;
            if x < intersection {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn interpolate_arc(start: DVec3, end: DVec3, amount: f64) -> DVec3 {
    let cosine = start.dot(end).clamp(-1.0, 1.0);
    let angle = cosine.acos();
    if angle < 0.000_001 {
        return start.lerp(end, amount).normalize();
    }
    let sine = angle.sin();
    (start * ((1.0 - amount) * angle).sin() / sine + end * (amount * angle).sin() / sine)
        .normalize()
}

fn braille_glyph(bits: u8) -> char {
    char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
}

fn scale_color(color: [u8; 3], brightness: f64) -> [u8; 3] {
    [
        (f64::from(color[0]) * brightness) as u8,
        (f64::from(color[1]) * brightness) as u8,
        (f64::from(color[2]) * brightness) as u8,
    ]
}

fn to_color(rgb: [u8; 3]) -> Color {
    Color::Rgb(rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geographic_origin_is_on_unit_sphere() {
        let point = geo_to_vec(0.0, 0.0);
        assert!((point.length() - 1.0).abs() < f64::EPSILON);
        assert!((point.x - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn target_focus_heading_places_city_marker_on_visible_hemisphere() {
        let topology =
            crate::model::Topology::from_json(include_str!("../data/demo-topology.json"))
                .expect("fixture should parse");
        let target = &topology.targets[0];
        let rotation = std::f64::consts::PI / 2.0 - target.location.longitude.to_radians();
        let projected = project_vec_camera(
            geo_to_vec(target.location.latitude, target.location.longitude),
            rotation,
            target.location.latitude.to_radians(),
            10.0,
            10.0,
            5.0,
            5.0,
        )
        .expect("focused target should be visible");
        assert!((projected.0 - 10.0).abs() < 0.001);
        assert!((projected.1 - 10.0).abs() < 0.001);
        assert!(projected.2 > 0.99);
    }

    #[test]
    fn cached_masks_include_coasts_and_country_boundaries() {
        let masks = globe_masks();
        assert_eq!(masks.land.len(), MASK_WIDTH * MASK_HEIGHT);
        assert!(masks.land.iter().any(|value| *value != 0));
        assert!(masks.coast.iter().any(|value| *value != 0));
        assert!(masks.boundary.iter().any(|value| *value != 0));
    }

    #[test]
    fn braille_dot_mapping_produces_unicode_braille() {
        let all_dots = BRAILLE_DOTS
            .iter()
            .fold(0_u8, |bits, (_, _, mask)| bits | mask);
        assert_eq!(all_dots, 0xff);
        assert_eq!(braille_glyph(all_dots), '\u{28ff}');
    }

    #[test]
    fn arc_interpolation_stays_on_unit_sphere() {
        let point = interpolate_arc(geo_to_vec(0.0, 0.0), geo_to_vec(0.0, 90.0), 0.5);
        assert!((point.length() - 1.0).abs() < 0.000_001);
        assert!((point.x - point.z).abs() < 0.000_001);
    }

    #[test]
    fn land_mask_separates_europe_from_mid_atlantic() {
        assert!(land_mask(52.37, 4.90)); // Amsterdam
        assert!(land_mask(51.50, -0.12)); // London, UK
        assert!(land_mask(53.35, -6.26)); // Dublin, Ireland
        assert!(land_mask(35.68, 139.65)); // Tokyo, Japan
        assert!(land_mask(32.08, 34.78)); // Tel Aviv, Israel
        assert!(land_mask(39.04, -77.49)); // Ashburn, VA
        assert!(land_mask(-33.86, 151.20)); // Sydney, Australia
        assert!(land_mask(-23.55, -46.63)); // São Paulo, Brazil
        assert!(!land_mask(0.0, -30.0)); // Mid-Atlantic Ocean
        assert!(!land_mask(0.0, 160.0)); // Pacific Ocean
        assert!(!land_mask(-50.0, 0.0)); // South Atlantic Ocean
    }

    #[test]
    fn world_stipple_is_deterministic_and_bounded() {
        let s1 = world_stipple(52.37, 4.90);
        let s2 = world_stipple(52.37, 4.90);
        assert_eq!(s1, s2);
        assert!((0.0..=1.0).contains(&s1));
    }

    #[test]
    fn theme_palettes_are_defined_for_all_variants() {
        for theme_id in [
            ThemeId::CyberOrbital,
            ThemeId::TacticalRadar,
            ThemeId::MinimalAtlas,
            ThemeId::AmberCrt,
            ThemeId::DeepSpace,
        ] {
            let palette = get_theme(theme_id);
            assert!(!palette.name.is_empty());
        }
    }
}
