# Changelog

## 0.2.0 (unreleased)

Complete rewrite in Rust, replacing the original Java implementation.

- **Pipeline**: fractal (fBm) OpenSimplex noise via the `noise` crate,
  radial island mask with noise-perturbed coastlines, particle-based
  hydraulic erosion (Beyer 2015), priority-flood depression filling
  (Barnes et al. 2014) and D8 flow accumulation. Rivers follow eroded
  valleys; filled depressions become lakes.
- **Outputs**: heightmap, flow/moisture map, river mask and lake mask as
  dense row-major `Grid`s; shaded (hillshaded biome colours) and greyscale
  PNG rendering.
- **Interfaces**: `terrain-generator` CLI (cargo `cli` feature, on by
  default), Rust library (`TerrainBuilder`), and an Elixir package
  wrapping the core through a Rustler NIF on dirty CPU schedulers.
- **Determinism**: identical output for a given seed and parameter set on
  every platform.
- Cargo features: `png` (PNG output) and `cli` (binary); library
  consumers can depend with `default-features = false`.
- Toolchain floors: the library and CLI need Rust 1.85; the Elixir NIF
  crate needs Rust 1.91 (required by Rustler 0.38).

## 0.1.1

Final Java/Maven version (fractal OpenSimplex noise, island mask,
random-walk rivers).
