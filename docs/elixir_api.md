# Elixir API

The repository doubles as a Mix package: the `TerrainGenerator` Elixir
module wraps the Rust core through a [Rustler](https://hexdocs.pm/rustler)
NIF located in `native/terrain_generator_nif`. Generation runs on a dirty
CPU scheduler, so multi-second maps never block the BEAM.

## Installation

Add the git dependency to your `mix.exs`:

```elixir
def deps do
  [
    {:terrain_generator, github: "fabio-t/terrain-generator"}
  ]
end
```

**Requirements:** a Rust toolchain (`cargo`) must be available at compile
time — Rustler builds the NIF as part of `mix compile`. Rust 1.91+ (required
by Rustler 0.38), Erlang/OTP 24+ and Elixir ~> 1.15.

The CLI and the Rust library itself only need Rust 1.85; the higher floor
applies to the NIF crate.

## Quick start

```elixir
# generate and inspect
{:ok, terrain} = TerrainGenerator.generate(width: 512, height: 512, seed: 42)

TerrainGenerator.Terrain.height_at(terrain, 100, 100)
#=> 0.53

TerrainGenerator.Terrain.river?(terrain, 100, 100)
#=> false

# or render straight to a PNG without keeping the data
:ok = TerrainGenerator.render_png("map.png", seed: 42)
:ok = TerrainGenerator.render_png("height.png", [seed: 42], :greyscale)
```

## `TerrainGenerator.generate(opts \\ [])`

Runs the full pipeline (fractal noise → island mask → hydraulic erosion →
flow routing) and returns `{:ok, %TerrainGenerator.Terrain{}}`.
Generation is fully deterministic for a given seed and option set.

Raises `ArgumentError` on unknown option keys.

### Options

| Key | Default | Description |
|-----|---------|-------------|
| `:width`, `:height` | `512`, `512` | Map size in cells. |
| `:seed` | `0` | Generation seed. |
| `:sea_level` | `0.08` | Sea level in `[0, 1)`. |
| `:octaves` | `8` | Noise octaves. |
| `:frequency` | `2.5` | Number of large features across the map (resolution-independent). |
| `:lacunarity` | `2.0` | Frequency multiplier between octaves. |
| `:persistence` | `0.5` | Amplitude multiplier between octaves. |
| `:island` | `1.0` | Island diameter as a fraction of the shorter map dimension, `true` for the default, or `false` to let terrain reach the borders. |
| `:coast_noise` | `1.0` | Coastline perturbation strength; `0` gives a round island. |
| `:erosion` | `true` | `true` (one droplet per cell), `false` (skip erosion), or a positive integer droplet count. |
| `:rivers` | `0.01` | Approximate fraction of land covered by rivers, `true` for the default, or `false` to disable rivers and lakes. |

Every option is validated on the Elixir side before any native code runs;
unknown keys and out-of-range values raise `ArgumentError` with a
descriptive message.

## `TerrainGenerator.render_png(path, opts \\ [], style \\ :shaded)`

Generates a terrain with the same options and writes a PNG to `path`.
`style` is `:shaded` (biome colours with hillshading) or `:greyscale`
(plain heightmap). Returns `:ok`, or `{:error, reason}` with a
human-readable reason string if the file cannot be written.

## `%TerrainGenerator.Terrain{}`

| Field | Type | Description |
|-------|------|-------------|
| `:width`, `:height` | `pos_integer` | Map dimensions. |
| `:sea_level` | `float` | Sea level used for generation. |
| `:heightmap` | `binary` | One 32-bit little-endian float per cell, elevation in `[0, 1]`. |
| `:flow` | `binary` | One 32-bit little-endian float per cell, log-normalised flow accumulation in `[0, 1]` (a useful moisture proxy). |
| `:rivers` | `binary` | One byte per cell, `1` where a river runs. |
| `:lakes` | `binary` | One byte per cell, `1` where a lake holds standing water. |

All buffers are row-major, indexed as `y * width + x`. They are plain BEAM
binaries: cheap to pass between processes (stored off-heap and
reference-counted), easy to send over a socket, or to upload wholesale to
a GPU/scenegraph on the client.

### Accessors

```elixir
alias TerrainGenerator.Terrain

Terrain.height_at(terrain, x, y)   # float in [0, 1]
Terrain.flow_at(terrain, x, y)     # float in [0, 1]
Terrain.river?(terrain, x, y)      # boolean
Terrain.lake?(terrain, x, y)       # boolean
Terrain.water?(terrain, x, y)      # sea, lake or river
Terrain.heightmap_list(terrain)    # [float], row by row (small maps only)
```

### Working with the binaries directly

Decode a whole buffer with a binary comprehension:

```elixir
elevations = for <<v::float-32-little <- terrain.heightmap>>, do: v
```

Random access without decoding everything:

```elixir
<<v::float-32-little>> =
  binary_part(terrain.heightmap, (y * terrain.width + x) * 4, 4)
```

Count river cells:

```elixir
terrain.rivers
|> :binary.matches(<<1>>)
|> length()
```

## Concurrency

The NIFs are scheduled on **dirty CPU schedulers**: a 1024×1024 map takes
a couple of seconds of CPU, and the BEAM stays fully responsive while it
runs. Generating many maps concurrently from `Task.async_stream/3` works
as expected — each call occupies one dirty scheduler.

## Module docs

`TerrainGenerator`, `TerrainGenerator.Terrain` and their functions carry
full `@doc`/`@moduledoc` documentation; run `mix docs` (ExDoc is a dev
dependency) or use `h TerrainGenerator` in IEx.
