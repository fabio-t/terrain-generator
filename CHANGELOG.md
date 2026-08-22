# Changelog

## Unreleased

### Added

- **Archipelago mode** (`ArchipelagoParams`, `--archipelago N`,
  `:archipelago`): several radial islands on one map, placed from the seed
  on a dedicated RNG stream. Single-island output is unchanged.
- **Basin breaching** (`BreachParams`, `--breach-basins`,
  `:breach_basins`), opt-in: carves an outlet along the minimax path out of
  each oversized closed basin so it drains instead of becoming an inland
  sea.
- **Ocean mask** (`Terrain::ocean`, `is_ocean`, `:ocean`): cells at or
  below sea level *and* connected to the map border, by flood fill. A
  closed basin below sea level is now correctly a lake rather than sea.
- **Raw flow accumulation** (`Terrain::flow_accumulation`,
  `flow_accumulation_at`, `:flow_accumulation`): upstream catchment in
  cells, for sizing channels. The normalised `flow` layer is unchanged.
- **`try_generate()`** returning `Result<Terrain, ParamError>` alongside
  the panicking `generate()`, for parameters that come from user input.
- **River threshold modes** (`RiverThreshold`): the existing `Density`
  area-share, plus absolute `MinUpstreamCells` and resolution-independent
  `PerEdge`. Documented when to use which.
- **Per-layer file output** (`Layer`, `Terrain::save_layer_png`,
  `--layers height,flow,rivers,lakes,ocean`): one generation, one file per
  layer. Elevation and flow are 16-bit greyscale; masks are 8-bit.

### Changed

- `is_water` is now the union of the ocean, lake and river masks rather
  than an elevation test, so enclosed low ground is no longer called sea.
- `RiverParams` is constructed via `RiverParams::density(..)` /
  `min_upstream_cells(..)` / `per_edge(..)` instead of a `density` field.
- Flow routing breaks ties on cell index, so ordering never depends on
  sort internals.

### Fixed

- Lake and river classification near below-sea-level basins (see the ocean
  mask above).

### Notes

- The heightmap layer is unchanged by all of the above: worlds stored as
  seeds keep the same elevation.

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
- Minimum supported Rust version: 1.91 (set by Rustler 0.38), declared
  once for the whole workspace and verified in CI.

## 0.1.1

Final Java/Maven version (fractal OpenSimplex noise, island mask,
random-walk rivers).
