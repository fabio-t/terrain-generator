#!/usr/bin/env bash
# Packs the Java bindings and one or more prebuilt native libraries into a
# single self-contained jar, so consumers need no Rust toolchain.
#
#   java/package-jar.sh <version> <natives-dir> [output-dir]
#
# <natives-dir> holds one directory per platform, named the way
# TerrainGenerator.platform() names it, each containing that platform's
# shared library:
#
#   natives/linux-x86_64/libterrain_generator_ffi.so
#   natives/macos-aarch64/libterrain_generator_ffi.dylib
#   natives/windows-x86_64/terrain_generator_ffi.dll
#
# Those land at /natives/<platform>/ inside the jar, where the binding's
# loader looks for them.
set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "usage: $0 <version> <natives-dir> [output-dir]" >&2
    exit 2
fi

version="$1"

# Resolve the caller's paths before moving to the repository root, so a
# relative argument means what it meant where it was typed.
if [ ! -d "$2" ]; then
    echo "error: no natives directory at '$2'" >&2
    exit 1
fi
natives="$(cd "$2" && pwd)"
outdir_arg="${3:-}"
if [ -n "$outdir_arg" ]; then
    mkdir -p "$outdir_arg"
    outdir="$(cd "$outdir_arg" && pwd)"
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

outdir="${outdir:-$root/java/out}"

# A jar with no native in it would fail only at run time, on the consumer's
# machine — refuse to build one.
if ! find "$natives" -type f \( -name '*.so' -o -name '*.dylib' -o -name '*.dll' \) | grep -q .; then
    echo "error: '$natives' contains no shared libraries" >&2
    exit 1
fi

classes="$outdir/classes"
staging="$outdir/jar"
jar="$outdir/terrain-generator-$version.jar"

rm -rf "$classes" "$staging"
mkdir -p "$classes" "$staging"

# --release 22 rather than the running JDK: the FFM API is final in 22, and
# compiling against a newer JDK would emit class files that a 22 or 23 JVM
# refuses to load, quietly narrowing the audience for the jar.
echo "==> compiling the bindings (--release 22)"
find java/com -name '*.java' -print0 | xargs -0 javac --release 22 -d "$classes"

echo "==> staging natives"
mkdir -p "$staging/natives"
cp -R "$natives"/. "$staging/natives/"
(cd "$staging/natives" && ls -1) | sed 's/^/    /'

cat > "$outdir/MANIFEST.MF" <<MANIFEST
Implementation-Title: terrain-generator
Implementation-Version: $version
Implementation-Vendor: Fabio Ticconi
MANIFEST

echo "==> writing $jar"
rm -f "$jar"
jar --create --file "$jar" --manifest "$outdir/MANIFEST.MF" \
    -C "$classes" . \
    -C "$staging" natives

echo "$jar"
