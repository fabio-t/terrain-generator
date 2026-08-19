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
//! 4. **Hydrology** — D8 flow routing over the eroded terrain; cells with
//!    high flow accumulation become rivers, so rivers follow the carved
//!    valleys.
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
//! let h = terrain.height_at(100, 100); // elevation in [0, 1]
//! let wet = terrain.flow_at(100, 100); // flow accumulation in [0, 1]
//! let river = terrain.is_river(100, 100);
//! # let _ = (h, wet, river);
//! ```
//!
//! Every stage can be tuned or disabled through the builder; see
//! [`TerrainBuilder`], [`NoiseParams`], [`IslandParams`], [`ErosionParams`]
//! and [`RiverParams`]. Generation is fully deterministic for a given seed
//! and parameter set.

mod base;
mod erosion;
mod hydrology;
mod render;
mod rng;
mod terrain;

pub use render::{RenderError, RenderStyle};
pub use terrain::{ErosionParams, IslandParams, NoiseParams, RiverParams, Terrain, TerrainBuilder};
