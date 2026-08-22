//! Determinism contract: the same (seed, parameters) must produce the same
//! terrain, run after run and — within a pinned version of this crate and
//! of the `noise` dependency — on every platform.
//!
//! The golden checksums below are the teeth of that promise. If a change
//! moves them, it changes every world every consumer has stored a seed for,
//! so it must be a deliberate, released, documented break.

use terrain_generator::{
    BreachParams, ErosionParams, RiverParams, RiverThreshold, Terrain, TerrainBuilder,
};

fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn height_sum(t: &Terrain) -> u64 {
    fnv(t.heightmap.as_slice().iter().flat_map(|v| v.to_le_bytes()))
}

fn mask_sum(mask: &terrain_generator::Grid<u8>) -> u64 {
    fnv(mask.as_slice().iter().copied())
}

#[test]
fn golden_heightmaps() {
    // The heightmap is the layer consumers persist worlds against; these
    // values have held since the erosion pipeline was introduced.
    let cases: [(&str, TerrainBuilder, u64); 4] = [
        (
            "default 128 seed 1",
            Terrain::builder().size(128, 128).seed(1),
            0x6f375144bc69ba10,
        ),
        (
            "no island 96 seed 7",
            Terrain::builder().size(96, 96).seed(7).no_island(),
            0x3921ca6ab4de01f4,
        ),
        (
            "no erosion 128 seed 2",
            Terrain::builder().size(128, 128).seed(2).no_erosion(),
            0xa74db811298060b6,
        ),
        (
            "50k droplets 128 seed 3",
            Terrain::builder()
                .size(128, 128)
                .seed(3)
                .erosion(ErosionParams {
                    droplets: Some(50_000),
                    ..Default::default()
                }),
            0x3d8e83270a759725,
        ),
    ];

    for (name, builder, expected) in cases {
        let got = height_sum(&builder.generate());
        assert_eq!(got, expected, "heightmap changed for {name}: {got:#018x}");
    }
}

#[test]
fn golden_hydrology_layers() {
    let t = Terrain::builder().size(128, 128).seed(1).generate();
    assert_eq!(mask_sum(&t.rivers), 0x48ba9be36648d0a2, "rivers changed");
    assert_eq!(mask_sum(&t.lakes), 0xc64ae30e8b2e55f6, "lakes changed");
    assert_eq!(mask_sum(&t.ocean), 0x4148296fa7c73b75, "ocean changed");
}

#[test]
fn repeated_runs_are_identical() {
    let build = || {
        Terrain::builder()
            .size(96, 96)
            .seed(21)
            .rivers(RiverParams::per_edge(1.5))
            .breach_basins(BreachParams::default())
            .generate()
    };
    let (a, b) = (build(), build());
    assert_eq!(a.heightmap, b.heightmap);
    assert_eq!(a.flow_accumulation, b.flow_accumulation);
    assert_eq!(a.flow, b.flow);
    assert_eq!(a.rivers, b.rivers);
    assert_eq!(a.lakes, b.lakes);
    assert_eq!(a.ocean, b.ocean);
}

#[test]
fn generate_and_try_generate_agree() {
    let b = Terrain::builder().size(64, 64).seed(9);
    assert_eq!(b.generate().heightmap, b.try_generate().unwrap().heightmap);
}

#[test]
fn stage_toggles_do_not_disturb_earlier_stages() {
    // hydrology settings must never feed back into the heightmap: a
    // consumer running its own hydrology gets exactly the same elevation
    // as one using ours
    let base = Terrain::builder().size(96, 96).seed(12);
    let full = base.clone().generate();
    let heightmap_only = base.clone().no_rivers().generate();
    let dense_rivers = base
        .clone()
        .rivers(RiverParams {
            threshold: RiverThreshold::MinUpstreamCells(50.0),
        })
        .generate();

    assert_eq!(full.heightmap, heightmap_only.heightmap);
    assert_eq!(full.heightmap, dense_rivers.heightmap);
    // ocean is derived from the heightmap alone, so it matches too
    assert_eq!(full.ocean, heightmap_only.ocean);
}
