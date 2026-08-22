//! Archipelago mode: several islands on one map.
//!
//! Erosion and hydrology are mask-agnostic, so the point of these tests is
//! that they keep working *per landmass* — each island drains on its own
//! and the sea between islands stays sea.

use terrain_generator::{ArchipelagoParams, IslandParams, RiverParams, Terrain};

/// Connected components of land (above sea level), 8-connected, ignoring
/// specks smaller than `min_size` cells.
fn landmasses(t: &Terrain, min_size: usize) -> Vec<Vec<usize>> {
    let (w, h) = (t.width(), t.height());
    let land = |i: usize| t.heightmap.as_slice()[i] > t.sea_level;
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();

    for start in 0..w * h {
        if seen[start] || !land(start) {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            for (ox, oy) in [
                (-1i32, -1i32),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let (nx, ny) = (x as i32 + ox, y as i32 + oy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                if !seen[n] && land(n) {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        if comp.len() >= min_size {
            out.push(comp);
        }
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.len()));
    out
}

fn archipelago(seed: u32, islands: usize) -> Terrain {
    Terrain::builder()
        .size(256, 256)
        .seed(seed)
        .archipelago(ArchipelagoParams {
            islands,
            size: 0.28,
            min_spacing: 0.30,
            ..Default::default()
        })
        .rivers(RiverParams::density(0.03))
        .generate()
}

#[test]
fn landmass_count_tracks_the_request() {
    // Exactness is not promised: neighbours can merge, and coastline noise
    // can bite an island in two. The count should still track the request.
    for islands in [2usize, 4, 6] {
        for seed in 0..6u32 {
            let n = landmasses(&archipelago(seed, islands), 40).len();
            let lo = (islands / 2).max(1);
            let hi = islands + 2;
            assert!(
                (lo..=hi).contains(&n),
                "islands={islands} seed={seed}: got {n} landmasses, expected {lo}..={hi}"
            );
        }
    }
}

#[test]
fn archipelago_is_deterministic() {
    let a = archipelago(4, 5);
    let b = archipelago(4, 5);
    assert_eq!(a.heightmap, b.heightmap);
    assert_eq!(a.rivers, b.rivers);
    assert_eq!(a.ocean, b.ocean);
}

#[test]
fn seeds_place_islands_differently() {
    assert_ne!(archipelago(1, 4).heightmap, archipelago(2, 4).heightmap);
}

#[test]
fn each_major_landmass_drains_on_its_own() {
    for seed in 0..4u32 {
        let t = archipelago(seed, 4);
        let comps = landmasses(&t, 200);
        assert!(comps.len() >= 2, "seed {seed} produced too few landmasses");
        // the two biggest islands must each carry their own river network
        for (rank, comp) in comps.iter().take(2).enumerate() {
            let rivers = comp
                .iter()
                .filter(|i| t.rivers.as_slice()[**i] == 1)
                .count();
            assert!(
                rivers > 0,
                "seed {seed}: landmass #{rank} ({} cells) has no rivers",
                comp.len()
            );
        }
    }
}

#[test]
fn no_river_or_lake_sits_on_the_open_sea() {
    for seed in 0..4u32 {
        let t = archipelago(seed, 5);
        for i in 0..t.heightmap.len() {
            if t.ocean.as_slice()[i] == 1 {
                assert_eq!(t.rivers.as_slice()[i], 0, "river on ocean at {i}");
                assert_eq!(t.lakes.as_slice()[i], 0, "lake on ocean at {i}");
            }
        }
    }
}

#[test]
fn open_water_separates_the_islands() {
    // A consumer routing boats needs the gap between islands to be sea, not
    // a chain of lakes or a land bridge: walking from one island's centroid
    // to the next must cross ocean.
    for seed in 0..4u32 {
        let t = archipelago(seed, 4);
        let comps = landmasses(&t, 200);
        assert!(comps.len() >= 2);

        let centroid = |c: &Vec<usize>| {
            let (mut sx, mut sy) = (0f64, 0f64);
            for i in c {
                sx += (i % t.width()) as f64;
                sy += (i / t.width()) as f64;
            }
            (sx / c.len() as f64, sy / c.len() as f64)
        };
        let (x0, y0) = centroid(&comps[0]);
        let (x1, y1) = centroid(&comps[1]);

        let steps = 512;
        let crossed_ocean = (0..=steps).any(|s| {
            let f = s as f64 / steps as f64;
            let x = (x0 + (x1 - x0) * f).round() as usize;
            let y = (y0 + (y1 - y0) * f).round() as usize;
            t.is_ocean(x.min(t.width() - 1), y.min(t.height() - 1))
        });
        assert!(
            crossed_ocean,
            "seed {seed}: no open sea between the two largest islands"
        );
    }
}

#[test]
fn switching_back_to_single_island_restores_single_island_output() {
    // archipelago() and island() are mutually exclusive; the builder must
    // not leave the archipelago set behind
    let single = Terrain::builder().size(128, 128).seed(1).generate();
    let round_trip = Terrain::builder()
        .size(128, 128)
        .seed(1)
        .archipelago(ArchipelagoParams::with_islands(3))
        .island(IslandParams::default())
        .generate();
    assert_eq!(single.heightmap, round_trip.heightmap);
}

#[test]
fn invalid_archipelago_params_are_reported() {
    let err = Terrain::builder()
        .size(64, 64)
        .archipelago(ArchipelagoParams::with_islands(0))
        .try_generate()
        .unwrap_err();
    assert_eq!(err.param, "archipelago islands");
}
