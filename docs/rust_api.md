# Rust API

`terrain-generator` is an ordinary Rust library crate; add it to any
application with cargo. This document is a stable overview — the complete,
always-up-to-date reference is the rustdoc, which you can build locally
with `cargo doc --open`.

## Installation

Requires Rust 1.91 or newer. From a git dependency:

```toml
[dependencies]
terrain-generator = { git = "https://github.com/fabio-t/terrain-generator", default-features = false }
```

Features:

| Feature | Default | Effect |
|---------|---------|--------|
| `png`   | on (via `cli`) | PNG output (`Terrain::save_png`, `RenderError`); pulls in the `png` crate. `Terrain::to_rgb` works without it. |
| `cli`   | on      | Builds the `terrain-generator` binary; pulls in `clap` and implies `png`. |

With `default-features = false` the library has a single dependency
(`noise`) — add `features = ["png"]` back if you want PNG files.

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
let elevation = terrain.heightmap[(100, 100)]; // f32 in [0, 1]
let moisture  = terrain.flow_at(100, 100);     // f32 in [0, 1]
let has_river = terrain.is_river(100, 100);
let has_lake  = terrain.is_lake(100, 100);

// zero-copy access to the flat row-major buffer, e.g. for a GPU upload:
let raw: &[f32] = terrain.heightmap.as_slice();
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

Generation is **deterministic**: the same seed and parameters always
produce the same terrain. Ordering that could otherwise vary — the
flow-routing sort and both priority queues — is broken by cell index, so
no result depends on sort or heap internals.

Two caveats matter if you persist worlds as seeds:

* **Pin the version.** Output depends on the `noise` crate and on tuning
  inside this crate. Depend on an exact tag or revision, commit your
  `Cargo.lock`, and treat any output change as breaking on your side.
* **`flow` uses `f32::ln`,** whose last-bit rounding is not guaranteed
  identical across platforms and libm implementations. Every other layer
  — heightmap, `flow_accumulation`, rivers, lakes, ocean — uses only
  IEEE-exact operations (`+ - * /`, `sqrt`, comparisons) and is
  reproducible bit-for-bit everywhere. Derive moisture from
  `flow_accumulation` if you need cross-platform exactness.

## Types

### `Terrain::builder() -> TerrainBuilder`

The entry point. All builder methods take `self` and return `Self`, so
calls chain; `generate(&self)` can be called repeatedly (e.g. with
different seeds via `.seed(n)`).

| Method | Default | Description |
|--------|---------|-------------|
| `.size(width, height)` | `512 × 512` | Map dimensions in cells. `width * height` must be at most `MAX_CELLS` (2³² − 1, so 65535 × 65535). |
| `.seed(u32)` | `0` | Generation seed. |
| `.sea_level(f32)` | `0.08` | Sea level in `[0, 1)`. |
| `.noise(NoiseParams)` | see below | Fractal noise parameters. |
| `.island(IslandParams)` / `.no_island()` | enabled | Single-island shaping. |
| `.archipelago(ArchipelagoParams)` | off | Several islands instead of one; replaces `.island()`. |
| `.erosion(ErosionParams)` / `.no_erosion()` | enabled | Hydraulic erosion. |
| `.breach_basins(BreachParams)` / `.no_breach()` | off | Carve outlets from oversized closed basins. |
| `.rivers(RiverParams)` / `.no_rivers()` | enabled | Flow routing, rivers and lakes. |
| `.generate()` | — | Validates all parameters, runs the pipeline, returns a `Terrain`. Panics on invalid parameters. |
| `.try_generate()` | — | Same, but returns `Result<Terrain, ParamError>`. |
| `.try_generate_with_progress(\|fraction, stage\| ...)` | — | Same as `try_generate()`, and calls back with the fraction done (`0.0` to `1.0`, never backwards) and a stage label such as `"Erosion"`. |

All validation happens when generating. `generate()` **panics with a
descriptive message** on out-of-range parameters, treating them as a
programmer error. When the parameters come from user input — a config
file, a web form, a game's world-creation screen — use `try_generate()`
instead:

```rust
use terrain_generator::Terrain;

match Terrain::builder().sea_level(user_value).try_generate() {
    Ok(terrain) => { /* ... */ }
    Err(e) => eprintln!("bad configuration: {e} (parameter: {})", e.param),
}
```

