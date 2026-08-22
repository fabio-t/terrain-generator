#!/usr/bin/env bash
# Builds the native library and the Java bindings, then runs the binding
# tests. Requires a Rust toolchain and JDK 22 or newer.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

case "$(uname -s)" in
    Darwin) lib="libterrain_generator_ffi.dylib" ;;
    MINGW*|MSYS*|CYGWIN*) lib="terrain_generator_ffi.dll" ;;
    *) lib="libterrain_generator_ffi.so" ;;
esac

echo "==> building the native library"
cargo build --release -p terrain_generator_ffi

echo "==> compiling the Java bindings"
rm -rf java/out
mkdir -p java/out
find java -name '*.java' -print0 | xargs -0 javac -d java/out

echo "==> running the binding tests"
java --enable-native-access=ALL-UNNAMED \
     -Dtergen.library="target/release/$lib" \
     -cp java/out TerrainGeneratorTest
