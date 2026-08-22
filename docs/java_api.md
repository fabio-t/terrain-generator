# Java API

The generator is usable from Java through a plain **C ABI** plus a thin
binding written with the [Foreign Function & Memory API][ffm] (Project
Panama). There is no JNI shim to build and no Java-specific code inside
the Rust library.

That is worth a sentence of justification, since Java interop used to mean
JNI. JNI requires a hand-written C bridge compiled against `jni.h`, and
would put Java-shaped code inside this repository's Rust. The FFM API
instead calls an ordinary C-ABI shared library directly, so the same
artefact that serves Java also serves C, C++, C#, Python (`ctypes`), Go
and Zig — the Java layer is then pure Java with zero dependencies.

## Requirements

* **JDK 22 or newer.** The FFM API was finalised in Java 22; in 21 it is a
  preview API and these sources will not compile.
* A Rust toolchain, to build the native library.

## Building

```sh
./java/build.sh
```

That builds `terrain_generator_ffi`, compiles the bindings into
`java/out/`, and runs the binding tests. Manually, it is:

```sh
cargo build --release -p terrain_generator_ffi
javac -d java/out $(find java -name '*.java')
java --enable-native-access=ALL-UNNAMED \
     -Dtergen.library=target/release/libterrain_generator_ffi.so \
     -cp java/out TerrainGeneratorTest
```

The sources are plain Java with no dependencies, so they drop into Maven,
Gradle or a plain classpath unchanged — copy
`java/com/github/fabioticconi/tergen/` into your source tree, or compile it
into a jar.

### Locating the native library

`TerrainGenerator` looks for the shared library in this order:

1. the path given by the `tergen.library` system property;
2. `libterrain_generator_ffi.so` (`.dylib` on macOS, `.dll` on Windows) on
   the default library search path.

Because FFM downcalls are a restricted operation, pass
`--enable-native-access=ALL-UNNAMED` (or the equivalent for your module) to
avoid a runtime warning.

## Quick start

```java
import com.github.fabioticconi.tergen.*;
import java.nio.file.Path;

TerrainConfig config = TerrainConfig.defaults()
        .size(512, 512)
        .seed(42);

try (Terrain terrain = TerrainGenerator.generate(config)) {
    float elevation = terrain.heightAt(100, 100);   // [0, 1]
    boolean river   = terrain.isRiver(100, 100);
    boolean sea     = terrain.isOcean(100, 100);

    float[] heightmap = terrain.floats(Layer.HEIGHT); // row-major copy
    terrain.savePng(Layer.SHADED, Path.of("map.png"));
}
```

`Terrain` owns native memory, so it is `AutoCloseable` — use
try-with-resources. Reading a closed terrain throws
`IllegalStateException` rather than touching freed memory.

## `TerrainConfig`

Start from `TerrainConfig.defaults()` — which reads the defaults *from the
native library*, so they cannot drift — and chain setters:

| Method | Meaning |
|--------|---------|
| `size(width, height)` | Map dimensions in cells. Default `512 × 512`. |
| `seed(int)` | Generation seed. |
| `seaLevel(float)` | Sea level in `[0, 1)`. Default `0.08`. |
| `noise(octaves, frequency, lacunarity, persistence)` | Fractal noise settings. |
| `island(size)` / `noIsland()` | Single-island diameter as a fraction of the shorter map side, or no mask at all. |
| `coastNoise(float)` | Coastline perturbation; `0` gives a round island. |
| `archipelago(n)` / `archipelagoShape(size, variation, spread, spacing)` | Place `n` islands instead of one. |
| `droplets(n)` / `noErosion()` | Erosion droplet count; negative means one per cell. |
| `breachBasins(fraction)` | Carve outlets from closed basins bigger than this fraction of the map. |
| `riverDensity(float)` | Approximate fraction of land covered by rivers. |
| `riverMinUpstreamCells(float)` | Absolute river threshold in cells of catchment; overrides density. |
| `noRivers()` | Skip flow routing entirely. |

Invalid values are not rejected in Java — the native generator validates
them and `generate` throws `TerrainException` naming the offending
parameter:

```java
try {
    TerrainGenerator.generate(TerrainConfig.defaults().seaLevel(5.0f));
} catch (TerrainException e) {
    e.getMessage(); // "sea level must be in range [0,1)"
}
```

## `Terrain` and layers

| Layer | Element type | Meaning |
|-------|--------------|---------|
| `Layer.HEIGHT` | float | Elevation in `[0, 1]`. |
| `Layer.FLOW` | float | Log-normalised flow accumulation; a moisture proxy. |
| `Layer.FLOW_ACCUMULATION` | float | Raw upstream catchment in cells. Size channels from this. |
| `Layer.RIVERS` | mask | `1` where a river runs. |
| `Layer.LAKES` | mask | `1` where a lake stands. |
| `Layer.OCEAN` | mask | `1` where the cell is sea — at or below sea level **and** connected to the map border. |
| `Layer.SHADED` | image only | The relief render; `savePng` only. |

Accessors: `width()`, `height()`, `cellCount()`, `seaLevel()`,
`heightAt(x, y)`, `flowAccumulationAt(x, y)`, `isRiver`, `isLake`,
`isOcean`, `isWater`, `floats(Layer)`, `mask(Layer)`, `segment(Layer)`,
`savePng(Layer, Path)`.

`floats` and `mask` copy into Java arrays. `segment` returns a read-only
`MemorySegment` view of the native buffer instead — no copy, valid until
the terrain is closed — which is what you want when handing a 1024×1024
heightmap (4 MB) straight to a GPU buffer.

### Ocean is connectivity, not elevation

`OCEAN` comes from a flood fill starting at the map border. A closed basin
whose floor lies below sea level is therefore a **lake**, not ocean. If you
classify water for your own rules — salinity, drinkability, navigability —
build on `isOcean`/`isLake` rather than comparing `heightAt` to
`seaLevel()`, which would silently call such a basin sea.

## Determinism

The same config produces the same terrain, and it is the *same* terrain
Rust and Elixir produce: the binding tests assert that a 128×128 map at
seed 1 hashes to the checksum pinned in the Rust test suite, so a mistake
in the FFI layer cannot pass unnoticed.

If you persist worlds as seeds, pin an exact revision of this project —
output depends on the `noise` crate and on tuning inside the generator.
One layer is exempt from cross-platform bit-exactness: `FLOW` is
log-normalised with a transcendental whose last-bit rounding can vary by
platform. Every other layer uses only exact operations.

## The C ABI directly

If you would rather generate bindings with `jextract`, or call the library
from C/C++, the interface is declared in
[`include/terrain_generator.h`](../include/terrain_generator.h) and
implemented by `native/terrain_generator_ffi`. Key points:

* A terrain is an opaque `TgTerrain *`; free it with `tg_terrain_free`.
* Fallible calls return `TgStatus`; `tg_last_error` gives the message for
  the calling thread.
* Layer accessors borrow the terrain's buffers — read-only, valid until
  the handle is freed.
* Nothing unwinds across the boundary: a panic inside the generator is
  reported as `TG_INTERNAL_ERROR`.
* `tg_abi_version()` and `tg_config_size()` let a binding verify at
  startup that it was built against a matching library. The Java binding
  checks both and fails loudly on a mismatch, rather than silently
  misreading every field after a layout change.

[ffm]: https://docs.oracle.com/en/java/javase/22/core/foreign-function-and-memory-api.html