`ParamError` exposes `param` (a stable `&'static str` naming the
offending parameter) and `message`, and implements `std::error::Error`.

## Heightmap-only generation

Consumers running their own hydrology should call `.no_rivers()`. This is
a supported first-class path, not a degraded mode: depression filling,
flow routing and river extraction are skipped entirely, and
`flow`/`flow_accumulation`/`rivers`/`lakes` come back all-zero.

```rust
use terrain_generator::Terrain;

let terrain = Terrain::builder()
    .size(1024, 1024)
    .seed(seed)
    .no_rivers()
    .generate();

let elevation: &[f32] = terrain.heightmap.as_slice(); // row-major, [0, 1]
```

The `ocean` mask is still computed — it is a linear-time flood fill over
the heightmap rather than hydrology — so "sea or inland basin?" stays
answerable. The heightmap is bit-identical whether hydrology runs or not;
hydrology never feeds back into elevation.

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
| `size: f32` | `1.0` | Island diameter as a fraction of the shorter map dimension. |
| `coast_noise: f32` | `1.0` | Coastline perturbation strength relative to the radial gradient; `0` gives a round island, `1` carves bays, peninsulas and offshore islets. |

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
| `threshold: RiverThreshold` | `Density(0.01)` | How the river network is cut out of flow accumulation. |

Build it with one of the constructors, one per `RiverThreshold` variant:

| Constructor | Range | A cell is a river when |
|-------------|-------|------------------------|
| `RiverParams::density(f)` | `[0, 1]` | it is in the top `f` of land cells by upstream catchment, so rivers cover about `f` of the land. |
| `RiverParams::min_upstream_cells(n)` | `≥ 0` | at least `n` cells drain through it. |
| `RiverParams::per_edge(k)` | `≥ 0` | at least `k × max(width, height)` cells drain through it. |

`density` is relative, so it reads the same on every map, but a small map
gets a visually denser network than a large one at the same value:
catchments grow with area, channels with length. `min_upstream_cells` is
absolute, the choice when a cell has a fixed real-world size, as in a
world tiled from several maps. `per_edge` scales with the map edge, for a
map that is always "one island" whatever its resolution.

### `ArchipelagoParams`

Several islands instead of one. Placement comes from the seed on its own
RNG stream, so enabling archipelago mode cannot change any single-island
or mask-less output.

| Field | Default | Description |
|-------|---------|-------------|
| `islands: usize` | `5` | How many islands to place. |
| `size: f32` | `0.35` | Mean island diameter as a fraction of the shorter map side. |
| `size_variation: f32` | `0.35` | Per-island size variation, in `[0, 1)`. |
| `spread: f32` | `0.85` | How much of the map centres may spread over, in `[0, 1]`. |
| `min_spacing: f32` | `0.22` | Minimum centre spacing as a fraction of the shorter map side (best-effort). |
| `coast_noise: f32` | `1.0` | Coastline perturbation, as for a single island. |

`ArchipelagoParams::with_islands(n)` is the defaults with `n` islands.

The mask is the union of per-island radial falloffs — the single-island
formula with `d³` replaced by the *nearest* island's `d³` — so islands
are shaped exactly like single islands and the coastline noise is applied
once, globally.

**Why not a noise threshold?** Thresholding a noise field gives organic
chains but no control: the island count falls out of the noise and jumps
around with small parameter changes. Explicit radial falloffs keep
`islands` and `size` as real controls. The trade-off is that neighbours
can overlap into one landmass, so the count is approximate — raise
`min_spacing` or lower `size` to separate them.

Erosion and hydrology need no changes for this: each landmass gets its
own drainage, and the water between islands stays ocean.

### `BreachParams`

Opt-in stage between erosion and hydrology. Droplet erosion deposits into
pits but cannot cut a rim tens of metres high, so large interior
depressions survive and become inland seas once depressions are filled.
Breaching finds each oversized basin's **minimax path** out — the path
whose highest cell is lowest, i.e. where water would wear through first —
and carves it into a channel descending from the basin floor to the exit.

| Field | Default | Description |
|-------|---------|-------------|
| `max_lake_fraction: f32` | `0.005` | Largest closed basin, as a fraction of map cells, that may remain a lake. |

A basin whose floor is lower than every reachable cell has nowhere to
drain to and is left alone. Carving only ever lowers terrain.

### `Grid<T>`

