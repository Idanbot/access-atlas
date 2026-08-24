use crate::app::App;
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

const BACKGROUND: [u8; 3] = [4, 9, 18];
const SEA: [u8; 3] = [27, 88, 126];
const LAND: [u8; 3] = [72, 153, 106];
const COAST: [u8; 3] = [205, 183, 91];
const BORDER: [u8; 3] = [170, 212, 156];
const ACTIVE: [u8; 3] = [255, 221, 113];
const OTHER: [u8; 3] = [94, 190, 232];
const ROUTE: [u8; 3] = [255, 132, 76];
const MASK_WIDTH: usize = 360;
const MASK_HEIGHT: usize = 180;
const BRAILLE_DOTS: [(usize, usize, u8); 8] = [
    (0, 0, 0b0000_0001),
    (0, 1, 0b0000_0010),
    (0, 2, 0b0000_0100),
    (1, 0, 0b0000_1000),
    (1, 1, 0b0001_0000),
    (1, 2, 0b0010_0000),
    (0, 3, 0b0100_0000),
    (1, 3, 0b1000_0000),
];

pub fn render(frame: &mut Frame, app: &App) {
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
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " ACCESS ATLAS ",
                Style::default().fg(Color::Rgb(120, 220, 255)),
            ),
            Span::raw("Braille-dot access topology demo"),
        ])),
        vertical[0],
    );

    let globe_block = Block::bordered()
        .title(" Globe: land / sea / target lock ")
        .border_style(Style::default().fg(Color::Rgb(40, 100, 145)));
    let globe_area = globe_block.inner(main[0]);
    frame.render_widget(globe_block, main[0]);
    frame.render_widget(GlobeWidget { app }, globe_area);

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
                Style::default().fg(Color::Rgb(255, 190, 90)),
            ),
        ]),
        Line::from(vec![
            Span::styled("binary: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &network_type.binary,
                Style::default().fg(Color::Rgb(180, 230, 255)),
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
                Style::default().fg(Color::Rgb(255, 221, 113)),
            ),
        ]),
        Line::from(vec![
            Span::styled("command: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &option.command,
                Style::default().fg(Color::Rgb(180, 230, 255)),
            ),
        ]),
        Line::from(""),
    ];
    for (index, row) in details.iter().enumerate() {
        let style = if index == app.detail_index() {
            Style::default()
                .fg(Color::Rgb(150, 255, 180))
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
            .block(Block::bordered().title(target_title))
            .wrap(Wrap { trim: false }),
        main[1],
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(" Tab/Shift+Tab: access option   Left/Right: target   Up/Down: detail"),
            Line::from(" Enter: no-op   q: exit"),
        ])
        .style(Style::default().fg(Color::DarkGray)),
        vertical[2],
    );
}

struct GlobeWidget<'a> {
    app: &'a App,
}

impl Widget for GlobeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_globe(area, buf, self.app);
    }
}

#[derive(Clone, Copy)]
struct Overlay {
    x: f64,
    y: f64,
    radius: f64,
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

fn render_globe(area: Rect, buf: &mut Buffer, app: &App) {
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
    let overlays = overlays(app, &geometry);

    for cell_y in 0..area.height as usize {
        for cell_x in 0..area.width as usize {
            let mut bits = 0_u8;
            let mut best_sample = DotSample {
                color: BACKGROUND,
                priority: 0,
            };
            // Six samples retain the Braille silhouette while avoiding the two middle
            // sub-pixels that add density without improving the small globe.
            for (dot_x, dot_y, mask) in RENDER_DOTS {
                let x = cell_x * 2 + dot_x;
                let y = cell_y * 4 + dot_y;
                if let Some(sample) = dot_sample(x, y, &geometry, &overlays) {
                    bits |= mask;
                    if sample.priority >= best_sample.priority {
                        best_sample = sample;
                    }
                }
            }
            let glyph = braille_glyph(bits);
            let style = if bits == 0 {
                Style::default().bg(to_color(BACKGROUND))
            } else {
                Style::default()
                    .fg(to_color(best_sample.color))
                    .bg(to_color(BACKGROUND))
            };
            buf.set_string(
                area.x + cell_x as u16,
                area.y + cell_y as u16,
                glyph.to_string(),
                style,
            );
        }
    }

    render_city_label(area, buf, app, &geometry);
}

const RENDER_DOTS: [(usize, usize, u8); 6] = [
    BRAILLE_DOTS[0],
    BRAILLE_DOTS[3],
    BRAILLE_DOTS[2],
    BRAILLE_DOTS[5],
    BRAILLE_DOTS[6],
    BRAILLE_DOTS[7],
];

fn render_city_label(area: Rect, buf: &mut Buffer, app: &App, geometry: &GlobeGeometry) {
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

    let label = format!(" {} ", target.location.city);
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
            .fg(to_color(ACTIVE))
            .bg(to_color(BACKGROUND))
            .add_modifier(Modifier::BOLD),
    );
}

