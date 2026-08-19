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

use crate::rng::Pcg32;
use crate::ErosionParams;

/// Erodes `heightmap` (row-major, `width * height`, values in `[0, 1]`)
/// in place. `seed` makes droplet placement deterministic.
pub fn erode(
    heightmap: &mut [f32],
    width: usize,
    height: usize,
    params: &ErosionParams,
    seed: u32,
) {
    assert_eq!(heightmap.len(), width * height);
    if width < 3 || height < 3 {
        return;
    }

    let droplets = params.droplets.unwrap_or(width * height);
    let brush = Brush::new(params.radius);

    let mut rng = Pcg32::new(seed as u64, 0x9e3779b97f4a7c15);

    // droplet positions must stay inside [0, width-2) x [0, height-2) so
    // that bilinear interpolation always has a valid 2x2 cell
    let max_x = (width - 1) as f32 - 1e-4;
    let max_y = (height - 1) as f32 - 1e-4;

    for _ in 0..droplets {
        let mut px = rng.next_f32() * max_x;
        let mut py = rng.next_f32() * max_y;
        let mut dir_x = 0.0f32;
        let mut dir_y = 0.0f32;
        let mut speed = 1.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;

        for _ in 0..params.max_lifetime {
            let xi = px as usize;
            let yi = py as usize;
            let fx = px - xi as f32;
            let fy = py - yi as f32;

            let (h, gx, gy) = height_and_gradient(heightmap, width, xi, yi, fx, fy);

            // blend the downhill gradient with the droplet's momentum
            dir_x = dir_x * params.inertia - gx * (1.0 - params.inertia);
            dir_y = dir_y * params.inertia - gy * (1.0 - params.inertia);

            let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
            if len <= f32::EPSILON {
                // flat cell and no momentum: the droplet dies here
                break;
            }
            dir_x /= len;
            dir_y /= len;

            px += dir_x;
            py += dir_y;

            if px < 0.0 || px >= max_x || py < 0.0 || py >= max_y {
                break;
            }

            let new_h = sample_height(heightmap, width, px, py);
            let dh = new_h - h;

            // carrying capacity grows with slope, speed and water volume
            let capacity = (-dh).max(params.min_capacity) * speed * water * params.capacity;

            if sediment > capacity || dh > 0.0 {
                // moving uphill or over capacity: deposit at the old cell.
                // when climbing, deposit at most enough to fill the pit
                let deposit = if dh > 0.0 {
                    sediment.min(dh)
                } else {
                    (sediment - capacity) * params.deposit_speed
                };
                sediment -= deposit;

                heightmap[yi * width + xi] += deposit * (1.0 - fx) * (1.0 - fy);
                heightmap[yi * width + xi + 1] += deposit * fx * (1.0 - fy);
                heightmap[(yi + 1) * width + xi] += deposit * (1.0 - fx) * fy;
                heightmap[(yi + 1) * width + xi + 1] += deposit * fx * fy;
            } else {
                // erode from the terrain around the droplet, never digging
                // deeper than the height difference just travelled (which
                // would carve pits)
                let erode_amount = ((capacity - sediment) * params.erode_speed).min(-dh);

                for (ox, oy, w) in brush.offsets.iter() {
                    let bx = xi as i32 + ox;
                    let by = yi as i32 + oy;
                    if bx < 0 || by < 0 || bx >= width as i32 || by >= height as i32 {
                        continue;
                    }
                    let idx = by as usize * width + bx as usize;
                    let delta = erode_amount * w;
                    let taken = heightmap[idx].min(delta);
                    heightmap[idx] -= taken;
                    sediment += taken;
                }
            }

            speed = (speed * speed - dh * params.gravity).max(0.0).sqrt();
            water *= 1.0 - params.evaporate_speed;
        }
    }
}

/// Bilinearly interpolated height and gradient at a droplet position.
fn height_and_gradient(
    heightmap: &[f32],
    width: usize,
    xi: usize,
    yi: usize,
    fx: f32,
    fy: f32,
) -> (f32, f32, f32) {
    let h00 = heightmap[yi * width + xi];
    let h10 = heightmap[yi * width + xi + 1];
    let h01 = heightmap[(yi + 1) * width + xi];
    let h11 = heightmap[(yi + 1) * width + xi + 1];

    let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

    let h = h00 * (1.0 - fx) * (1.0 - fy)
        + h10 * fx * (1.0 - fy)
        + h01 * (1.0 - fx) * fy
        + h11 * fx * fy;

    (h, gx, gy)
}

fn sample_height(heightmap: &[f32], width: usize, px: f32, py: f32) -> f32 {
    let xi = px as usize;
    let yi = py as usize;
    let fx = px - xi as f32;
    let fy = py - yi as f32;
    height_and_gradient(heightmap, width, xi, yi, fx, fy).0
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

    fn slope_map(n: usize) -> Vec<f32> {
        let mut map = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                map[y * n + x] = x as f32 / (n - 1) as f32;
            }
        }
        map
    }

    #[test]
    fn erosion_changes_terrain() {
        let mut map = slope_map(64);
        let before = map.clone();
        erode(&mut map, 64, 64, &ErosionParams::default(), 1);
        assert_ne!(map, before);
    }

    #[test]
    fn erosion_is_deterministic() {
        let mut a = slope_map(64);
        let mut b = slope_map(64);
        erode(&mut a, 64, 64, &ErosionParams::default(), 5);
        erode(&mut b, 64, 64, &ErosionParams::default(), 5);
        assert_eq!(a, b);
    }

    #[test]
    fn erosion_roughly_conserves_mass() {
        let mut map = slope_map(128);
        let before: f32 = map.iter().sum();
        erode(&mut map, 128, 128, &ErosionParams::default(), 2);
        let after: f32 = map.iter().sum();
        // sediment leaves the map with droplets that exit the border, so
        // mass can only decrease, and not by much
        assert!(after <= before);
        assert!(after > before * 0.9);
    }

    #[test]
    fn no_negative_heights() {
        let mut map = slope_map(64);
        erode(&mut map, 64, 64, &ErosionParams::default(), 3);
        assert!(map.iter().all(|v| *v >= 0.0));
    }
}
