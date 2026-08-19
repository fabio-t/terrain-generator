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

use crate::grid::Grid;
use crate::RiverParams;

/// Result of flow routing over a heightmap.
pub struct Hydrology {
    /// Log-normalised flow accumulation per cell, in `[0, 1]`.
    pub flow: Grid<f32>,
    /// River mask per cell: `1` where a river runs, `0` elsewhere.
    pub rivers: Grid<u8>,
    /// Lake mask per cell: `1` where an inland depression holds water.
    pub lakes: Grid<u8>,
}

/// Minimum fill depth for a cell to count as lake rather than numeric noise.
const LAKE_DEPTH: f32 = 2e-3;

/// The 8 neighbour offsets of the D8 scheme.
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Routes flow over `heightmap` and extracts rivers and lakes.
///
/// `sea_level` limits rivers and lakes to land cells; flow is still routed
/// through ocean cells so deltas terminate correctly.
pub fn compute(heightmap: &Grid<f32>, sea_level: f32, params: &RiverParams) -> Hydrology {
    // route over a depression-filled copy of the terrain, so flow is never
    // trapped in local pits and rivers run continuously to the map border;
    // land/water classification still uses the original heightmap
    let filled = fill_depressions(heightmap);

    let accum = route_flow(&filled);
    let flow = normalise_flow(heightmap, &accum);
    let lakes = detect_lakes(heightmap, &filled, sea_level);
    let rivers = extract_rivers(heightmap, &accum, &lakes, sea_level, params);

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
fn fill_depressions(heightmap: &Grid<f32>) -> Grid<f32> {
    const EPSILON: f32 = 1e-5;

    let width = heightmap.width();
    let height = heightmap.height();
    let mut filled = heightmap.clone();
    let mut visited = Grid::new(width, height, false);

    // min-heap of (height, index); Reverse turns std's max-heap around.
    // f32 is not Ord, so heights are keyed by their ordered bit pattern
    // (valid because all values here are finite and non-negative)
    let key = |h: f32| Reverse(h.to_bits());
    let mut heap: BinaryHeap<(Reverse<u32>, u32)> = BinaryHeap::new();

    // seed with the border: water can always leave the map there
    for x in 0..width {
        for y in [0, height - 1] {
            if !std::mem::replace(&mut visited[(x, y)], true) {
                heap.push((key(filled[(x, y)]), filled.idx(x, y) as u32));
            }
        }
    }
    for y in 0..height {
        for x in [0, width - 1] {
            if !std::mem::replace(&mut visited[(x, y)], true) {
                heap.push((key(filled[(x, y)]), filled.idx(x, y) as u32));
            }
        }
    }

    while let Some((_, idx)) = heap.pop() {
        let (x, y) = filled.coords(idx as usize);
        let level = filled[(x, y)];

        for (ox, oy) in NEIGHBOURS {
            let nx = x as i32 + ox;
            let ny = y as i32 + oy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let n = (nx as usize, ny as usize);
            if !std::mem::replace(&mut visited[n], true) {
                if filled[n] < level + EPSILON {
                    filled[n] = level + EPSILON;
                }
                heap.push((key(filled[n]), filled.idx(n.0, n.1) as u32));
            }
        }
    }

    filled
}

/// D8 flow accumulation over a pit-free surface: every cell receives one
/// unit of rain and passes its total to its steepest downslope neighbour.
fn route_flow(routing: &Grid<f32>) -> Grid<f32> {
    let width = routing.width();
    let height = routing.height();
    let surface = routing.as_slice();

    // process cells from highest to lowest so upstream flow is complete
    // before a cell passes it on
    let mut order: Vec<u32> = (0..routing.len() as u32).collect();
    order.sort_unstable_by(|a, b| surface[*b as usize].total_cmp(&surface[*a as usize]));

    let mut accum = Grid::new(width, height, 1.0f32);

    for &idx in &order {
        let (x, y) = routing.coords(idx as usize);
        let h = routing[(x, y)];

        // steepest descent among the 8 neighbours, diagonal distance-weighted
        let mut best: Option<(usize, usize)> = None;
        let mut best_slope = 0.0f32;
        for (ox, oy) in NEIGHBOURS {
            let nx = x as i32 + ox;
            let ny = y as i32 + oy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let n = (nx as usize, ny as usize);
            let dist = if ox != 0 && oy != 0 {
                std::f32::consts::SQRT_2
            } else {
                1.0
            };
            let slope = (h - routing[n]) / dist;
            if slope > best_slope {
                best_slope = slope;
                best = Some(n);
            }
        }

        if let Some(n) = best {
            let inflow = accum[(x, y)];
            accum[n] += inflow;
        }
    }

    accum
}

/// Log-normalises raw accumulation into a `[0, 1]` flow/moisture map.
fn normalise_flow(heightmap: &Grid<f32>, accum: &Grid<f32>) -> Grid<f32> {
    let max_accum = accum.iter().cloned().fold(1.0f32, f32::max);
    let log_max = (1.0 + max_accum).ln();

    Grid::from_vec(
        heightmap.width(),
        heightmap.height(),
        accum.iter().map(|a| (1.0 + a).ln() / log_max).collect(),
    )
}

/// Marks cells whose filled level sits noticeably above the terrain:
/// depressions that hold standing water.
fn detect_lakes(heightmap: &Grid<f32>, filled: &Grid<f32>, sea_level: f32) -> Grid<u8> {
    Grid::from_vec(
        heightmap.width(),
        heightmap.height(),
        heightmap
            .iter()
            .zip(filled.iter())
            .map(|(h, f)| u8::from(*h > sea_level && f - h > LAKE_DEPTH))
            .collect(),
    )
}

/// Thresholds flow accumulation at the `(1 - density)` quantile of land
/// cells, so `density` is roughly the fraction of land covered by rivers.
/// Rivers vanish beneath lakes and continue from their outlet.
fn extract_rivers(
    heightmap: &Grid<f32>,
    accum: &Grid<f32>,
    lakes: &Grid<u8>,
    sea_level: f32,
    params: &RiverParams,
) -> Grid<u8> {
    let mut land_accum: Vec<f32> = heightmap
        .iter()
        .zip(accum.iter())
        .filter(|(h, _)| **h > sea_level)
        .map(|(_, a)| *a)
        .collect();

    if land_accum.is_empty() || params.density <= 0.0 {
        return Grid::new(heightmap.width(), heightmap.height(), 0);
    }

    land_accum.sort_unstable_by(f32::total_cmp);
    let q = ((land_accum.len() - 1) as f32 * (1.0 - params.density)) as usize;
    // rivers need actual upstream area, not just their own rain
    let threshold = land_accum[q].max(1.0 + f32::EPSILON);

    Grid::from_vec(
        heightmap.width(),
        heightmap.height(),
        heightmap
            .iter()
            .zip(accum.iter())
            .zip(lakes.iter())
            .map(|((h, a), lake)| u8::from(*h > sea_level && *lake == 0 && *a >= threshold))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_accumulates_downhill() {
        // a V-shaped valley draining along the centre column
        let n = 32;
        let mut map = Grid::new(n, n, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                let vx = ((x as f32 / (n - 1) as f32) - 0.5).abs(); // valley shape
                let vy = 1.0 - y as f32 / (n - 1) as f32; // tilt toward y = max
                map[(x, y)] = vx + vy * 0.5;
            }
        }
        let hydro = compute(&map, -1.0, &RiverParams { density: 0.05 });

        // the valley floor near the outlet must carry more flow than a ridge
        assert!(hydro.flow[(n / 2, n - 2)] > hydro.flow[(1, n / 2)]);
        // and the outlet should be marked as river
        assert_eq!(hydro.rivers[(n / 2, n - 2)], 1);
    }

    #[test]
    fn depressions_are_filled_to_spill_level() {
        // a 1-cell pit inside a flat plateau with a rim
        let n = 9;
        let mut map = Grid::new(n, n, 0.5f32);
        map[(4, 4)] = 0.1; // the pit
        let filled = fill_depressions(&map);
        // the pit must be raised to (at least) the plateau level
        assert!(filled[(4, 4)] >= 0.5);
        // untouched terrain keeps its height
        assert_eq!(filled[(0, 0)], 0.5);
    }

    #[test]
    fn density_zero_means_no_rivers() {
        let map = Grid::new(16, 16, 0.5f32);
        let hydro = compute(&map, 0.1, &RiverParams { density: 0.0 });
        assert!(hydro.rivers.iter().all(|r| *r == 0));
    }
}
