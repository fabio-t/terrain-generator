# Terrain generator

[![CI](https://github.com/fabio-t/terrain-generator/actions/workflows/ci.yml/badge.svg)](https://github.com/fabio-t/terrain-generator/actions/workflows/ci.yml)
[![Latest Github release](https://img.shields.io/github/release/fabio-t/terrain-generator.svg)](https://github.com/fabio-t/terrain-generator/releases/latest)

Procedural terrain generator written in Rust. It builds a fractal-noise
heightmap, shapes it into an island with noisy coastlines, carves it with
particle-based **hydraulic erosion**, and routes water over the result so
that **rivers follow the eroded valleys** and depressions become lakes.

![example map](docs/example.png)

It is usable three ways:

- as a **CLI** that renders maps to PNG,
- as a **Rust library** (`cargo` dependency),
- as an **Elixir library** via a Rustler NIF (`mix` dependency).

Generation is fully deterministic: the same seed and parameters always
produce the same terrain, from every entry point. A 512×512 map generates
in about half a second, 1024×1024 in about two.

## Pipeline

1. **Base heightmap** — fractal (fBm) OpenSimplex noise from the
   [`noise`](https://crates.io/crates/noise) crate.
2. **Island mask** — radial falloff, perturbed by a secondary noise field
   for natural coastlines.
3. **Hydraulic erosion** — droplet simulation (Beyer 2015): rain droplets
   roll downhill, pick up sediment and deposit it on flats, carving
   valleys and drainage networks.
4. **Hydrology** — priority-flood depression filling (Barnes et al. 2014)
   plus D8 flow accumulation; high-flow cells become rivers, filled
   depressions become lakes.

The output is not just an image: you get the heightmap, a flow/moisture
map, and river and lake masks as raw data for use in games or simulations.

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

Requires a [Rust toolchain](https://rustup.rs) (1.85 or newer):

```sh
cargo install --git https://github.com/fabio-t/terrain-generator terrain-generator
```

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

## Rust library

```toml
[dependencies]
# add features = ["png"] if you want PNG file output; without it the
# library's only dependency is the `noise` crate
terrain-generator = { git = "https://github.com/fabio-t/terrain-generator", default-features = false }
```

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

Full reference: [docs/rust_api.md](docs/rust_api.md), or `cargo doc --open`.

## Elixir library

The repository is also a Mix package wrapping the same Rust core through
[Rustler](https://hexdocs.pm/rustler). Erlang/OTP 24+, Elixir 1.15+ and a
Rust toolchain are required at compile time.

```elixir
def deps do
  [{:terrain_generator, github: "fabio-t/terrain-generator"}]
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

Full reference: [docs/elixir_api.md](docs/elixir_api.md), or `mix docs`.

## Development

```sh
cargo test                 # Rust unit + doc tests
cargo clippy --workspace   # lints
mix test                   # Elixir tests (needs Elixir + Rust)
```

CI runs formatting, clippy and both test suites on every push. Releases
are cut by pushing a tag: `v1.2.3` publishes a release with binaries for
Linux, macOS and Windows attached; a pre-release tag like `v1.2.3-rc1`
publishes a GitHub pre-release.

## License

Apache-2.0
