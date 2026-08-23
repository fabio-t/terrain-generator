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
  preview API and these sources will not compile. The released jar is
  compiled with `--release 22`, so a Java 22 runtime can load it even
  though CI builds it on a newer JDK.
* A Rust toolchain **only if you build from source** — the released jar
  needs none.

## Using the released jar

Every release attaches `terrain-generator-<version>.jar`. It contains the
bindings *and* a prebuilt native library for each supported platform:

| Directory in the jar | Platform |
|----------------------|----------|
| `natives/linux-x86_64/libterrain_generator_ffi.so` | Linux x86-64, glibc 2.35 or newer |
| `natives/macos-aarch64/libterrain_generator_ffi.dylib` | macOS on Apple Silicon |
| `natives/macos-x86_64/libterrain_generator_ffi.dylib` | macOS on Intel |
| `natives/windows-x86_64/terrain_generator_ffi.dll` | Windows x86-64 |

Put it on the classpath and it works — no `cargo`, no `LD_LIBRARY_PATH`,
nothing to install:

```sh
java --enable-native-access=ALL-UNNAMED -cp terrain-generator-0.2.1.jar:. YourApp
```

This is the same trick sqlite-jdbc, LWJGL and netty-tcnative use, and it
has the same two caveats. The Linux library is linked against glibc, not
musl: an Alpine JVM needs a library built for it (build from source, or
point `tergen.library` at your own). And the extracted copy cannot be
deleted on Windows while the process holds it open, so a few hundred
kilobytes may linger in the system temporary directory until the OS
reclaims them.

### Locating the native library

`TerrainGenerator` looks for the shared library in this order:

1. the path given by the `tergen.library` system property;
2. `libterrain_generator_ffi.so` (`.dylib` on macOS, `.dll` on Windows) on
   the default library search path;
3. `/natives/<os>-<arch>/<library>` on the classpath, copied out to a
   private temporary directory and loaded from there.

The bundled copy is deliberately last, so a library you built yourself
still wins while you are working on the Rust side. `<os>` is `linux`,
`macos` or `windows`; `<arch>` is `x86_64` or `aarch64`, normalised from
`os.arch` (so `amd64` and `arm64` map onto the same directories).

If none of the three work, the failure names the platform it looked for
and carries both underlying errors as suppressed exceptions.

Because FFM downcalls are a restricted operation, pass
`--enable-native-access=ALL-UNNAMED` (or the equivalent for your module) to
avoid a runtime warning.

## Building from source

```sh
./java/build.sh
```

That builds `terrain_generator_ffi` for the host, packs
`java/out/terrain-generator-<version>.jar` with that one native inside it,
and runs the binding tests twice: once against the freshly built library,
and once against the jar with nothing on the library search path — which
is the only way to exercise the extraction path every jar consumer takes.

To pack a jar for several platforms at once, collect the libraries into
one directory named the way the loader expects and call the packaging
script directly:

```sh
bash java/package-jar.sh 0.2.1 path/to/natives
```

The sources are plain Java with no dependencies, so they also drop into
Maven, Gradle or a plain classpath unchanged — copy
`java/com/github/fabioticconi/tergen/` into your source tree if you would
rather vendor them than depend on the jar.

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
