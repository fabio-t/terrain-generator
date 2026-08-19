//! Procedural terrain generation with fractal noise, hydraulic erosion and
//! river extraction.
//!
//! The pipeline has four stages, each optional after the first:
//!
//! 1. **Base heightmap** — fractal (fBm) OpenSimplex noise via the
//!    [`noise`] crate.
//! 2. **Island mask** — a radial falloff with noise-perturbed coastlines.
//! 3. **Hydraulic erosion** — a particle/droplet simulation that carves
//!    valleys and drainage networks into the terrain.
//! 4. **Hydrology** — depressions are filled (priority-flood), then D8
//!    flow routing over the eroded terrain; cells with high flow
//!    accumulation become rivers and filled depressions become lakes, so
//!    water features follow the carved valleys.
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
//! let h = terrain.heightmap[(100, 100)]; // elevation in [0, 1]
//! let wet = terrain.flow_at(100, 100);   // flow accumulation in [0, 1]
//! let river = terrain.is_river(100, 100);
//! # let _ = (h, wet, river);
//! ```
//!
//! Every stage can be tuned or disabled through the builder; see
//! [`TerrainBuilder`], [`NoiseParams`], [`IslandParams`], [`ErosionParams`]
//! and [`RiverParams`].
//!
//! # Data model
//!
//! All per-cell outputs are [`Grid`]s: dense row-major buffers indexed as
//! `grid[(x, y)]`, with the flat storage available via [`Grid::as_slice`]
//! for zero-copy hand-off to FFI, GPUs or file formats.
//!
//! # Determinism
//!
//! Generation is fully deterministic for a given seed and parameter set on
//! every platform. Note that the base noise comes from the [`noise`] crate:
//! upgrading that dependency across breaking versions may change the
//! generated terrain, so pin versions if you rely on stable worlds.
//!
//! # Cargo features
//!
//! * `png` *(default)* — PNG output ([`Terrain::save_png`]); pulls in the
//!   [`png`](https://crates.io/crates/png) crate. [`Terrain::to_rgb`] works
//!   without it.
//! * `cli` *(default)* — the `terrain-generator` command-line binary;
//!   pulls in `clap`. Library consumers should use
//!   `default-features = false` (plus `features = ["png"]` if they want
//!   PNG output).

mod base;
mod erosion;
mod grid;
mod hydrology;
mod render;
mod rng;
mod terrain;

pub use grid::Grid;
#[cfg(feature = "png")]
pub use render::RenderError;
pub use render::RenderStyle;
pub use terrain::{ErosionParams, IslandParams, NoiseParams, RiverParams, Terrain, TerrainBuilder};