Every per-cell buffer is a `Grid<T>`: a dense, row-major 2D grid indexed
as `grid[(x, y)]`. Key methods: `width()`, `height()`, `len()`,
`as_slice()` / `as_mut_slice()` (the flat row-major buffer, for zero-copy
FFI or GPU hand-off), `into_vec()`, `iter()`, and `idx(x, y)` /
`coords(idx)` for converting between coordinates and flat indices.

### `Terrain`

| Field | Type | Description |
|-------|------|-------------|
| `sea_level` | `f32` | The sea level the terrain was generated with. |
| `heightmap` | `Grid<f32>` | Elevation per cell, in `[0, 1]`. |
| `flow_accumulation` | `Grid<f32>` | Raw upstream catchment per cell, in cells (each contributes 1). Size channels from this — width typically scales with √catchment. |
| `flow` | `Grid<f32>` | `flow_accumulation` log-normalised to `[0, 1]`; the more useful form as a moisture proxy. |
| `rivers` | `Grid<u8>` | `1` where a river runs. Continuous from source to sea or to a lake. |
| `lakes` | `Grid<u8>` | `1` where an inland depression holds standing water. |
| `ocean` | `Grid<u8>` | `1` where the cell is at or below sea level **and** connected to the map border. |

Convenience accessors: `width()`, `height()`, `height_at(x, y)`,
`flow_at(x, y)`, `flow_accumulation_at(x, y)`, `is_river(x, y)`,
`is_lake(x, y)`, `is_ocean(x, y)`, `is_water(x, y)` (the union of the
three water masks).

### Ocean vs. inland water

`ocean` is a flood fill from the map border across cells at or below sea
level, **not** an elevation test. A closed basin whose floor lies below
sea level is therefore *not* ocean: it is a lake. This matters for
anything modelling salinity, drinkability or navigability — an elevation
test alone silently calls such a basin sea.

`is_water` is the union of the three masks, so it never makes that
mistake. If you classify water yourself, build on `ocean`/`lakes` rather
than on `height_at(..) <= sea_level`.

### Rendering

| Item | Description |
|------|-------------|
| `RenderStyle::Shaded` | Biome colours (ocean depth, beach, grass, forest, rock, snow) with Lambertian hillshading; rivers and lakes in blue. |
| `RenderStyle::Greyscale` | Plain heightmap, black = 0 to white = 1. |
| `Terrain::to_rgb(style) -> Vec<u8>` | Row-major `width * height * 3` RGB buffer — feed it to any image or texture API. Always available. |
| `Terrain::save_png(path, style) -> Result<(), RenderError>` | Renders and writes a PNG. Requires the `png` feature. |
| `Terrain::save_layer_png(layer, path)` | Writes one `Layer` as a PNG. Requires the `png` feature. |

### Layers

`Layer` selects one output layer for file export: `Shaded`, `Height`,
`Flow`, `Rivers`, `Lakes`, `Ocean`. `Shaded` is a picture; the rest are
data, written losslessly — elevation and flow as **16-bit** greyscale
(8 bits would quantise elevation into visible terraces), masks as 8-bit
black/white. `Layer` implements `Display` and `FromStr` (lowercase names)
so it round-trips through CLI arguments and config files.

`RenderError` wraps the underlying I/O or PNG encoding error and
implements `std::error::Error`.

## Performance

Single-threaded, release build. Erosion dominates: on one desktop core a
default 1024² map takes a few seconds, of which roughly three quarters is
the droplet simulation and well under a tenth is hydrology (flow routing,
ocean/lake/river extraction). A 512² map is around a quarter of that.
Treat these as orders of magnitude, not benchmarks — they vary several-fold
across machines.

To speed things up: lower `ErosionParams::droplets` (the single biggest
lever), or use `no_erosion()` for layout previews. `no_rivers()` saves
comparatively little, so choose it for correctness reasons — bringing your
own hydrology — rather than for speed.

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
            size: 1.0,
            coast_noise: 1.0,
        })
        .erosion(ErosionParams {
            droplets: Some(2_000_000), // extra-deep valleys
            ..Default::default()
        })
        .rivers(RiverParams::density(0.015));

    for seed in 0..10 {
        let terrain = builder.clone().seed(seed).generate();
        terrain.save_png(format!("map{seed}.png"), RenderStyle::Shaded)?;
    }

    Ok(())
}
```
