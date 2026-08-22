# Changelog

Notable changes to this project. Versions follow
[Semantic Versioning](https://semver.org).

Generated terrain is part of the public contract: any change to the output
for a given seed and parameter set is a breaking change, and is called out
here. If you regenerate worlds from stored seeds, pin an exact version.

## 0.2.0 — 2026-08-22

Complete rewrite in Rust, replacing the original Java implementation. The
project is now a workspace producing **separable terrain data layers**,
consumable from a CLI, Rust, Elixir and Java.

### Pipeline

Five stages; everything after the first is optional:

- **Base heightmap** — fractal (fBm) OpenSimplex noise via the `noise`
  crate, with resolution-independent frequency.
- **Land mask** — a single radial island, or an **archipelago** of several
  (`ArchipelagoParams`, `--archipelago N`, `:archipelago`), placed from the
  seed on a dedicated RNG stream so single-island output is unaffected.
  Coastlines are perturbed by a secondary noise field.
- **Hydraulic erosion** — particle/droplet simulation (Beyer 2015) carving
  dendritic valleys and drainage networks.
- **Basin breaching** *(opt-in)* — `BreachParams`, `--breach-basins`,
  `:breach_basins`: carves an outlet along the minimax path out of each
  oversized closed basin, so it drains instead of becoming an inland sea.
  Basins with no lower ground reachable are left alone; carving only ever
  lowers terrain.
- **Hydrology** — priority-flood depression filling (Barnes et al. 2014)
  and D8 flow accumulation. Rivers follow the eroded valleys, filled
  depressions become lakes, and border-connected sea becomes the ocean
  mask.

This replaces the previous random-walk river generation entirely.

### Output layers

All layers are dense row-major `Grid`s with zero-copy `as_slice()`:

- `heightmap` — elevation in `[0, 1]`.
- `flow_accumulation` — raw upstream catchment in cells, for sizing
  channels.
- `flow` — the same, log-normalised to `[0, 1]` as a moisture proxy.
- `rivers`, `lakes`, `ocean` — masks.

**Ocean is connectivity, not elevation**: a flood fill from the map border,
so a closed basin below sea level is a lake rather than sea. `is_water` is
the union of the three masks.

The library deliberately stops short of domain semantics — no biome enum,
no "drinkable". Judgement calls are parameters or derivable from the raw
layers.

### Interfaces

- **CLI** — every stage tunable or disableable;
  `--layers height,flow,rivers,lakes,ocean` writes one file per layer from
  a single generation. Elevation and flow are 16-bit greyscale so nothing
  is lost to quantisation; masks are 8-bit.
- **Rust library** — `Terrain::builder()`. With
  `default-features = false` the only dependency is `noise`. `png` and
  `cli` are cargo features.
- **Elixir** — Mix package wrapping a Rustler NIF on dirty CPU schedulers;
  layers returned as off-heap binaries, options validated Elixir-side.
- **Java** — Java 22+ binds a new **C ABI**
  (`native/terrain_generator_ffi`, `include/terrain_generator.h`) directly
  through the Foreign Function & Memory API: no JNI shim, and no
  Java-specific code in the Rust sources. The same C interface serves C,
  C++, C# and other FFI-capable languages.

### API

- `try_generate()` returns `Result<Terrain, ParamError>` alongside the
  panicking `generate()`, for parameters that come from user input.
- River thresholds are a choice (`RiverThreshold`): area-share `Density`,
  absolute `MinUpstreamCells`, or resolution-independent `PerEdge`.
- `no_rivers()` is a documented first-class path for consumers running
  their own hydrology: flow routing is skipped entirely, the heightmap is
  bit-identical either way, and the ocean mask is still computed.

### Determinism

- Identical output for a given seed and parameter set, from every binding.
  The Java binding tests assert the same checksum the Rust suite pins.
- Ordering that could otherwise vary — the flow-routing sort and both
  priority queues — is broken by cell index, so no result depends on sort
  or heap internals.
- Caveat: the normalised `flow` layer is computed with `f32::ln`, whose
  last-bit rounding is not guaranteed identical across platforms. Every
  other layer uses IEEE-exact operations only.

### Tooling

- Minimum supported Rust version 1.91 (set by Rustler 0.38), declared once
  for the whole workspace and verified in CI.
- CI covers Rust (fmt, clippy `-D warnings`, tests across feature
  combinations, MSRV), Elixir and Java.
- Tagged releases build CLI binaries for Linux (static musl), macOS
  (Intel and Apple Silicon) and Windows, with checksums attached.

### Removed

- All Java sources, `pom.xml` and the IntelliJ module file. The 1:1 Rust
  port of the Java generator, which produced byte-identical output to it,
  is preserved in history at `dcaad8c`.

## 0.1.1

Final Java/Maven version (fractal OpenSimplex noise, island mask,
random-walk rivers).
