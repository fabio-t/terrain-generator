//! Particle-based hydraulic erosion.
//!
//! Simulates rain droplets rolling downhill: each droplet picks up sediment
//! proportionally to its speed and water volume, carries it downstream, and
//! deposits it when it slows down or reaches a pit. Over many droplets this
//! carves realistic valleys, ridges and drainage networks into the raw
//! fractal heightmap.
//!
//! The algorithm follows Hans Theobald Beyer's "Implementation of a method
//! for hydraulic erosion" (2015), the same model popularised by Sebastian
//! Lague's erosion simulations.

use crate::grid::Grid;
use crate::rng::Pcg32;
use crate::ErosionParams;

/// The state a droplet carries between simulation steps.
struct Droplet {
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    speed: f32,
    water: f32,
    sediment: f32,
}

/// Erodes the heightmap (values in `[0, 1]`) in place. `seed` makes droplet
/// placement deterministic.
pub fn erode(heightmap: &mut Grid<f32>, params: &ErosionParams, seed: u32) {
    if heightmap.width() < 3 || heightmap.height() < 3 {
        return;
    }

    let droplets = params.droplets.unwrap_or_else(|| heightmap.len());
    let brush = Brush::new(params.radius);

    let mut rng = Pcg32::new(seed as u64, 0x9e3779b97f4a7c15);

    // droplet positions must stay inside [0, width-2) x [0, height-2) so
    // that bilinear interpolation always has a valid 2x2 cell
    let max_x = (heightmap.width() - 1) as f32 - 1e-4;
    let max_y = (heightmap.height() - 1) as f32 - 1e-4;

    for _ in 0..droplets {
        let mut drop = Droplet {
            x: rng.next_f32() * max_x,
            y: rng.next_f32() * max_y,
            dir_x: 0.0,
            dir_y: 0.0,
            speed: 1.0,
            water: 1.0,
            sediment: 0.0,
        };

        for _ in 0..params.max_lifetime {
            if !step_droplet(heightmap, params, &brush, &mut drop, max_x, max_y) {
                break;
            }
        }
    }
}

/// Advances a droplet by one step, eroding or depositing along the way.
/// Returns `false` when the droplet dies (stops or leaves the map).
fn step_droplet(
    heightmap: &mut Grid<f32>,
    params: &ErosionParams,
    brush: &Brush,
    drop: &mut Droplet,
    max_x: f32,
    max_y: f32,
) -> bool {
    let xi = drop.x as usize;
    let yi = drop.y as usize;
    let fx = drop.x - xi as f32;
    let fy = drop.y - yi as f32;

    let (h, gx, gy) = height_and_gradient(heightmap, xi, yi, fx, fy);

    // blend the downhill gradient with the droplet's momentum
    drop.dir_x = drop.dir_x * params.inertia - gx * (1.0 - params.inertia);
    drop.dir_y = drop.dir_y * params.inertia - gy * (1.0 - params.inertia);

    let len = (drop.dir_x * drop.dir_x + drop.dir_y * drop.dir_y).sqrt();
    if len <= f32::EPSILON {
        // flat cell and no momentum: the droplet dies here
        return false;
    }
    drop.dir_x /= len;
    drop.dir_y /= len;

    drop.x += drop.dir_x;
    drop.y += drop.dir_y;

    if drop.x < 0.0 || drop.x >= max_x || drop.y < 0.0 || drop.y >= max_y {
        return false;
    }

    let new_h = sample_height(heightmap, drop.x, drop.y);
    let dh = new_h - h;

    // carrying capacity grows with slope, speed and water volume
    let capacity = (-dh).max(params.min_capacity) * drop.speed * drop.water * params.capacity;

    if drop.sediment > capacity || dh > 0.0 {
        deposit(heightmap, drop, capacity, dh, params, xi, yi, fx, fy);
    } else {
        erode_around(heightmap, drop, capacity, dh, params, brush, xi, yi);
    }

    drop.speed = (drop.speed * drop.speed - dh * params.gravity)
        .max(0.0)
        .sqrt();
    drop.water *= 1.0 - params.evaporate_speed;

    true
}

/// Drops part of the carried sediment on the cell the droplet just left,
/// bilinearly distributed. When climbing, deposits at most enough to fill
/// the pit behind the droplet.
#[allow(clippy::too_many_arguments)]
fn deposit(
    heightmap: &mut Grid<f32>,
    drop: &mut Droplet,
    capacity: f32,
    dh: f32,
    params: &ErosionParams,
    xi: usize,
    yi: usize,
    fx: f32,
    fy: f32,
) {
    let amount = if dh > 0.0 {
        drop.sediment.min(dh)
    } else {
        (drop.sediment - capacity) * params.deposit_speed
    };
    drop.sediment -= amount;

    heightmap[(xi, yi)] += amount * (1.0 - fx) * (1.0 - fy);
    heightmap[(xi + 1, yi)] += amount * fx * (1.0 - fy);
    heightmap[(xi, yi + 1)] += amount * (1.0 - fx) * fy;
    heightmap[(xi + 1, yi + 1)] += amount * fx * fy;
}

