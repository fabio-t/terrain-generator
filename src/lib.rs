//! Procedural terrain generation with fractal noise, hydraulic erosion and
//! river extraction.
//!
//! The pipeline runs in five stages; everything after the first is
//! optional:
//!
//! 1. **Base heightmap** — fractal (fBm) OpenSimplex noise via the
//!    [`noise`] crate.
//! 2. **Land mask** — either a single radial island
//!    ([`IslandParams`]) or an archipelago of several
//!    ([`ArchipelagoParams`]), both with noise-perturbed coastlines.
//! 3. **Hydraulic erosion** — a particle/droplet simulation that carves
//!    valleys and drainage networks into the terrain.
//! 4. **Basin breaching** *(opt-in)* — cuts outlets through the rims of
//!    oversized closed basins so they drain instead of becoming inland
//!    seas. See [`TerrainBuilder::breach_basins`].
//! 5. **Hydrology** — depressions are filled (priority-flood), then D8
//!    flow routing; cells with enough upstream catchment become rivers,
//!    filled depressions become lakes, and border-connected sea becomes
//!    the [`ocean`](Terrain::ocean) mask.
//!
//! # Quick start
//!
//! ```no_run
//! use terrain_generator::{RenderStyle, Terrain};
//!
//! let terrain = Terrain::builder()
//!     .size(512, 512)
//!     .seed(42)
//!     .generate();
//!
//! terrain.save_png("map.png", RenderStyle::Shaded).unwrap();
//!
//! // raw data access for games/simulations:
//! let h = terrain.heightmap[(100, 100)];          // elevation in [0, 1]
//! let wet = terrain.flow_at(100, 100);            // moisture proxy in [0, 1]
//! let catchment = terrain.flow_accumulation_at(100, 100); // cells upstream
//! let river = terrain.is_river(100, 100);
//! # let _ = (h, wet, catchment, river);
//! ```
//!
//! # What this crate does and does not decide
//!
//! The output is a set of raw, independent layers: elevation, flow
//! accumulation, and river/lake/ocean masks. It deliberately stops short
//! of gameplay or domain semantics — there is no "drinkable", no biome
//! enum, no navigability. Anything that is a judgement call is either a
//! parameter (how dense are rivers? how big may a lake be?) or derivable
//! from the raw layers, so a consumer can substitute its own rules.
//!
//! Consumers that bring their own hydrology should use
//! [`TerrainBuilder::no_rivers`], which skips flow routing entirely and
//! leaves [`Terrain::heightmap`] as the single source of truth.
//!
//! # Data model
//!
//! All per-cell outputs are [`Grid`]s: dense row-major buffers indexed as
//! `grid[(x, y)]`, with the flat storage available via [`Grid::as_slice`]
//! for zero-copy hand-off to FFI, GPUs or file formats.
//!
//! # Determinism
//!
//! The same seed and parameters produce the same terrain, run after run.
//! Ordering that could otherwise vary — the flow-routing sort and both
//! priority queues — is broken by cell index, so no result depends on
//! sort or heap internals.
//!
//! Two caveats worth knowing before you persist a world as a seed:
//!
//! * **Pin the version.** The base noise comes from the [`noise`] crate,
//!   and tuning inside this crate also changes output. If you regenerate
//!   terrain from a stored seed, depend on an exact tag or revision and
//!   commit your `Cargo.lock`; treat any change to the generated output as
//!   a breaking change on your side.
//! * **`flow` uses a transcendental.** [`Terrain::flow`] is log-normalised
//!   with `f32::ln`, whose last-bit rounding is not guaranteed identical
//!   across platforms and libm implementations. Every other layer —
//!   heightmap, [`flow_accumulation`](Terrain::flow_accumulation), rivers,
//!   lakes, ocean — is computed with IEEE-exact operations only
//!   (`+ - * /`, `sqrt`, comparisons), so those are reproducible
//!   bit-for-bit everywhere. If you need cross-platform exactness for
//!   moisture, derive it from `flow_accumulation` yourself.
//!
//! # Cargo features
//!
//! * `png` *(default)* — PNG output ([`Terrain::save_png`],
//!   [`Terrain::save_layer_png`]); pulls in the
//!   [`png`](https://crates.io/crates/png) crate. [`Terrain::to_rgb`]
//!   works without it.
//! * `cli` *(default)* — the `terrain-generator` command-line binary;
//!   pulls in `clap` and implies `png`. Library consumers should use
//!   `default-features = false`, which leaves `noise` as the only
//!   dependency.

mod base;
mod breach;
mod erosion;
mod grid;
mod hydrology;
mod render;
mod rng;
mod terrain;

pub use grid::Grid;
pub use hydrology::ocean_mask;
#[cfg(feature = "png")]
pub use render::RenderError;
pub use render::{Layer, RenderStyle, UnknownLayer};
pub use terrain::{
    ArchipelagoParams, BreachParams, ErosionParams, IslandParams, NoiseParams, ParamError,
    RiverParams, RiverThreshold, Terrain, TerrainBuilder,
};
