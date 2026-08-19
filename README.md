# Terrain generator

[![Latest Github release](https://img.shields.io/github/release/fabio-t/terrain-generator.svg)](https://github.com/fabio-t/terrain-generator/releases/latest)

Minimal terrain generator based on OpenSimplexNoise. It supports island shapes, allows river creation and
makes it simple to configure the octaved (or "fractal") noise generation for heightmaps, moisture and so on.

It ships with a complete example (just run it and inspect the images generated, then play around with parameters).

When and if it will become more complex, I'll write better documentation. Right now, you are going to play around
with the parameters yourself.

Feel free to write an Issue if you have questions or need help.

## Rust version

The generator has been ported to Rust (the original Java implementation is
still available under `src/main/java`). The Rust port produces bit-identical
images to the Java version for the same seeds: the OpenSimplex permutation
setup, the fractal noise accumulation and the river placement (which uses a
`java.util.Random`-compatible generator) all match the original exactly.

Build and run the example with:

```sh
cargo run --release
```

This writes `map0.png` .. `map9.png` to the current directory, one island
per seed. The library itself lives in `src/` (`heightmap`, `noise`,
`utils` modules) and can be used as a crate dependency.

## Install (Java version)

1. First add Jitpack as a repository inside your pom.xml

```xml
<repositories>
  <repository>
    <id>jitpack.io</id>
    <url>https://jitpack.io</url>
  </repository>
</repositories>
```

2. Then add `terrain-generator` as a dependency (make sure to change `version` to one of the
[releases](https://github.com/fabio-t/terrain-generator/releases)):

```xml
<dependency>
    <groupId>com.github.fabio-t</groupId>
    <artifactId>terrain-generator</artifactId>
    <version>version</version>
</dependency>
```
