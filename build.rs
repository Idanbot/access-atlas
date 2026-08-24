use serde::Deserialize;
use std::{env, fs::File, io::Write, path::Path};

const MASK_WIDTH: usize = 1440;
const MASK_HEIGHT: usize = 720;
const GRID_ROWS: usize = 72;
const GRID_COLS: usize = 144;

type LandGeometry = Vec<Vec<Vec<(f64, f64)>>>;

#[derive(Deserialize)]
#[serde(transparent)]
struct LandData(LandGeometry);

struct PreparedShape {
    exterior: Vec<(f64, f64)>,
    holes: Vec<Vec<(f64, f64)>>,
    min_lon: f64,
    max_lon: f64,
    min_lat: f64,
    max_lat: f64,
}

fn main() {
    println!("cargo:rerun-if-changed=data/ne_50m_land.json");
    println!("cargo:rerun-if-changed=data/ne_50m_boundaries.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let dest_path = Path::new(&out_dir).join("ne_50m_masks.bin");

    let land_raw = include_str!("data/ne_50m_land.json");
    let land_data: LandData =
        serde_json::from_str(land_raw).expect("data/ne_50m_land.json must be valid JSON");

    let boundaries_raw = include_str!("data/ne_50m_boundaries.json");
    let boundaries: Vec<Vec<(f64, f64)>> = serde_json::from_str(boundaries_raw)
        .expect("data/ne_50m_boundaries.json must be valid JSON");

    let mut prepared: Vec<PreparedShape> = Vec::with_capacity(land_data.0.len());
    let mut grid: Vec<Vec<usize>> = vec![Vec::new(); GRID_ROWS * GRID_COLS];

    for (idx, mut rings) in land_data.0.into_iter().enumerate() {
        if rings.is_empty() {
            continue;
        }
        let exterior = rings.remove(0);
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        for &(lon, lat) in &exterior {
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
        }

        let r_start = (((90.0 - max_lat) / 180.0 * GRID_ROWS as f64).floor() as isize)
            .clamp(0, GRID_ROWS as isize - 1) as usize;
        let r_end = (((90.0 - min_lat) / 180.0 * GRID_ROWS as f64).floor() as isize)
            .clamp(0, GRID_ROWS as isize - 1) as usize;

        if max_lon - min_lon >= 360.0 {
            for r in r_start..=r_end {
                for c in 0..GRID_COLS {
                    grid[r * GRID_COLS + c].push(idx);
                }
            }
        } else {
            let c_start = ((((min_lon + 180.0).rem_euclid(360.0)) / 360.0 * GRID_COLS as f64)
                .floor() as usize)
                % GRID_COLS;
            let c_end = ((((max_lon + 180.0).rem_euclid(360.0)) / 360.0 * GRID_COLS as f64).floor()
                as usize)
                % GRID_COLS;

            for r in r_start..=r_end {
                if c_start <= c_end {
                    for c in c_start..=c_end {
                        grid[r * GRID_COLS + c].push(idx);
                    }
                } else {
                    for c in c_start..GRID_COLS {
                        grid[r * GRID_COLS + c].push(idx);
                    }
                    for c in 0..=c_end {
                        grid[r * GRID_COLS + c].push(idx);
                    }
                }
            }
        }

        prepared.push(PreparedShape {
            exterior,
            holes: rings,
            min_lon,
            max_lon,
            min_lat,
            max_lat,
        });
    }

    let is_land = |lat: f64, lon: f64| -> bool {
        let r = (((90.0 - lat) / 180.0 * GRID_ROWS as f64).floor() as isize)
            .clamp(0, GRID_ROWS as isize - 1) as usize;
        let c = (((lon + 180.0).rem_euclid(360.0) / 360.0 * GRID_COLS as f64).floor() as usize)
            % GRID_COLS;

        for &idx in &grid[r * GRID_COLS + c] {
            let shape = &prepared[idx];
            if lat < shape.min_lat || lat > shape.max_lat {
                continue;
            }
            if shape.max_lon - shape.min_lon < 360.0 && (lon < shape.min_lon || lon > shape.max_lon)
            {
                continue;
            }
            if point_in_polygon(lon, lat, &shape.exterior)
                && !shape.holes.iter().any(|h| point_in_polygon(lon, lat, h))
            {
                return true;
            }
        }
        false
    };

    let mut land_bits = vec![0_u8; (MASK_WIDTH * MASK_HEIGHT).div_ceil(8)];
    for y in 0..MASK_HEIGHT {
        let lat = 90.0 - (y as f64 + 0.5) * 180.0 / MASK_HEIGHT as f64;
        for x in 0..MASK_WIDTH {
            let lon = -180.0 + (x as f64 + 0.5) * 360.0 / MASK_WIDTH as f64;
            if is_land(lat, lon) {
                let idx = y * MASK_WIDTH + x;
                land_bits[idx / 8] |= 1 << (idx % 8);
            }
        }
    }

    let mut coast_bits = vec![0_u8; (MASK_WIDTH * MASK_HEIGHT).div_ceil(8)];
    for y in 0..MASK_HEIGHT {
        for x in 0..MASK_WIDTH {
            let idx = y * MASK_WIDTH + x;
            if (land_bits[idx / 8] & (1 << (idx % 8))) == 0 {
                continue;
            }
            let left = y * MASK_WIDTH + (x + MASK_WIDTH - 1) % MASK_WIDTH;
            let right = y * MASK_WIDTH + (x + 1) % MASK_WIDTH;
            let above = y.saturating_sub(1) * MASK_WIDTH + x;
            let below = (y + 1).min(MASK_HEIGHT - 1) * MASK_WIDTH + x;

            let is_coast = (land_bits[left / 8] & (1 << (left % 8))) == 0
                || (land_bits[right / 8] & (1 << (right % 8))) == 0
                || (land_bits[above / 8] & (1 << (above % 8))) == 0
                || (land_bits[below / 8] & (1 << (below % 8))) == 0;
            if is_coast {
                coast_bits[idx / 8] |= 1 << (idx % 8);
            }
        }
    }

    let mut boundary_bits = vec![0_u8; (MASK_WIDTH * MASK_HEIGHT).div_ceil(8)];
    for line in boundaries {
        for w in line.windows(2) {
            let (lon1, lat1) = w[0];
            let (lon2, lat2) = w[1];
            let mut d_lon = lon2 - lon1;
            if d_lon > 180.0 {
                d_lon -= 360.0;
            } else if d_lon < -180.0 {
                d_lon += 360.0;
            }
            let d_lat = lat2 - lat1;
            let steps = (((d_lon.abs() * MASK_WIDTH as f64 / 360.0)
                .max(d_lat.abs() * MASK_HEIGHT as f64 / 180.0)
                .max(1.0))
                * 1.5)
                .ceil() as usize;

            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let lon_s = lon1 + d_lon * t;
                let lat_s = lat1 + d_lat * t;
                let bx = (((lon_s + 180.0).rem_euclid(360.0) / 360.0 * MASK_WIDTH as f64).floor()
                    as usize)
                    % MASK_WIDTH;
                let by = (((90.0 - lat_s.clamp(-90.0, 90.0)) / 180.0 * MASK_HEIGHT as f64).floor()
                    as usize)
                    .min(MASK_HEIGHT - 1);

                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx * dx + dy * dy > 1 {
                            continue;
                        }
                        let nx = ((bx as isize + dx).rem_euclid(MASK_WIDTH as isize)) as usize;
                        let ny = ((by as isize + dy).clamp(0, MASK_HEIGHT as isize - 1)) as usize;
                        let b_idx = ny * MASK_WIDTH + nx;
                        boundary_bits[b_idx / 8] |= 1 << (b_idx % 8);
                    }
                }
            }
        }
    }

    let mut out_file = File::create(dest_path).expect("failed to create destination file");
    out_file.write_all(&land_bits).unwrap();
    out_file.write_all(&coast_bits).unwrap();
    out_file.write_all(&boundary_bits).unwrap();
}

fn point_in_polygon(x: f64, y: f64, poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (pix, piy) = poly[i];
        let (pjx, pjy) = poly[j];
        if (piy > y) != (pjy > y) && x < (pjx - pix) * (y - piy) / (pjy - piy) + pix {
            inside = !inside;
        }
        j = i;
    }
    inside
}
