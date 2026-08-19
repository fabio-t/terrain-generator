# Rust API

`terrain-generator` is an ordinary Rust library crate; add it to any
application with cargo. This document is a stable overview — the complete,
always-up-to-date reference is the rustdoc, which you can build locally
with `cargo doc --open`.

## Installation

From a git dependency:

```toml
[dependencies]
terrain-generator = { git = "https://github.com/fabio-t/terrain-generator", default-features = false }
```

The crate has one feature:

| Feature | Default | Effect |
|---------|---------|--------|
| `cli`   | on      | Builds the `terrain-generator` binary and pulls in `clap`. Library consumers should disable it (`default-features = false`) to skip the `clap` dependency. |

## Quick start

```rust
use terrain_generator::{RenderStyle, Terrain};

let terrain = Terrain::builder()
    .size(512, 512)
    .seed(42)
    .generate();

// render an image
terrain.save_png("map.png", RenderStyle::Shaded)?;

// or consume the raw data
let elevation = terrain.height_at(100, 100);   // f32 in [0, 1]
let moisture  = terrain.flow_at(100, 100);     // f32 in [0, 1]
let has_river = terrain.is_river(100, 100);
let has_lake  = terrain.is_lake(100, 100);
```

## Pipeline

`TerrainBuilder::generate` runs four stages. Stages 2–4 are enabled by
default and can be disabled individually.

1. **Base heightmap** — fractal (fBm) OpenSimplex noise from the
   [`noise`](https://crates.io/crates/noise) crate, normalised to `[0, 1]`.
2. **Island mask** — radial falloff with a secondary noise field
   perturbing the coastline.
3. **Hydraulic erosion** — particle-based droplet simulation (Beyer 2015):
   droplets pick up sediment on steep, fast descents and deposit it on
   flats, carving valleys and drainage networks.
4. **Hydrology** — depressions are filled with the priority-flood
   algorithm (Barnes et al. 2014), flow is routed with D8 steepest
   descent, and cells above a flow-accumulation threshold become rivers.
   Filled depressions deeper than a small epsilon become lakes.

Generation is **fully deterministic**: the same seed and parameters always
produce the same terrain, on every platform.

## Types

### `Terrain::builder() -> TerrainBuilder`

The entry point. All builder methods take `self` and return `Self`, so
calls chain; `generate(&self)` can be called repeatedly (e.g. with
different seeds via `.seed(n)`).

| Method | Default | Description |
|--------|---------|-------------|
| `.size(width, height)` | `512 × 512` | Map dimensions in cells. Panics if either is `0`. |
| `.seed(u32)` | `0` | Generation seed. |
| `.sea_level(f32)` | `0.08` | Sea level in `[0, 1)`. Panics outside that range. |
| `.noise(NoiseParams)` | see below | Fractal noise parameters. |
| `.island(IslandParams)` / `.no_island()` | enabled | Island shaping. |
| `.erosion(ErosionParams)` / `.no_erosion()` | enabled | Hydraulic erosion. |
| `.rivers(RiverParams)` / `.no_rivers()` | enabled | Flow routing, rivers and lakes. |
| `.generate()` | — | Runs the pipeline, returns a `Terrain`. |

### `NoiseParams`

| Field | Default | Description |
|-------|---------|-------------|
| `octaves: usize` | `8` | Detail levels of the fBm noise. |
| `frequency: f64` | `2.5` | Number of large features across the map. Resolution-independent. |
| `lacunarity: f64` | `2.0` | Frequency multiplier between octaves. |
| `persistence: f64` | `0.5` | Amplitude multiplier between octaves. |

### `IslandParams`

| Field | Default | Description |
|-------|---------|-------------|
| `size: f32` | `0.9` | Island diameter as a fraction of the shorter map dimension. |
| `coast_noise: f32` | `0.35` | Coastline perturbation strength; `0` gives a round island. |

### `ErosionParams`

The defaults work well for maps between 256² and 2048²; usually only
`droplets` needs tuning.

| Field | Default | Description |
|-------|---------|-------------|
| `droplets: Option<usize>` | `None` (one per cell) | Number of simulated droplets. |
| `max_lifetime: usize` | `30` | Maximum steps per droplet. |
| `inertia: f32` | `0.05` | Momentum retention in `[0, 1]`. |
| `capacity: f32` | `4.0` | Sediment capacity multiplier; higher carves deeper. |
| `min_capacity: f32` | `0.01` | Minimum capacity on flats. |
| `erode_speed: f32` | `0.3` | Fraction of free capacity eroded per step. |
| `deposit_speed: f32` | `0.3` | Fraction of surplus sediment deposited per step. |
| `evaporate_speed: f32` | `0.01` | Water lost per step. |
| `gravity: f32` | `4.0` | Acceleration on descent. |
| `radius: usize` | `3` | Erosion brush radius in cells. |

### `RiverParams`

| Field | Default | Description |
|-------|---------|-------------|
| `density: f32` | `0.01` | Approximate fraction of land cells covered by rivers. |

### `Terrain`

All buffers are row-major `Vec`s indexed as `y * width + x`.

| Field | Type | Description |
|-------|------|-------------|
| `width`, `height` | `usize` | Map dimensions. |
| `sea_level` | `f32` | The sea level the terrain was generated with. |
| `heightmap` | `Vec<f32>` | Elevation per cell, in `[0, 1]`. |
| `flow` | `Vec<f32>` | Log-normalised flow accumulation in `[0, 1]`; a useful moisture proxy for biome placement. |
| `rivers` | `Vec<u8>` | `1` where a river runs. Rivers are continuous from source to sea (or to a lake). |
| `lakes` | `Vec<u8>` | `1` where an inland depression holds standing water. |

Convenience accessors: `height_at(x, y)`, `flow_at(x, y)`,
`is_river(x, y)`, `is_lake(x, y)`, `is_water(x, y)` (sea, lake or river).

### Rendering

| Item | Description |
|------|-------------|
| `RenderStyle::Shaded` | Biome colours (ocean depth, beach, grass, forest, rock, snow) with Lambertian hillshading; rivers and lakes in blue. |
| `RenderStyle::Greyscale` | Plain heightmap, black = 0 to white = 1. |
| `Terrain::to_rgb(style) -> Vec<u8>` | Row-major `width * height * 3` RGB buffer — feed it to any image or texture API. |
| `Terrain::save_png(path, style) -> Result<(), RenderError>` | Renders and writes a PNG. |

`RenderError` wraps the underlying I/O or PNG encoding error and
implements `std::error::Error`.

## Performance

Indicative single-threaded timings (release build): a full 512² map with
default settings generates in ~0.5 s, 1024² in ~2 s. Erosion dominates;
lower `droplets` for faster previews.

## Full example

```rust
use terrain_generator::{
    ErosionParams, IslandParams, NoiseParams, RenderStyle, RiverParams, Terrain,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = Terrain::builder()
        .size(1024, 1024)
        .sea_level(0.08)
        .noise(NoiseParams {
            octaves: 8,
            frequency: 2.5,
            ..Default::default()
        })
        .island(IslandParams {
            size: 0.9,
            coast_noise: 0.35,
        })
        .erosion(ErosionParams {
            droplets: Some(2_000_000), // extra-deep valleys
            ..Default::default()
        })
        .rivers(RiverParams { density: 0.015 });

    for seed in 0..10 {
        let terrain = builder.clone().seed(seed).generate();
        terrain.save_png(format!("map{seed}.png"), RenderStyle::Shaded)?;
    }

    Ok(())
}
```
