//! Base heightmap generation: fractal (fBm) OpenSimplex noise plus an
//! optional radial island mask with noisy coastlines.

use noise::{Fbm, MultiFractal, NoiseFn, OpenSimplex};

use crate::grid::Grid;
use crate::{IslandParams, NoiseParams};

/// Builds the base heightmap in `[0, 1]`, before erosion.
///
/// Coordinates are normalised by `max(width, height)`, so
/// [`NoiseParams::frequency`] reads as "number of large features across the
/// map" and is resolution-independent.
pub fn generate(
    width: usize,
    height: usize,
    seed: u32,
    noise: &NoiseParams,
    island: Option<&IslandParams>,
) -> Grid<f32> {
    let fbm = Fbm::<OpenSimplex>::new(seed)
        .set_octaves(noise.octaves)
        .set_frequency(noise.frequency)
        .set_lacunarity(noise.lacunarity)
        .set_persistence(noise.persistence);

    let scale = 1.0 / width.max(height) as f64;

    let mut map = Grid::new(width, height, 0.0f32);
    let mut min_v = f32::MAX;
    let mut max_v = f32::MIN;

    for y in 0..height {
        for x in 0..width {
            let v = fbm.get([x as f64 * scale, y as f64 * scale]) as f32;
            map[(x, y)] = v;
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }

    // normalise to [0, 1] so the rest of the pipeline (erosion, sea level,
    // rendering) works on a known range
    let range = (max_v - min_v).max(f32::EPSILON);
    for v in map.iter_mut() {
        *v = (*v - min_v) / range;
    }

    if let Some(island) = island {
        apply_island_mask(&mut map, seed, island);
    }

    map
}

/// Multiplies the map by a radial falloff so terrain fades into ocean at the
/// edges. A secondary noise field perturbs the falloff to produce irregular,
/// natural-looking coastlines: bays, peninsulas and offshore islets.
fn apply_island_mask(map: &mut Grid<f32>, seed: u32, island: &IslandParams) {
    let width = map.width();
    let height = map.height();

    let coast = Fbm::<OpenSimplex>::new(seed.wrapping_add(1))
        .set_octaves(6)
        .set_frequency(3.0)
        .set_persistence(0.5);

    let scale = 1.0 / width.max(height) as f64;

    // sample the coast field and normalise it to [-1, 1], so `coast_noise`
    // is a true amplitude relative to the radial gradient regardless of
    // the raw range the noise happens to produce
    let mut wobble = Grid::new(width, height, 0.0f32);
    let mut min_v = f32::MAX;
    let mut max_v = f32::MIN;
    for y in 0..height {
        for x in 0..width {
            let v = coast.get([x as f64 * scale, y as f64 * scale]) as f32;
            wobble[(x, y)] = v;
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }
    let range = (max_v - min_v).max(f32::EPSILON);

    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let radius = 0.5 * width.min(height) as f32 * island.size;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt() / radius;

            let w = ((wobble[(x, y)] - min_v) * 2.0 / range - 1.0).abs();

            let falloff = 1.0 - (d.powi(3) + w * island.coast_noise);

            let v = &mut map[(x, y)];
            *v = (*v * falloff).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_in_unit_range() {
        let map = generate(64, 64, 3, &NoiseParams::default(), None);
        assert_eq!(map.len(), 64 * 64);
        assert!(map.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn deterministic_for_seed() {
        let a = generate(32, 32, 9, &NoiseParams::default(), None);
        let b = generate(32, 32, 9, &NoiseParams::default(), None);
        assert_eq!(a, b);
    }

    #[test]
    fn island_fades_edges() {
        let island = IslandParams::default();
        let map = generate(64, 64, 3, &NoiseParams::default(), Some(&island));
        // corners must be ocean
        assert_eq!(map[(0, 0)], 0.0);
        assert_eq!(map[(63, 0)], 0.0);
        assert_eq!(map[(0, 63)], 0.0);
        assert_eq!(map[(63, 63)], 0.0);
    }
}