fn overlays(app: &App, geometry: &GlobeGeometry) -> Vec<Overlay> {
    let mut overlays = Vec::with_capacity(48);
    let target = app.target();
    let origin = geo_to_vec(
        app.topology().origin.location.latitude,
        app.topology().origin.location.longitude,
    );
    let destination = geo_to_vec(target.location.latitude, target.location.longitude);

    if app.route_progress() > 0.0 {
        let steps = 40;
        let visible_steps = (steps as f32 * app.route_progress()).ceil() as usize;
        for step in 0..=visible_steps.min(steps) {
            let amount = step as f64 / steps as f64;
            if let Some((x, y, _depth)) = project_vec_camera(
                interpolate_arc(origin, destination, amount),
                geometry.rotation,
                geometry.pitch,
                geometry.center_x,
                geometry.center_y,
                geometry.radius_x,
                geometry.radius_y,
            ) {
                overlays.push(Overlay {
                    x,
                    y,
                    radius: 0.75,
                    color: ROUTE,
                    priority: 2,
                });
            }
        }
    }

    for (index, target) in app.topology().targets.iter().enumerate() {
        let point = geo_to_vec(target.location.latitude, target.location.longitude);
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
            overlays.push(Overlay {
                x,
                y,
                radius: if active { 2.25 } else { 1.0 },
                color: if active { ACTIVE } else { OTHER },
                priority: if active { 4 } else { 3 },
            });
        }
    }
    overlays
}

fn dot_sample(
    x: usize,
    y: usize,
    geometry: &GlobeGeometry,
    overlays: &[Overlay],
) -> Option<DotSample> {
    let screen_x = x as f64 + 0.5;
    let screen_y = y as f64 + 0.5;
    let overlay = overlays
        .iter()
        .filter_map(|overlay| {
            let distance_squared = (screen_x - overlay.x).powi(2) + (screen_y - overlay.y).powi(2);
            (distance_squared <= overlay.radius * overlay.radius).then_some(DotSample {
                color: overlay.color,
                priority: overlay.priority,
            })
        })
        .max_by_key(|sample| sample.priority);
    if overlay.is_some() {
        return overlay;
    }

    let surface = surface_at(screen_x, screen_y, geometry)?;
    let map = map_sample(surface.latitude, surface.longitude);
    let density = if map.boundary {
        0.98
    } else if map.coast {
        0.90
    } else if map.land {
        0.70
    } else {
        0.40
    };
    let stipple = ((x.wrapping_mul(37) + y.wrapping_mul(17)) % 100) as f64 / 100.0;
    if stipple > density {
        return None;
    }

    let base = if map.boundary {
        BORDER
    } else if map.coast {
        COAST
    } else if map.land {
        LAND
    } else {
        SEA
    };
    Some(DotSample {
        color: scale_color(base, surface.brightness),
        priority: 1,
    })
}

#[derive(Clone, Copy)]
struct SurfacePoint {
    latitude: f64,
    longitude: f64,
    brightness: f64,
}

type LandGeometry = Vec<Vec<Vec<(f64, f64)>>>;
type BoundaryGeometry = Vec<Vec<(f64, f64)>>;

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

