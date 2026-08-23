#!/usr/bin/env bash
# Builds the native library, packs it into a jar with the Java bindings,
# and runs the binding tests twice: once against the freshly built library
# and once against the jar alone. Requires a Rust toolchain and JDK 22+.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Must agree with TerrainGenerator.platform(), which is what looks the
# bundled library up inside the jar.
case "$(uname -s)" in
    Darwin) os=macos;   lib="libterrain_generator_ffi.dylib"; sep=":" ;;
    MINGW*|MSYS*|CYGWIN*) os=windows; lib="terrain_generator_ffi.dll"; sep=";" ;;
    *)      os=linux;   lib="libterrain_generator_ffi.so"; sep=":" ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) arch="$(uname -m)" ;;
esac
platform="$os-$arch"

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"

echo "==> building the native library"
cargo build --release -p terrain_generator_ffi

echo "==> packaging the jar for $platform"
rm -rf java/out
mkdir -p "java/out/natives/$platform"
cp "target/release/$lib" "java/out/natives/$platform/"
bash java/package-jar.sh "$version" java/out/natives java/out

jar="java/out/terrain-generator-$version.jar"

echo "==> compiling the binding tests"
mkdir -p java/out/test
javac --release 22 -cp "$jar" -d java/out/test java/TerrainGeneratorTest.java

echo "==> running the binding tests against target/release/$lib"
java --enable-native-access=ALL-UNNAMED \
     -Dtergen.library="target/release/$lib" \
     -cp "$jar$sep"java/out/test TerrainGeneratorTest

# The point of this second run: no -Dtergen.library, and the library is
# not on the search path, so the only way it can work is the loader
# extracting the native out of the jar. That is the path every consumer of
# a released jar takes, and it would otherwise ship untested.
echo "==> running the binding tests against the jar alone"
java --enable-native-access=ALL-UNNAMED \
     -cp "$jar$sep"java/out/test TerrainGeneratorTest
