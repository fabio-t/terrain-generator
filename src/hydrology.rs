//! Flow routing and river extraction.
//!
//! After erosion, water flow is routed over the heightmap with the classic
//! D8 scheme: every cell receives one unit of rain and passes its
//! accumulated flow to its steepest downslope neighbour. Cells whose
//! accumulated flow exceeds a density-derived threshold form the river
//! network — rivers therefore follow the valleys the erosion carved,
//! instead of wandering randomly.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::RiverParams;

/// Result of flow routing over a heightmap.
pub struct Hydrology {
    /// Log-normalised flow accumulation per cell, in `[0, 1]`.
    pub flow: Vec<f32>,
    /// River mask per cell: `1` where a river runs, `0` elsewhere.
    pub rivers: Vec<u8>,
    /// Lake mask per cell: `1` where an inland depression holds water.
    pub lakes: Vec<u8>,
}

/// Minimum fill depth for a cell to count as lake rather than numeric noise.
const LAKE_DEPTH: f32 = 2e-3;

/// Routes flow over `heightmap` and extracts a river network.
///
/// `sea_level` limits rivers to land cells; flow is still routed through
/// ocean cells so deltas terminate correctly.
pub fn compute(
    heightmap: &[f32],
    width: usize,
    height: usize,
    sea_level: f32,
    params: &RiverParams,
) -> Hydrology {
    let n = width * height;
    assert_eq!(heightmap.len(), n);

    // route over a depression-filled copy of the terrain, so flow is never
    // trapped in local pits and rivers run continuously to the map border;
    // land/water classification still uses the original heightmap
    let filled = fill_depressions(heightmap, width, height);
    let routing = filled.as_slice();

    // process cells from highest to lowest so upstream flow is complete
    // before a cell passes it on
    let mut order: Vec<u32> = (0..n as u32).collect();
    order.sort_unstable_by(|a, b| {
        routing[*b as usize]
            .partial_cmp(&routing[*a as usize])
            .unwrap()
    });

    let mut accum = vec![1.0f32; n];

    for &idx in &order {
        let idx = idx as usize;
        let x = (idx % width) as i32;
        let y = (idx / width) as i32;
        let h = routing[idx];

        // steepest descent among the 8 neighbours, diagonal distance-weighted
        let mut best: Option<usize> = None;
        let mut best_slope = 0.0f32;
        for oy in -1..=1i32 {
            for ox in -1..=1i32 {
                if ox == 0 && oy == 0 {
                    continue;
                }
                let nx = x + ox;
                let ny = y + oy;
                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                let nidx = ny as usize * width + nx as usize;
                let dist = if ox != 0 && oy != 0 {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
                let slope = (h - routing[nidx]) / dist;
                if slope > best_slope {
                    best_slope = slope;
                    best = Some(nidx);
                }
            }
        }

        if let Some(nidx) = best {
            accum[nidx] += accum[idx];
        }
    }

    // log-normalise the accumulation so it is usable as a moisture/flow map
    let max_accum = accum.iter().cloned().fold(1.0f32, f32::max);
    let log_max = (1.0 + max_accum).ln();
    let flow: Vec<f32> = accum.iter().map(|a| (1.0 + a).ln() / log_max).collect();

    // depressions that were filled by a noticeable depth hold standing
    // water: they are lakes, and rivers vanish beneath them
    let lakes: Vec<u8> = (0..n)
        .map(|i| u8::from(heightmap[i] > sea_level && filled[i] - heightmap[i] > LAKE_DEPTH))
        .collect();

    // threshold at the (1 - density) quantile of land-cell accumulation, so
    // `density` is roughly the fraction of land covered by rivers
    let mut land_accum: Vec<f32> = (0..n)
        .filter(|i| heightmap[*i] > sea_level)
        .map(|i| accum[i])
        .collect();

    let rivers = if land_accum.is_empty() || params.density <= 0.0 {
        vec![0u8; n]
    } else {
        land_accum.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        let q = ((land_accum.len() - 1) as f32 * (1.0 - params.density)) as usize;
        // rivers need actual upstream area, not just their own rain
        let threshold = land_accum[q].max(1.0 + f32::EPSILON);

        (0..n)
            .map(|i| u8::from(heightmap[i] > sea_level && lakes[i] == 0 && accum[i] >= threshold))
            .collect()
    };

    Hydrology {
        flow,
        rivers,
        lakes,
    }
}

/// Priority-flood depression filling (Barnes, Lehman & Mulla 2014).
///
/// Returns a copy of the heightmap where every inland depression is raised
/// to its spill level (plus a tiny gradient), so steepest-descent routing
/// always finds a path to the map border.
fn fill_depressions(heightmap: &[f32], width: usize, height: usize) -> Vec<f32> {
    const EPSILON: f32 = 1e-5;

    let n = width * height;
    let mut filled = heightmap.to_vec();
    let mut visited = vec![false; n];

    // min-heap of (height, index); Reverse turns std's max-heap around.
    // f32 is not Ord, so heights are compared through their ordered bit
    // pattern (all values here are finite and non-negative)
    let key = |h: f32| Reverse((h.to_bits(), 0u32));
    let mut heap: BinaryHeap<(Reverse<(u32, u32)>, u32)> = BinaryHeap::new();

    let push = |heap: &mut BinaryHeap<(Reverse<(u32, u32)>, u32)>,
                visited: &mut Vec<bool>,
                filled: &Vec<f32>,
                idx: usize| {
        if !visited[idx] {
            visited[idx] = true;
            heap.push((key(filled[idx]), idx as u32));
        }
    };

    // seed with the border: water can always leave the map there
    for x in 0..width {
        push(&mut heap, &mut visited, &filled, x);
        push(&mut heap, &mut visited, &filled, (height - 1) * width + x);
    }
    for y in 0..height {
        push(&mut heap, &mut visited, &filled, y * width);
        push(&mut heap, &mut visited, &filled, y * width + width - 1);
    }

    while let Some((_, idx)) = heap.pop() {
        let idx = idx as usize;
        let x = (idx % width) as i32;
        let y = (idx / width) as i32;
        let level = filled[idx];

        for (ox, oy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let nx = x + ox;
            let ny = y + oy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let nidx = ny as usize * width + nx as usize;
            if !visited[nidx] {
                visited[nidx] = true;
                if filled[nidx] < level + EPSILON {
                    filled[nidx] = level + EPSILON;
                }
                heap.push((key(filled[nidx]), nidx as u32));
            }
        }
    }

    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_accumulates_downhill() {
        // a V-shaped valley draining along the centre column
        let n = 32;
        let mut map = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let vx = ((x as f32 / (n - 1) as f32) - 0.5).abs(); // valley shape
                let vy = 1.0 - y as f32 / (n - 1) as f32; // tilt toward y = max
                map[y * n + x] = vx + vy * 0.5;
            }
        }
        let hydro = compute(&map, n, n, -1.0, &RiverParams { density: 0.05 });

        // the valley floor near the outlet must carry more flow than a ridge
        let outlet = (n - 2) * n + n / 2;
        let ridge = (n / 2) * n + 1;
        assert!(hydro.flow[outlet] > hydro.flow[ridge]);
        // and the outlet should be marked as river
        assert_eq!(hydro.rivers[outlet], 1);
    }

    #[test]
    fn depressions_are_filled_to_spill_level() {
        // a 1-cell pit inside a flat plateau with a rim
        let n = 9;
        let mut map = vec![0.5f32; n * n];
        map[4 * n + 4] = 0.1; // the pit
        let filled = fill_depressions(&map, n, n);
        // the pit must be raised to (at least) the plateau level
        assert!(filled[4 * n + 4] >= 0.5);
        // untouched terrain keeps its height
        assert_eq!(filled[0], 0.5);
    }

    #[test]
    fn density_zero_means_no_rivers() {
        let map = vec![0.5f32; 16 * 16];
        let hydro = compute(&map, 16, 16, 0.1, &RiverParams { density: 0.0 });
        assert!(hydro.rivers.iter().all(|r| *r == 0));
    }
}
