//! Optional basin breaching: carving outlets through the rims of oversized
//! closed basins.
//!
//! Droplet erosion deposits sediment into pits but cannot cut through a rim
//! tens of metres high, so large interior depressions survive it. Left
//! alone, priority-flood depression filling turns each into a lake, and a
//! big one reads as an inland sea. That is a legitimate landscape, so it is
//! the default — but consumers who want drained, valley-carved interiors
//! can opt into breaching.
//!
//! For every basin above a size limit, this stage finds the *minimax path*
//! out of it: among all paths from the basin floor to lower ground, the one
//! whose highest cell is lowest. That is where water would wear through
//! first, so it is where the outlet belongs. The path is then carved into a
//! channel that descends monotonically from the basin floor to the exit,
//! after which normal flow routing drains the basin through it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::grid::Grid;
use crate::hydrology::{fill_depressions, NEIGHBOURS};
use crate::BreachParams;

/// Minimum depth for a cell to count as part of a depression rather than
/// numeric noise from the fill. Matches the lake threshold in `hydrology`.
const DEPRESSION_DEPTH: f32 = 2e-3;

/// Carves outlets from every closed basin larger than the configured limit.
///
/// Returns the number of basins breached. Basins whose floor has no lower
/// ground anywhere reachable (a global minimum) are left alone.
pub fn breach(heightmap: &mut Grid<f32>, ocean: &Grid<u8>, params: &BreachParams) -> usize {
    let width = heightmap.width();
    let height = heightmap.height();
    let max_cells = (params.max_lake_fraction as f64 * heightmap.len() as f64) as usize;

    let filled = fill_depressions(heightmap);

    // depression cells: raised noticeably by the fill, and not sea
    let is_depression: Vec<bool> = (0..heightmap.len())
        .map(|i| {
            ocean.as_slice()[i] == 0
                && filled.as_slice()[i] - heightmap.as_slice()[i] > DEPRESSION_DEPTH
        })
        .collect();

    let mut visited = vec![false; heightmap.len()];
    let mut breached = 0;

    // scan in index order so the set and order of breached basins is a
    // pure function of the heightmap
    for start in 0..heightmap.len() {
        if visited[start] || !is_depression[start] {
            continue;
        }

        // flood the connected basin
        let mut basin = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(idx) = stack.pop() {
            basin.push(idx);
            let (x, y) = heightmap.coords(idx);
            for (ox, oy) in NEIGHBOURS {
                let nx = x as i32 + ox;
                let ny = y as i32 + oy;
                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    continue;
                }
                let n = heightmap.idx(nx as usize, ny as usize);
                if !visited[n] && is_depression[n] {
                    visited[n] = true;
                    stack.push(n);
                }
            }
        }

        if basin.len() <= max_cells {
            continue; // small enough to stay a lake
        }

        let in_basin: std::collections::HashSet<usize> = basin.iter().copied().collect();

        // the floor is the lowest cell; ties resolved by index
        let floor = *basin
            .iter()
            .min_by(|a, b| {
                heightmap.as_slice()[**a]
                    .total_cmp(&heightmap.as_slice()[**b])
                    .then_with(|| a.cmp(b))
            })
            .expect("basin is non-empty");

        if carve_outlet(heightmap, floor, &in_basin) {
            breached += 1;
        }
    }

    breached
}