fn surface_at(x: f64, y: f64, geometry: &GlobeGeometry) -> Option<SurfacePoint> {
    let normalized_x = (x - geometry.center_x) / geometry.radius_x;
    let normalized_y = (y - geometry.center_y) / geometry.radius_y;
    let sphere_distance = normalized_x * normalized_x + normalized_y * normalized_y;
    if sphere_distance > 1.0 {
        return None;
    }

    let normal_z = (1.0 - sphere_distance).sqrt();
    let yaw_x = normalized_x;
    let yaw_y = -normalized_y * geometry.pitch_cos + normal_z * geometry.pitch_sin;
    let yaw_z = normalized_y * geometry.pitch_sin + normal_z * geometry.pitch_cos;
    let world_normal = DVec3::new(
        yaw_x * geometry.rotation_cos + yaw_z * geometry.rotation_sin,
        yaw_y,
        -yaw_x * geometry.rotation_sin + yaw_z * geometry.rotation_cos,
    );
    let brightness =
        (world_normal.x * -0.437_529 + world_normal.y * 0.340_300 + world_normal.z * 0.893_153)
            .max(0.0)
            .mul_add(0.58, 0.42)
            .min(1.0);
    Some(SurfacePoint {
        latitude: world_normal.y.asin().to_degrees(),
        longitude: world_normal.z.atan2(world_normal.x).to_degrees(),
        brightness,
    })
}

pub fn geo_to_vec(latitude: f64, longitude: f64) -> DVec3 {
    let lat = latitude.to_radians();
    let lon = longitude.to_radians();
    DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin())
}

pub fn project_vec(
    point: DVec3,
    rotation: f64,
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
) -> Option<(f64, f64, f64)> {
    project_vec_camera(point, rotation, 0.0, center_x, center_y, radius_x, radius_y)
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

fn land_shapes() -> &'static [LandShape] {
    static SHAPES: OnceLock<Vec<LandShape>> = OnceLock::new();
    SHAPES
        .get_or_init(|| {
            let geometry: LandGeometry =
                serde_json::from_str(include_str!("../data/ne_110m_land.json"))
                    .expect("embedded Natural Earth land geometry must be valid JSON");
            geometry
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
                .collect()
        })
        .as_slice()
}

fn boundary_geometry() -> &'static [Vec<(f64, f64)>] {
    static GEOMETRY: OnceLock<BoundaryGeometry> = OnceLock::new();
    GEOMETRY
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/ne_110m_boundaries.json"))
                .expect("embedded Natural Earth boundary geometry must be valid JSON")
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
    land_shapes().iter().any(|shape| {
        let longitude_span = shape.max_longitude - shape.min_longitude;
        let longitude_matches = longitude_span > 180.0
            || (shape.min_longitude..=shape.max_longitude).contains(&longitude);
        if !(shape.min_latitude..=shape.max_latitude).contains(&latitude) || !longitude_matches {
            return false;
        }
        shape.rings.first().is_some_and(|exterior| {
            point_in_polygon(longitude, latitude, exterior)
                && !shape
                    .rings
                    .iter()
                    .skip(1)
                    .any(|hole| point_in_polygon(longitude, latitude, hole))
        })
    })
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
    fn projection_rotates_longitude_and_preserves_front_points() {
        let point = geo_to_vec(0.0, 0.0);
        let projected = project_vec(point, std::f64::consts::PI / 2.0, 10.0, 10.0, 5.0, 5.0)
            .expect("front point");
        assert!((projected.0 - 10.0).abs() < 0.001);
        assert!((projected.1 - 10.0).abs() < 0.001);
        assert!((projected.2 - 1.0).abs() < 0.001);
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
    fn land_mask_separates_europe_from_mid_atlantic() {
        assert!(land_mask(52.37, 4.90));
        assert!(!land_mask(0.0, -30.0));
        assert!(land_mask(39.04, -77.49));
        assert!(land_mask(35.68, 139.65));
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
}
