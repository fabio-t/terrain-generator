# Changelog

Notable changes to this project. Versions follow
[Semantic Versioning](https://semver.org).

Generated terrain is part of the public contract: any change to the output
for a given seed and parameter set is a breaking change, and is called out
here. If you regenerate worlds from stored seeds, pin an exact version.

## 0.3.0 — 2026-10-20

A desktop GUI for tuning, previewing and exporting terrain, contributed by
@romangrigorii. The libraries gain progress reporting, and `noise` is now
pinned exactly. The minor version moves because that pin can conflict
with a different `noise` 0.9 patch in your own dependency graph.

This is the first release published to
[crates.io](https://crates.io/crates/terrain-generator) and
[Hex](https://hex.pm/packages/terrain_generator), so neither library
needs a git dependency any more.

### GUI

- New `terrain-gui` desktop app in `gui/` (`cargo run --release
  --manifest-path gui/Cargo.toml`): every CLI generation option as a
  control, a live preview with a per-cell readout, progress while it
  generates, and export of the picture, the 16-bit heightmap, or every
  layer. It also shows the CLI command that reproduces the current map.
  Its exports are identical to the CLI's for the same settings.
  Contributed by @romangrigorii (#8).
- Not among the release downloads yet: build it from a clone with the
  command above. It is not published to crates.io, and it is a separate
  Cargo workspace, so none of the libraries depend on it or ship it.
  On Linux, its file dialogs use `xdg-desktop-portal`, which desktop
  sessions run.

### Rust library

- On crates.io: `terrain-generator = { version = "0.3",
  default-features = false }`, and `cargo install terrain-generator` for
  the CLI. API documentation is on docs.rs.
- `noise` is pinned to exactly 0.9.0. Before, it was `0.9`, so a
  `cargo update` could pull in a new 0.9 patch release, and with it
  different terrain, without this crate changing. If your own code
  depends on a different 0.9 patch of `noise`, it can no longer build
  alongside this crate.
- `TerrainBuilder::try_generate_with_progress` reports how far generation
  has got, as a fraction from 0 to 1 that never goes backwards plus a
  short stage name. `try_generate` is unchanged and now calls it with a
  no-op callback.
- A map may have at most `MAX_CELLS` cells (2³² − 1, so 65535 × 65535),
  and `try_generate` returns a `ParamError` for anything bigger. Before,
  such a map got past validation: hydrology's 32-bit cell indices
  wrapped, and in a release build `width * height` could too, giving a
  panic or wrong layers instead of an error.

### Elixir and Java

- The Elixir library is on Hex: `{:terrain_generator, "~> 0.3"}`. The
  NIF still compiles from source, so Rust 1.91+ is needed at compile
  time. Documentation is on HexDocs.
- The same cell limit applies. For a bigger map, the Elixir wrapper
  raises `ArgumentError`, the C ABI returns `TG_INVALID_PARAMS`, and Java
  throws `TerrainException`.
- No other API changes in either. The NIF, the C ABI and the jar are
  rebuilt on the updated dependencies.

No change to generated terrain: the same seed and parameters give the
same map as 0.2.x, progress callback or not.

## 0.2.1 — 2026-08-23

### Java

- The released jar now carries a **native library for every supported
  platform** inside it (`natives/<os>-<arch>/`), so a Java consumer needs
  no Rust toolchain and nothing to install — the pattern sqlite-jdbc and
  LWJGL use. Linux x86-64 (glibc 2.35+), macOS on Apple Silicon and Intel,
  and Windows x86-64 are covered.
- `TerrainGenerator` gained a third and final step in its library search:
  after the `tergen.library` property and the default library search path,
  it extracts the bundled copy for the running platform to a private
  temporary directory and loads that. A locally built library still wins,
  and a jar/native mismatch is still caught at class-init by the existing
  ABI-version check.
- The bindings are compiled with `--release 22`, so the jar loads on any
  Java 22 or newer runtime regardless of which JDK built it.
- `java/build.sh` now also packs the jar and runs the binding tests a
  second time against it with nothing on the library search path, so the
  extraction path cannot regress unnoticed. Tagged releases build the four
  native libraries, verify the loader on each platform's own runner, and
  attach `terrain-generator-<version>.jar` alongside the CLI archives.

- The release workflow can now be run manually from the Actions tab. Such
  a run builds and tests every artefact a release would carry but stops
  short of publishing one, so the packaging can be exercised on a branch
  without burning a tag.

No change to generated terrain, and none to the Rust or Elixir libraries:
this release touches only `java/` and the release workflow.

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