/// Finds the minimax path from `floor` to any cell outside the basin that
/// is strictly lower than the floor, and carves it into a descending
/// channel. Returns `false` if no lower ground is reachable.
fn carve_outlet(
    heightmap: &mut Grid<f32>,
    floor: usize,
    in_basin: &std::collections::HashSet<usize>,
) -> bool {
    let width = heightmap.width();
    let height = heightmap.height();
    let floor_h = heightmap.as_slice()[floor];

    // Dijkstra where a path's cost is the highest cell on it. The index is
    // part of the heap key, so equal costs have a total order and the
    // search is reproducible.
    let mut best = vec![f32::INFINITY; heightmap.len()];
    let mut prev = vec![u32::MAX; heightmap.len()];
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();

    best[floor] = floor_h;
    heap.push(Reverse((floor_h.to_bits(), floor as u32)));

    let mut exit = None;
    while let Some(Reverse((cost_bits, idx))) = heap.pop() {
        let idx = idx as usize;
        let cost = f32::from_bits(cost_bits);
        if cost > best[idx] {
            continue; // stale heap entry
        }

        // escaped: outside the basin and genuinely downhill from the floor
        if idx != floor && !in_basin.contains(&idx) && heightmap.as_slice()[idx] < floor_h {
            exit = Some(idx);
            break;
        }

        let (x, y) = heightmap.coords(idx);
        for (ox, oy) in NEIGHBOURS {
            let nx = x as i32 + ox;
            let ny = y as i32 + oy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            let n = heightmap.idx(nx as usize, ny as usize);
            let n_cost = cost.max(heightmap.as_slice()[n]);
            if n_cost < best[n] {
                best[n] = n_cost;
                prev[n] = idx as u32;
                heap.push(Reverse((n_cost.to_bits(), n as u32)));
            }
        }
    }

    let Some(exit) = exit else {
        return false;
    };

    // walk back to the floor, then carve from floor (high) to exit (low)
    let mut path = vec![exit];
    let mut cur = exit;
    while cur != floor {
        cur = prev[cur] as usize;
        path.push(cur);
    }
    path.reverse(); // floor .. exit

    let exit_h = heightmap.as_slice()[exit];
    let steps = (path.len() - 1) as f32;
    for (step, &idx) in path.iter().enumerate() {
        let t = step as f32 / steps;
        let target = floor_h + (exit_h - floor_h) * t;
        // only ever cut down; never raise terrain
        let cell = &mut heightmap.as_mut_slice()[idx];
        if *cell > target {
            *cell = target;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hydrology;
    use crate::{RiverParams, TerrainBuilder};

    /// A plateau tilted towards +x — down to well below the basin floor,
    /// as real terrain descends to the sea — with a big walled basin cut
    /// into the high side.
    fn basin_map(n: usize) -> Grid<f32> {
        let mut map = Grid::new(n, n, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                map[(x, y)] = 0.9 - 0.85 * (x as f32 / (n - 1) as f32);
            }
        }
        // carve a deep square basin well inside the map
        for y in 8..24 {
            for x in 8..24 {
                map[(x, y)] = 0.2;
            }
        }
        map
    }

    /// The same basin, but on a plateau that never descends below the
    /// basin floor: there is nowhere for the water to go.
    fn sunken_basin_map(n: usize) -> Grid<f32> {
        let mut map = Grid::new(n, n, 0.0f32);
        for y in 0..n {
            for x in 0..n {
                map[(x, y)] = 0.9 - 0.5 * (x as f32 / (n - 1) as f32);
            }
        }
        for y in 8..24 {
            for x in 8..24 {
                map[(x, y)] = 0.2;
            }
        }
        map
    }

    #[test]
    fn oversized_basin_is_breached_and_drains() {
        let n = 48;
        let mut map = basin_map(n);
        let ocean = Grid::new(n, n, 0u8);

        // 16x16 = 256 cells; allow at most 0.5% of 2304 = ~11 cells
        let count = breach(
            &mut map,
            &ocean,
            &BreachParams {
                max_lake_fraction: 0.005,
            },
        );
        assert_eq!(count, 1, "the basin should be breached exactly once");

        // after breaching, filling should no longer raise the basin floor
        let filled = hydrology::fill_depressions(&map);
        let residual = filled[(16, 16)] - map[(16, 16)];
        assert!(
            residual < 0.05,
            "basin should mostly drain, residual {residual}"
        );
    }

    #[test]
    fn small_basins_are_left_alone() {
        let n = 48;
        let mut map = basin_map(n);
        let before = map.clone();
        let ocean = Grid::new(n, n, 0u8);

        // allow lakes up to 50% of the map: the basin stays
        let count = breach(
            &mut map,
            &ocean,
            &BreachParams {
                max_lake_fraction: 0.5,
            },
        );
        assert_eq!(count, 0);
        assert_eq!(map, before, "heightmap must be untouched");
    }

    #[test]
    fn basin_below_all_surrounding_ground_is_left_alone() {
        // the floor sits lower than every other cell, so no outlet can
        // slope downwards: the basin has to stay a lake
        let n = 48;
        let mut map = sunken_basin_map(n);
        let before = map.clone();
        let ocean = Grid::new(n, n, 0u8);
        let count = breach(
            &mut map,
            &ocean,
            &BreachParams {
                max_lake_fraction: 0.005,
            },
        );
        assert_eq!(count, 0, "there is nowhere to drain to");
        assert_eq!(map, before);
    }

    #[test]
    fn carving_only_lowers_terrain() {
        let n = 48;
        let mut map = basin_map(n);
        let before = map.clone();
        let ocean = Grid::new(n, n, 0u8);
        breach(
            &mut map,
            &ocean,
            &BreachParams {
                max_lake_fraction: 0.005,
            },
        );
        for i in 0..map.len() {
            assert!(
                map.as_slice()[i] <= before.as_slice()[i] + f32::EPSILON,
                "cell {i} was raised"
            );
        }
    }

    #[test]
    fn breaching_is_deterministic() {
        let n = 48;
        let run = || {
            let mut map = basin_map(n);
            let ocean = Grid::new(n, n, 0u8);
            breach(
                &mut map,
                &ocean,
                &BreachParams {
                    max_lake_fraction: 0.005,
                },
            );
            map
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn breaching_reduces_lake_area_on_real_terrain() {
        let lake_cells = |breach: Option<BreachParams>| {
            let mut b = TerrainBuilder::default()
                .size(160, 160)
                .seed(5)
                .rivers(RiverParams::density(0.02));
            b = match breach {
                Some(p) => b.breach_basins(p),
                None => b,
            };
            let t = b.generate();
            t.lakes.iter().filter(|v| **v == 1).count()
        };

        let without = lake_cells(None);
        let with = lake_cells(Some(BreachParams {
            max_lake_fraction: 0.002,
        }));
        assert!(
            with <= without,
            "breaching should not create lake area: {with} vs {without}"
        );
    }
}