/// Removes material from the terrain around the droplet using the brush,
/// never digging deeper than the height difference just travelled (which
/// would carve pits).
#[allow(clippy::too_many_arguments)]
fn erode_around(
    heightmap: &mut Grid<f32>,
    drop: &mut Droplet,
    capacity: f32,
    dh: f32,
    params: &ErosionParams,
    brush: &Brush,
    xi: usize,
    yi: usize,
) {
    let amount = ((capacity - drop.sediment) * params.erode_speed).min(-dh);

    for &(ox, oy, w) in &brush.offsets {
        let bx = xi as i32 + ox;
        let by = yi as i32 + oy;
        if bx < 0 || by < 0 || bx >= heightmap.width() as i32 || by >= heightmap.height() as i32 {
            continue;
        }
        let cell = &mut heightmap[(bx as usize, by as usize)];
        let taken = cell.min(amount * w);
        *cell -= taken;
        drop.sediment += taken;
    }
}

/// Bilinearly interpolated height and gradient at a droplet position.
fn height_and_gradient(
    heightmap: &Grid<f32>,
    xi: usize,
    yi: usize,
    fx: f32,
    fy: f32,
) -> (f32, f32, f32) {
    let h00 = heightmap[(xi, yi)];
    let h10 = heightmap[(xi + 1, yi)];
    let h01 = heightmap[(xi, yi + 1)];
    let h11 = heightmap[(xi + 1, yi + 1)];

    let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

    let h = h00 * (1.0 - fx) * (1.0 - fy)
        + h10 * fx * (1.0 - fy)
        + h01 * (1.0 - fx) * fy
        + h11 * fx * fy;

    (h, gx, gy)
}

fn sample_height(heightmap: &Grid<f32>, px: f32, py: f32) -> f32 {
    let xi = px as usize;
    let yi = py as usize;
    let fx = px - xi as f32;
    let fy = py - yi as f32;
    height_and_gradient(heightmap, xi, yi, fx, fy).0
}

/// Precomputed erosion brush: cell offsets within `radius` and their
/// normalised weights, so eroded material is removed from a smooth area
/// rather than a single cell (which would leave spiky artefacts).
struct Brush {
    offsets: Vec<(i32, i32, f32)>,
}

impl Brush {
    fn new(radius: usize) -> Self {
        let r = radius.max(1) as i32;
        let mut offsets = Vec::new();
        let mut sum = 0.0f32;
        for oy in -r..=r {
            for ox in -r..=r {
                let d = ((ox * ox + oy * oy) as f32).sqrt();
                if d <= r as f32 {
                    let w = 1.0 - d / r as f32;
                    if w > 0.0 {
                        offsets.push((ox, oy, w));
                        sum += w;
                    }
                }
            }
        }
        for (_, _, w) in offsets.iter_mut() {
            *w /= sum;
        }
        Self { offsets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slope_map(n: usize) -> Grid<f32> {
        let mut map = Grid::new(n, n, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                map[(x, y)] = x as f32 / (n - 1) as f32;
            }
        }
        map
    }

    #[test]
    fn erosion_changes_terrain() {
        let mut map = slope_map(64);
        let before = map.clone();
        erode(&mut map, &ErosionParams::default(), 1);
        assert_ne!(map, before);
    }

    #[test]
    fn erosion_is_deterministic() {
        let mut a = slope_map(64);
        let mut b = slope_map(64);
        erode(&mut a, &ErosionParams::default(), 5);
        erode(&mut b, &ErosionParams::default(), 5);
        assert_eq!(a, b);
    }

    #[test]
    fn erosion_roughly_conserves_mass() {
        let mut map = slope_map(128);
        let before: f32 = map.iter().sum();
        erode(&mut map, &ErosionParams::default(), 2);
        let after: f32 = map.iter().sum();
        // sediment leaves the map with droplets that exit the border, so
        // mass can only decrease, and not by much
        assert!(after <= before);
        assert!(after > before * 0.9);
    }

    #[test]
    fn no_negative_heights() {
        let mut map = slope_map(64);
        erode(&mut map, &ErosionParams::default(), 3);
        assert!(map.iter().all(|v| *v >= 0.0));
    }

    #[test]
    fn brush_weights_are_normalised() {
        for radius in 1..=5 {
            let brush = Brush::new(radius);
            let sum: f32 = brush.offsets.iter().map(|(_, _, w)| w).sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }
    }
}
