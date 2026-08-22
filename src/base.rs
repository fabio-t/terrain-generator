//! Base heightmap generation: fractal (fBm) OpenSimplex noise plus an
//! optional radial island mask with noisy coastlines.

use noise::{Fbm, MultiFractal, NoiseFn, OpenSimplex};

use crate::grid::Grid;
use crate::rng::Pcg32;
use crate::{ArchipelagoParams, IslandParams, NoiseParams};

/// Which land-shaping mask to apply to the raw noise, if any.
pub(crate) enum Mask<'a> {
    /// One radial island in the middle of the map.
    Island(&'a IslandParams),
    /// Several radial islands placed from the seed.
    Archipelago(&'a ArchipelagoParams),
}

/// PCG stream for island placement. It is separate from the erosion stream
/// so that adding archipelago support cannot perturb any output that does
/// not use it.
const PLACEMENT_STREAM: u64 = 0x5ea1_5ca9_e131_0d5b;

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
    mask: Option<Mask<'_>>,
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

    match mask {
        Some(Mask::Island(island)) => apply_island_mask(&mut map, seed, island),
        Some(Mask::Archipelago(arch)) => apply_archipelago_mask(&mut map, seed, arch),
        None => {}
    }

    map
}

/// A placed island: centre in cells, radius in cells.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlacedIsland {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
}

/// Places island centres deterministically from the seed.
///
/// Spacing is best-effort: each centre gets a bounded number of attempts to
/// land at least `min_spacing` away from the ones already placed, and the
/// last candidate is taken if none qualifies. That keeps the island *count*
/// exact — which is what a caller asking for "five islands" wants — at the
/// cost of occasionally letting two neighbours merge into one bigger
/// landmass.
pub(crate) fn place_islands(
    width: usize,
    height: usize,
    seed: u32,
    p: &ArchipelagoParams,
) -> Vec<PlacedIsland> {
    const ATTEMPTS: usize = 64;

    let mut rng = Pcg32::new(seed as u64, PLACEMENT_STREAM);

    let short = width.min(height) as f32;
    let base_radius = 0.5 * short * p.size;
    let min_dist = p.min_spacing * short;

    // keep centres far enough inside that an island is not mostly cropped
    let margin = base_radius * 0.6;
    let span_x = (width as f32 * 0.5 - margin).max(0.0) * p.spread;
    let span_y = (height as f32 * 0.5 - margin).max(0.0) * p.spread;

    let mut placed: Vec<PlacedIsland> = Vec::with_capacity(p.islands);

    for _ in 0..p.islands {
        let mut candidate = None;
        for attempt in 0..ATTEMPTS {
            // draw unconditionally so the RNG sequence does not depend on
            // how many attempts were rejected
            let cx = width as f32 * 0.5 + (rng.next_f32() * 2.0 - 1.0) * span_x;
            let cy = height as f32 * 0.5 + (rng.next_f32() * 2.0 - 1.0) * span_y;
            let radius = base_radius * (1.0 + (rng.next_f32() * 2.0 - 1.0) * p.size_variation);

            let ok = placed.iter().all(|o| {
                let dx = o.cx - cx;
                let dy = o.cy - cy;
                (dx * dx + dy * dy).sqrt() >= min_dist
            });

            candidate = Some(PlacedIsland { cx, cy, radius });
            if ok || attempt == ATTEMPTS - 1 {
                break;
            }
        }
        if let Some(c) = candidate {
            placed.push(c);
        }
    }

    placed
}

/// Multiplies the map by the union of several radial falloffs, so the land
/// forms an archipelago instead of a single island.
///
/// The union is a `min` over each island's normalised distance, which makes
/// this exactly the single-island formula with `d³` replaced by the nearest
/// island's `d³` — the same coastline noise is then applied once, globally,
/// so individual islands are shaped like the single-island ones.
fn apply_archipelago_mask(map: &mut Grid<f32>, seed: u32, arch: &ArchipelagoParams) {
    let width = map.width();
    let height = map.height();

    let islands = place_islands(width, height, seed, arch);
    if islands.is_empty() {
        return;
    }

    let wobble = coast_field(width, height, seed);

    for y in 0..height {
        for x in 0..width {
            let mut nearest = f32::MAX;
            for island in &islands {
                let dx = x as f32 - island.cx;
                let dy = y as f32 - island.cy;
                let d = (dx * dx + dy * dy).sqrt() / island.radius;
                let d3 = d * d * d;
                if d3 < nearest {
                    nearest = d3;
                }
            }

            let falloff = 1.0 - (nearest + wobble[(x, y)] * arch.coast_noise);

            let v = &mut map[(x, y)];
            *v = (*v * falloff).clamp(0.0, 1.0);
        }
    }
}

/// The coastline perturbation field, normalised to `[0, 1]` after taking
/// the absolute value — shared by both mask modes.
fn coast_field(width: usize, height: usize, seed: u32) -> Grid<f32> {
    let coast = Fbm::<OpenSimplex>::new(seed.wrapping_add(1))
        .set_octaves(6)
        .set_frequency(3.0)
        .set_persistence(0.5);

    let scale = 1.0 / width.max(height) as f64;

    let mut raw = Grid::new(width, height, 0.0f32);
    let mut min_v = f32::MAX;
    let mut max_v = f32::MIN;
    for y in 0..height {
        for x in 0..width {
            let v = coast.get([x as f64 * scale, y as f64 * scale]) as f32;
            raw[(x, y)] = v;
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }
    let range = (max_v - min_v).max(f32::EPSILON);
    for v in raw.iter_mut() {
        *v = ((*v - min_v) * 2.0 / range - 1.0).abs();
    }
    raw
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
        let map = generate(
            64,
            64,
            3,
            &NoiseParams::default(),
            Some(Mask::Island(&island)),
        );
        // corners must be ocean
        assert_eq!(map[(0, 0)], 0.0);
        assert_eq!(map[(63, 0)], 0.0);
        assert_eq!(map[(0, 63)], 0.0);
        assert_eq!(map[(63, 63)], 0.0);
    }

    #[test]
    fn island_placement_is_deterministic_and_seed_dependent() {
        let p = ArchipelagoParams::default();
        assert_eq!(
            format!("{:?}", place_islands(256, 256, 4, &p)),
            format!("{:?}", place_islands(256, 256, 4, &p))
        );
        assert_ne!(
            format!("{:?}", place_islands(256, 256, 4, &p)),
            format!("{:?}", place_islands(256, 256, 5, &p))
        );
    }

    #[test]
    fn placement_returns_the_requested_count_inside_the_map() {
        let p = ArchipelagoParams {
            islands: 7,
            ..Default::default()
        };
        let placed = place_islands(300, 200, 1, &p);
        assert_eq!(placed.len(), 7);
        for island in placed {
            assert!(island.radius > 0.0);
            assert!(island.cx >= 0.0 && island.cx <= 300.0);
            assert!(island.cy >= 0.0 && island.cy <= 200.0);
        }
    }

    #[test]
    fn archipelago_mask_leaves_the_corners_as_sea() {
        let arch = ArchipelagoParams::with_islands(3);
        let map = generate(
            96,
            96,
            2,
            &NoiseParams::default(),
            Some(Mask::Archipelago(&arch)),
        );
        assert_eq!(map[(0, 0)], 0.0);
        assert_eq!(map[(95, 95)], 0.0);
    }
}
