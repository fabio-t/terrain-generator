//! Flow routing, ocean/lake classification and river extraction.
//!
//! After erosion, water flow is routed over the heightmap with the classic
//! D8 scheme: every cell receives one unit of rain and passes its
//! accumulated flow to its steepest downslope neighbour. Cells whose
//! accumulated flow exceeds a threshold form the river network — rivers
//! therefore follow the valleys the erosion carved, instead of wandering
//! randomly.
//!
//! Standing water is split into two facts about the terrain:
//!
//! * **ocean** — cells at or below sea level that are connected to the map
//!   border by other such cells (a flood fill). This is what makes an
//!   enclosed basin whose floor lies below sea level an inland basin
//!   rather than sea.
//! * **lakes** — cells that priority-flood depression filling had to raise
//!   noticeably above the terrain, and that are not ocean.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::grid::Grid;
use crate::{RiverParams, RiverThreshold};

/// Result of flow routing over a heightmap.
pub struct Hydrology {
    /// Raw D8 flow accumulation per cell, in cells of upstream catchment
    /// (every cell contributes 1, including itself).
    pub accumulation: Grid<f32>,
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
pub(crate) const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Marks cells that belong to the ocean: at or below `sea_level` and
/// reachable from the map border through other such cells.
///
/// This is a pure function of the heightmap and sea level — no hydrology
/// needed — so it is available even when river extraction is disabled.
/// Enclosed basins below sea level are deliberately *not* ocean: they are
/// inland water, and a consumer modelling salinity or drainage needs that
/// distinction.
pub fn ocean_mask(heightmap: &Grid<f32>, sea_level: f32) -> Grid<u8> {
    let width = heightmap.width();
    let height = heightmap.height();
    let mut ocean = Grid::new(width, height, 0u8);

    // explicit stack rather than recursion: a map-sized flood fill would
    // overflow the stack on large maps
    let mut stack: Vec<u32> = Vec::new();

    let seed_cell = |stack: &mut Vec<u32>, ocean: &mut Grid<u8>, x: usize, y: usize| {
        if ocean[(x, y)] == 0 && heightmap[(x, y)] <= sea_level {
            ocean[(x, y)] = 1;
            stack.push(heightmap.idx(x, y) as u32);
        }
    };

    for x in 0..width {
        seed_cell(&mut stack, &mut ocean, x, 0);
        seed_cell(&mut stack, &mut ocean, x, height - 1);
    }
    for y in 0..height {
        seed_cell(&mut stack, &mut ocean, 0, y);
        seed_cell(&mut stack, &mut ocean, width - 1, y);
    }

    while let Some(idx) = stack.pop() {
        let (x, y) = heightmap.coords(idx as usize);
        for (ox, oy) in NEIGHBOURS {
            let nx = x as i32 + ox;
            let ny = y as i32 + oy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let n = (nx as usize, ny as usize);
            if ocean[n] == 0 && heightmap[n] <= sea_level {
                ocean[n] = 1;
                stack.push(heightmap.idx(n.0, n.1) as u32);
            }
        }
    }

    ocean
}

/// Routes flow over `heightmap` and extracts rivers and lakes.
///
/// `ocean` comes from [`ocean_mask`]; rivers and lakes are excluded from it,
/// while flow is still routed through ocean cells so deltas terminate
/// correctly.
pub fn compute(heightmap: &Grid<f32>, ocean: &Grid<u8>, params: &RiverParams) -> Hydrology {
    // route over a depression-filled copy of the terrain, so flow is never
    // trapped in local pits and rivers run continuously to the map border;
    // land/water classification still uses the original heightmap
    let filled = fill_depressions(heightmap);

    let accumulation = route_flow(&filled);
    let flow = normalise_flow(&accumulation);
    let lakes = detect_lakes(heightmap, &filled, ocean);
    let rivers = extract_rivers(&accumulation, &lakes, ocean, params);

    Hydrology {
        accumulation,
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
pub(crate) fn fill_depressions(heightmap: &Grid<f32>) -> Grid<f32> {
    const EPSILON: f32 = 1e-5;

    let width = heightmap.width();
    let height = heightmap.height();
    let mut filled = heightmap.clone();
    let mut visited = Grid::new(width, height, false);

    // min-heap of (height, index); Reverse turns std's max-heap around.
    // f32 is not Ord, so heights are keyed by their ordered bit pattern
    // (valid because all values here are finite and non-negative). The
    // index is part of the key, so equal heights have a total order and
    // the pop sequence never depends on heap internals.
    let key = |h: f32, idx: usize| Reverse((h.to_bits(), idx as u32));
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();

    // seed with the border: water can always leave the map there
    for x in 0..width {
        for y in [0, height - 1] {
            if !std::mem::replace(&mut visited[(x, y)], true) {
                heap.push(key(filled[(x, y)], filled.idx(x, y)));
            }
        }
    }
    for y in 0..height {
        for x in [0, width - 1] {
            if !std::mem::replace(&mut visited[(x, y)], true) {
                heap.push(key(filled[(x, y)], filled.idx(x, y)));
            }
        }
    }

    while let Some(Reverse((_, idx))) = heap.pop() {
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
                heap.push(key(filled[n], filled.idx(n.0, n.1)));
            }
        }
    }

    filled
}

/// D8 flow accumulation over a pit-free surface: every cell receives one
/// unit of rain and passes its total to its steepest downslope neighbour.
///
/// Cells are processed highest first. Equal heights are ordered by cell
/// index so the sequence is fully determined by the input, never by the
/// sort implementation — `sort_unstable_by` leaves equal elements in an
/// unspecified order, which could otherwise vary between compiler versions.
fn route_flow(routing: &Grid<f32>) -> Grid<f32> {
    let width = routing.width();
    let height = routing.height();
    let surface = routing.as_slice();

    let mut order: Vec<u32> = (0..routing.len() as u32).collect();
    order.sort_unstable_by(|a, b| {
        surface[*b as usize]
            .total_cmp(&surface[*a as usize])
            .then_with(|| a.cmp(b))
    });

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
fn normalise_flow(accum: &Grid<f32>) -> Grid<f32> {
    let max_accum = accum.iter().cloned().fold(1.0f32, f32::max);
    let log_max = (1.0 + max_accum).ln();

    Grid::from_vec(
        accum.width(),
        accum.height(),
        accum.iter().map(|a| (1.0 + a).ln() / log_max).collect(),
    )
}

/// Marks cells whose filled level sits noticeably above the terrain and
/// that are not ocean: depressions that hold standing water.
fn detect_lakes(heightmap: &Grid<f32>, filled: &Grid<f32>, ocean: &Grid<u8>) -> Grid<u8> {
    Grid::from_vec(
        heightmap.width(),
        heightmap.height(),
        heightmap
            .iter()
            .zip(filled.iter())
            .zip(ocean.iter())
            .map(|((h, f), o)| u8::from(*o == 0 && f - h > LAKE_DEPTH))
            .collect(),
    )
}

/// Applies the configured [`RiverThreshold`] to the accumulation map.
/// Rivers vanish beneath lakes and continue from their outlet, and never
/// run across the ocean.
fn extract_rivers(
    accum: &Grid<f32>,
    lakes: &Grid<u8>,
    ocean: &Grid<u8>,
    params: &RiverParams,
) -> Grid<u8> {
    let width = accum.width();
    let height = accum.height();

    // a cell can carry a river only if it is dry land outside any lake
    let eligible = |i: usize| ocean.as_slice()[i] == 0 && lakes.as_slice()[i] == 0;

    let threshold = match params.threshold {
        RiverThreshold::Density(density) => {
            if density <= 0.0 {
                return Grid::new(width, height, 0);
            }
            let mut land: Vec<f32> = (0..accum.len())
                .filter(|i| ocean.as_slice()[*i] == 0)
                .map(|i| accum.as_slice()[i])
                .collect();
            if land.is_empty() {
                return Grid::new(width, height, 0);
            }
            land.sort_unstable_by(f32::total_cmp);
            let q = ((land.len() - 1) as f32 * (1.0 - density)) as usize;
            // rivers need actual upstream area, not just their own rain
            land[q].max(1.0 + f32::EPSILON)
        }
        RiverThreshold::MinUpstreamCells(cells) => cells.max(1.0 + f32::EPSILON),
        RiverThreshold::PerEdge(factor) => {
            (factor * width.max(height) as f32).max(1.0 + f32::EPSILON)
        }
    };

    Grid::from_vec(
        width,
        height,
        (0..accum.len())
            .map(|i| u8::from(eligible(i) && accum.as_slice()[i] >= threshold))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A V-shaped valley draining towards `y = max`.
    fn valley(n: usize) -> Grid<f32> {
        let mut map = Grid::new(n, n, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                let vx = ((x as f32 / (n - 1) as f32) - 0.5).abs();
                let vy = 1.0 - y as f32 / (n - 1) as f32;
                map[(x, y)] = vx + vy * 0.5;
            }
        }
        map
    }

    fn hydro(map: &Grid<f32>, sea_level: f32, density: f32) -> Hydrology {
        let ocean = ocean_mask(map, sea_level);
        compute(map, &ocean, &RiverParams::density(density))
    }

    #[test]
    fn flow_accumulates_downhill() {
        let n = 32;
        let map = valley(n);
        let h = hydro(&map, -1.0, 0.05);

        assert!(h.flow[(n / 2, n - 2)] > h.flow[(1, n / 2)]);
        assert!(h.accumulation[(n / 2, n - 2)] > h.accumulation[(1, n / 2)]);
        assert_eq!(h.rivers[(n / 2, n - 2)], 1);
    }

    #[test]
    fn accumulation_is_in_catchment_cells() {
        let n = 16;
        let map = valley(n);
        let h = hydro(&map, -1.0, 0.05);
        // every cell drains at least itself, and no cell can collect more
        // than the whole map
        assert!(h.accumulation.iter().all(|a| *a >= 1.0));
        let max = h.accumulation.iter().cloned().fold(0.0f32, f32::max);
        assert!(max <= (n * n) as f32);
        // the outlet of a single-valley map gathers a large share of it
        assert!(max > (n * n) as f32 * 0.5);
    }

    #[test]
    fn depressions_are_filled_to_spill_level() {
        let n = 9;
        let mut map = Grid::new(n, n, 0.5f32);
        map[(4, 4)] = 0.1;
        let filled = fill_depressions(&map);
        assert!(filled[(4, 4)] >= 0.5);
        assert_eq!(filled[(0, 0)], 0.5);
    }

    #[test]
    fn density_zero_means_no_rivers() {
        let map = Grid::new(16, 16, 0.5f32);
        let h = hydro(&map, 0.1, 0.0);
        assert!(h.rivers.iter().all(|r| *r == 0));
    }

    #[test]
    fn ocean_is_border_connected_only() {
        // a low rim of sea-level cells around a plateau, plus an enclosed
        // pit in the middle that is also below sea level
        let n = 11;
        let mut map = Grid::new(n, n, 0.5f32);
        for i in 0..n {
            map[(i, 0)] = 0.0;
            map[(i, n - 1)] = 0.0;
            map[(0, i)] = 0.0;
            map[(n - 1, i)] = 0.0;
        }
        map[(5, 5)] = 0.0; // enclosed, below sea level

        let ocean = ocean_mask(&map, 0.05);
        assert_eq!(ocean[(0, 0)], 1, "border ring is ocean");
        assert_eq!(ocean[(5, 0)], 1);
        assert_eq!(
            ocean[(5, 5)],
            0,
            "enclosed below-sea-level pit is not ocean"
        );
    }

    #[test]
    fn enclosed_below_sea_level_basin_is_a_lake() {
        let n = 11;
        let mut map = Grid::new(n, n, 0.5f32);
        for i in 0..n {
            map[(i, 0)] = 0.0;
            map[(i, n - 1)] = 0.0;
            map[(0, i)] = 0.0;
            map[(n - 1, i)] = 0.0;
        }
        map[(5, 5)] = 0.0;

        let h = hydro(&map, 0.05, 0.01);
        assert_eq!(h.lakes[(5, 5)], 1, "inland basin holds a lake");
        assert_eq!(h.lakes[(0, 0)], 0, "ocean is not a lake");
    }

    #[test]
    fn min_upstream_threshold_is_absolute() {
        let n = 32;
        let map = valley(n);
        let ocean = ocean_mask(&map, -1.0);

        let strict = compute(&map, &ocean, &RiverParams::min_upstream_cells(400.0));
        let loose = compute(&map, &ocean, &RiverParams::min_upstream_cells(20.0));

        let count = |g: &Grid<u8>| g.iter().filter(|v| **v == 1).count();
        assert!(count(&loose.rivers) > count(&strict.rivers));
        // no cell below the threshold may be marked
        for i in 0..map.len() {
            if strict.rivers.as_slice()[i] == 1 {
                assert!(strict.accumulation.as_slice()[i] >= 400.0);
            }
        }
    }

    #[test]
    fn per_edge_threshold_scales_with_map_size() {
        // the same PerEdge factor should mark a similar *share* of a small
        // and a large map, unlike a fixed absolute threshold
        let small = valley(32);
        let large = valley(64);
        let share = |map: &Grid<f32>| {
            let ocean = ocean_mask(map, -1.0);
            let h = compute(map, &ocean, &RiverParams::per_edge(2.0));
            h.rivers.iter().filter(|v| **v == 1).count() as f32 / map.len() as f32
        };
        let (s, l) = (share(&small), share(&large));
        assert!((s - l).abs() < 0.05, "shares diverge: {s} vs {l}");
    }

    #[test]
    fn routing_order_is_stable_on_flat_ties() {
        // a completely flat map is all ties: the result must still be
        // identical every run
        let map = Grid::new(24, 24, 0.42f32);
        let ocean = ocean_mask(&map, 0.0);
        let a = compute(&map, &ocean, &RiverParams::default());
        let b = compute(&map, &ocean, &RiverParams::default());
        assert_eq!(a.accumulation, b.accumulation);
    }
}
