# Terrain generator

[![CI](https://github.com/fabio-t/terrain-generator/actions/workflows/ci.yml/badge.svg)](https://github.com/fabio-t/terrain-generator/actions/workflows/ci.yml)
[![Latest Github release](https://img.shields.io/github/release/fabio-t/terrain-generator.svg)](https://github.com/fabio-t/terrain-generator/releases/latest)
[![crates.io](https://img.shields.io/crates/v/terrain-generator.svg)](https://crates.io/crates/terrain-generator)
[![Hex.pm](https://img.shields.io/hexpm/v/terrain_generator.svg)](https://hex.pm/packages/terrain_generator)
[![docs.rs](https://img.shields.io/docsrs/terrain-generator)](https://docs.rs/terrain-generator)

Procedural terrain generator written in Rust. It builds a fractal-noise
heightmap, shapes it into an island with noisy coastlines, carves it with
particle-based **hydraulic erosion**, and routes water over the result so
that **rivers follow the eroded valleys** and depressions become lakes.

![example map](https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/example.png)

It is usable five ways:

- as a **CLI** that renders maps to PNG,
- as a **desktop GUI** for trying settings with a live preview,
- as a **Rust library** (`cargo` dependency),
- as an **Elixir library** via a Rustler NIF (`mix` dependency),
- as a **Java library** via the Foreign Function & Memory API — and, through
  the same C ABI, from C, C++, C# or anything else with an FFI.

Generation is fully deterministic: the same seed and parameters always
produce the same terrain, from every entry point. It is single-threaded
and erosion dominates the cost: on a typical desktop core, expect roughly
a second for a 512×512 map and a few seconds for 1024×1024. Lower
`--droplets` for faster previews.

## Pipeline

1. **Base heightmap** — fractal (fBm) OpenSimplex noise from the
   [`noise`](https://crates.io/crates/noise) crate.
2. **Land mask** — a single radial island, or an archipelago of several,
   perturbed by a secondary noise field for natural coastlines.
3. **Hydraulic erosion** — droplet simulation (Beyer 2015): rain droplets
   roll downhill, pick up sediment and deposit it on flats, carving
   valleys and drainage networks.
4. **Basin breaching** *(opt-in)* — cuts outlets through the rims of
   oversized closed basins so they drain instead of becoming inland seas.
5. **Hydrology** — priority-flood depression filling (Barnes et al. 2014)
   plus D8 flow accumulation; high-flow cells become rivers, filled
   depressions become lakes, and border-connected sea becomes the ocean
   mask.

The output is not just an image: you get elevation, raw flow accumulation
(upstream catchment per cell), and river, lake and ocean masks as separate
raw layers for use in games or simulations.

## Installing the CLI

### Prebuilt binaries

Grab an archive for your platform from the
[latest release](https://github.com/fabio-t/terrain-generator/releases/latest):

| Platform | File |
|----------|------|
| Linux x86_64 (static, runs on any distro) | `terrain-generator-<tag>-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `terrain-generator-<tag>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `terrain-generator-<tag>-x86_64-apple-darwin.tar.gz` |
| Windows x86_64 | `terrain-generator-<tag>-x86_64-pc-windows-msvc.zip` |

Unpack it and run the `terrain-generator` binary inside — there are no
other files to install. `SHA256SUMS.txt` on the release page has the
checksums.

### With cargo

Requires a [Rust toolchain](https://rustup.rs) (1.91 or newer):

```sh
cargo install terrain-generator
```

Add `--git https://github.com/fabio-t/terrain-generator` for the
unreleased state of `main`.

### From a clone

```sh
git clone https://github.com/fabio-t/terrain-generator
cd terrain-generator
cargo run --release            # generates map0.png in the current directory
```

## Using the CLI

```sh
terrain-generator                          # one 512x512 map, seed 0 -> map0.png
terrain-generator -s 10 -n 5               # five maps, seeds 10..14
terrain-generator -W 1024 -H 1024          # bigger map
terrain-generator --rivers 0.02 --island 0.7 --droplets 2000000
terrain-generator --grey -o heightmaps/    # greyscale heightmaps into a directory
terrain-generator --help                   # all options
```

Every pipeline stage can be tuned (`--octaves`, `--frequency`, `--island`,
`--coast-noise`, `--droplets`, `--rivers`, `--sea-level`, ...) or disabled
(`--no-island`, `--no-erosion`, `--no-rivers`). The same seed with the
same options always reproduces the same map.

## What the options do

Every map below uses **seed 1** at 512×512; each row changes a single
option against the defaults. The same knobs exist under the same names in
the Rust API (`IslandParams`, `ErosionParams`, ...) and the Elixir API
(`:island`, `:coast_noise`, `:erosion`, `:rivers`, `:sea_level`, ...).

### Island shape — `--coast-noise`

How strongly noise perturbs the radial island falloff: `0` is a perfectly
round island, `1` (the default) carves bays, peninsulas and offshore
islets.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/coast-noise-0.png" width="240"><br><code>--coast-noise 0</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/coast-noise-05.png" width="240"><br><code>--coast-noise 0.5</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br><code>--coast-noise 1</code> (default)</td>
</tr></table>

### Island size — `--island` / `--no-island`

Island diameter as a fraction of the shorter map dimension, or no island
mask at all (terrain runs to the borders — useful for tiling a larger
world out of maps).

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/island-07.png" width="240"><br><code>--island 0.7</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br><code>--island 1.0</code> (default)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/no-island.png" width="240"><br><code>--no-island</code></td>
</tr></table>

### Erosion — `--no-erosion` / `--droplets`

Without erosion the raw fractal terrain is smooth and blobby. The default
simulates one droplet per cell; more droplets keep carving — valleys
deepen, sediment builds plains, coastlines erode.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/no-erosion.png" width="240"><br><code>--no-erosion</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br>default (262k droplets)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/droplets-1m.png" width="240"><br><code>--droplets 1000000</code></td>
</tr></table>

### River density — `--rivers`

The approximate fraction of land covered by rivers (extracted from flow
accumulation, so they always follow the valleys). Shown as 1:1 crops so
the individual rivers are visible.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/rivers-0003.png" width="240"><br><code>--rivers 0.003</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/rivers-001.png" width="240"><br><code>--rivers 0.01</code> (default)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/rivers-004.png" width="240"><br><code>--rivers 0.04</code></td>
</tr></table>

A share of the land gives a different river network at every map size.
Two thresholds hold steady instead, and either replaces `--rivers`:
`--river-min-upstream N` makes a river wherever at least `N` cells drain
through, for when a cell has a fixed real-world size, and
`--river-per-edge K` wherever the catchment reaches `K` times the longer
map edge, for a same-looking network at every resolution.

### Sea level — `--sea-level`

Cells at or below this elevation are ocean. Raising it drowns the
lowlands and turns valley systems into fjord-like inlets.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/sea-level-003.png" width="240"><br><code>--sea-level 0.03</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br><code>--sea-level 0.08</code> (default)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/sea-level-02.png" width="240"><br><code>--sea-level 0.2</code></td>
</tr></table>

### Base noise — `--frequency` / `--octaves`

`--frequency` is the number of large features across the map (defaults to
`2.5`): lower gives one big landmass, higher gives busier, more broken
terrain. `--octaves` (default `8`) controls fine detail; fewer octaves
give smoother, softer relief. Between octaves, `--lacunarity` (default
`2`) multiplies the frequency and `--persistence` (default `0.5`) the
amplitude: a higher persistence keeps more of the fine detail.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/frequency-15.png" width="240"><br><code>--frequency 1.5</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/frequency-4.png" width="240"><br><code>--frequency 4</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/octaves-4.png" width="240"><br><code>--octaves 4</code></td>
</tr></table>

### Archipelagos — `--archipelago`

`--archipelago N` places N islands instead of one, with `--islands-size`,
`--islands-spacing`, `--islands-spread` and `--islands-variation` to shape
the group. Placement comes from the seed, so it is reproducible; the count
is approximate, because neighbours can merge and coastline noise can split
an island in two. Erosion and rivers work per landmass.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br>single island (default)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/archipelago-5.png" width="240"><br><code>--archipelago 5</code></td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/archipelago-9.png" width="240"><br><code>--archipelago 9 --islands-size 0.22</code></td>
</tr></table>

### Draining big basins — `--breach-basins`

Droplet erosion fills pits but cannot cut through a high rim, so large
interior depressions survive and become inland seas. `--breach-basins F`
carves an outlet from every closed basin bigger than fraction `F` of the
map, along the path where water would wear through first, leaving valleys
instead of lakes.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/breach-off.png" width="240"><br>default (basins stay lakes)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/breach-on.png" width="240"><br><code>--breach-basins 0.002</code></td>
</tr></table>

### Output style — `--grey`

The shaded render is for looking at; the greyscale render is the raw
heightmap (black = 0, white = 1), ready to import into game engines and
other tools.

<table><tr>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/default.png" width="240"><br>default (shaded)</td>
<td align="center"><img src="https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/options/greyscale.png" width="240"><br><code>--grey</code></td>
</tr></table>

## Getting the data out — `--layers`

One generation can emit every layer as its own file, for feeding into
other tools:

```sh
terrain-generator --layers height,flow,rivers,lakes,ocean -o out/
```

This writes `map0_height.png`, `map0_flow.png` and so on. Elevation and
flow are written as **16-bit** greyscale so nothing is lost to
quantisation (8 bits would terrace the elevation); the masks are 8-bit
black/white. `shaded` is also available as a layer name, and remains the
default when `--layers` is not given.

## Desktop GUI

![the desktop GUI](https://github.com/fabio-t/terrain-generator/raw/HEAD/docs/gui.png)

```sh
cargo run --release --manifest-path gui/Cargo.toml
```

Every CLI generation option as a control, a preview that shows the height
and water type of the cell under the pointer, and export of the picture,
the 16-bit heightmap or all layers. **Generate** builds the map on a
background thread; the *CLI command* panel gives the `terrain-generator`
invocation for the current settings, and its exports match the CLI's
byte for byte. Use `--release`: erosion is many times slower in a debug
build.

On Linux the file dialogs go through the desktop portal
(`xdg-desktop-portal`), which every desktop session runs.

Contributed by [@romangrigorii](https://github.com/romangrigorii) in
[#8](https://github.com/fabio-t/terrain-generator/pull/8).

## Rust library

```toml
[dependencies]
# add features = ["png"] if you want PNG file output; without it the
# library's only dependency is the `noise` crate
terrain-generator = { version = "0.3", default-features = false }
```

API documentation is on [docs.rs](https://docs.rs/terrain-generator).

```rust
use terrain_generator::{RenderStyle, Terrain};

let terrain = Terrain::builder().size(512, 512).seed(42).generate();

let elevation = terrain.heightmap[(100, 100)]; // [0, 1]
let moisture  = terrain.flow_at(100, 100);     // [0, 1], from flow accumulation
let river     = terrain.is_river(100, 100);
let lake      = terrain.is_lake(100, 100);

// with the `png` feature:
terrain.save_png("map.png", RenderStyle::Shaded)?;
```

Consumers who run their own hydrology can call `.no_rivers()`: flow
routing is skipped entirely and `heightmap` is the single source of truth.
The `ocean` mask is still computed, so "sea or inland basin?" stays
answerable without it.

Full reference: [docs/rust_api.md](https://github.com/fabio-t/terrain-generator/blob/HEAD/docs/rust_api.md), or `cargo doc --open`.

### Determinism and stored worlds

The same seed and parameters always produce the same terrain. If you
persist worlds **as seeds** and regenerate them on load, pin an exact version
of this crate and commit your `Cargo.lock`. Any change to the output is a
breaking change for your saves. The output also depends on the `noise`
crate, which this crate pins to an exact version, so `noise` changes
only when this crate does.

One exception to cross-platform bit-exactness: the normalised `flow` layer
is computed with `f32::ln`, whose last-bit rounding can differ between
platforms. `heightmap`, `flow_accumulation`, `rivers`, `lakes` and `ocean`
use only IEEE-exact operations and are identical everywhere.

## Elixir library

A [Hex package](https://hex.pm/packages/terrain_generator) wraps the same
Rust core through [Rustler](https://hexdocs.pm/rustler). The NIF compiles
from source when you fetch it, so Erlang/OTP 24+, Elixir 1.15+ and Rust
1.91+ (required by Rustler 0.38) are needed at compile time.

```elixir
def deps do
  [{:terrain_generator, "~> 0.3"}]
end
```

```elixir
{:ok, terrain} = TerrainGenerator.generate(width: 512, height: 512, seed: 42)
TerrainGenerator.Terrain.height_at(terrain, 100, 100)
#=> 0.53

:ok = TerrainGenerator.render_png("map.png", seed: 42)
```

Heightmap, flow, river and lake buffers are returned as plain binaries
(32-bit floats / bytes), cheap to pass around the BEAM. The NIF runs on a
dirty CPU scheduler, so generation never blocks your schedulers.

Full reference: [HexDocs](https://hexdocs.pm/terrain_generator), or
[docs/elixir_api.md](https://github.com/fabio-t/terrain-generator/blob/HEAD/docs/elixir_api.md).

## Java library

The generator exposes a plain **C ABI**, which Java 22+ binds directly
through the [Foreign Function & Memory API][ffm] — no JNI shim, and no
Java-specific code inside the Rust library. The Java sources have no
dependencies, so they drop into Maven, Gradle or a plain classpath.

### Using the released jar

Every release attaches `terrain-generator-<version>.jar`, which carries a
native library for each supported platform inside it:

| | |
|---|---|
| Linux | x86-64 (glibc 2.35+) |
| macOS | Apple Silicon and Intel |
| Windows | x86-64 |

Put it on the classpath and you are done — **no Rust toolchain, nothing to
install, nothing to build**:

```sh
java --enable-native-access=ALL-UNNAMED -cp terrain-generator-0.3.1.jar:. YourApp
```

The binding loads its native library from the first of these that works:

1. the path in the `tergen.library` system property, if set;
2. the default library search path;
3. the copy bundled in the jar for this platform, extracted to a
   temporary file.

So a locally built library still wins while you are working on the Rust
side, and everyone else gets step 3 without knowing it exists. The jar is
compiled for Java 22, and mismatched jar/native combinations are caught at
class-init by an ABI-version check rather than by misreading memory.

### Building from source

```sh
./java/build.sh   # builds the native library, packs the jar, runs the binding tests
```

That runs the tests twice: once against the freshly built library and once
against the jar alone, so the extraction path is covered too.

```java
import com.github.fabioticconi.tergen.*;

TerrainConfig config = TerrainConfig.defaults().size(512, 512).seed(42);

try (Terrain terrain = TerrainGenerator.generate(config)) {
    float elevation = terrain.heightAt(100, 100);
    boolean river   = terrain.isRiver(100, 100);
    terrain.savePng(Layer.SHADED, Path.of("map.png"));
}
```

Layers can be copied into Java arrays or borrowed as a zero-copy
`MemorySegment`. The binding tests assert that a map generated from Java
hashes to the same checksum as the Rust test suite pins, so the three
language bindings are known to produce identical terrain.

Full reference: [docs/java_api.md](https://github.com/fabio-t/terrain-generator/blob/HEAD/docs/java_api.md). The C interface
itself is [`include/terrain_generator.h`](https://github.com/fabio-t/terrain-generator/blob/HEAD/include/terrain_generator.h), if
you would rather use `jextract` or call it from C/C++.

[ffm]: https://docs.oracle.com/en/java/javase/22/core/foreign-function-and-memory-api.html

## Development

```sh
cargo test --workspace     # Rust unit, integration and doc tests
cargo clippy --workspace   # lints
mix test                   # Elixir tests (needs Elixir + Rust)
./java/build.sh            # Java jar + binding tests (needs JDK 22+ and Rust)
```

CI (formatting, clippy, both test suites) runs automatically on pushes
to `main`, and on demand for any other branch via **Run workflow** on
the [Actions tab](https://github.com/fabio-t/terrain-generator/actions/workflows/ci.yml)
— pull requests are not built automatically. Releases
are cut by running the Release workflow on `main` with `cut`, which tags
`main` at its version, or by pushing a tag: `v1.2.3` (or `1.2.3`)
publishes a release with
CLI binaries for Linux, macOS and Windows attached, plus the Java jar with
every platform's native library inside it, and release notes taken from
that version's CHANGELOG section, then publishes to crates.io and Hex; a
pre-release tag like `v1.2.3-rc1` publishes a GitHub pre-release only.

## License

Apache-2.0
