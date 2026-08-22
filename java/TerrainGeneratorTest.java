import com.github.fabioticconi.tergen.Layer;
import com.github.fabioticconi.tergen.Terrain;
import com.github.fabioticconi.tergen.TerrainConfig;
import com.github.fabioticconi.tergen.TerrainException;
import com.github.fabioticconi.tergen.TerrainGenerator;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/**
 * End-to-end checks for the Java bindings, written as a plain class so the
 * binding stays dependency-free: {@code java TerrainGeneratorTest}.
 *
 * <p>Run from the repository root after building the native library:
 *
 * <pre>{@code
 * cargo build --release -p terrain_generator_ffi
 * javac -d java/out $(find java -name '*.java')
 * java --enable-native-access=ALL-UNNAMED \
 *      -Dtergen.library=target/release/libterrain_generator_ffi.so \
 *      -cp java/out TerrainGeneratorTest
 * }</pre>
 */
public final class TerrainGeneratorTest {

    private static int checks = 0;

    public static void main(String[] args) throws Exception {
        defaultsComeFromTheNativeLibrary();
        generatesTerrainOfTheRequestedSize();
        layersHaveSaneValues();
        matchesTheRustGoldenChecksum();
        generationIsDeterministic();
        archipelagoDiffersFromSingleIsland();
        heightmapOnlyModeSkipsHydrology();
        invalidParametersRaiseWithTheOffendingParameter();
        imageOnlyLayerIsRejectedAsData();
        useAfterCloseIsRejected();
        writesPngLayers();

        System.out.println("OK: " + checks + " checks passed");
    }

    // ---- checks ---------------------------------------------------------

    private static void defaultsComeFromTheNativeLibrary() {
        TerrainConfig config = TerrainConfig.defaults();
        try (Terrain terrain = TerrainGenerator.generate(config)) {
            // the native defaults are 512x512 with sea level 0.08
            check(terrain.width() == 512, "default width is 512");
            check(terrain.height() == 512, "default height is 512");
            check(Math.abs(terrain.seaLevel() - 0.08f) < 1e-6, "default sea level is 0.08");
        }
    }

    private static void generatesTerrainOfTheRequestedSize() {
        try (Terrain terrain = TerrainGenerator.generate(
                TerrainConfig.defaults().size(96, 64).seed(3))) {
            check(terrain.width() == 96 && terrain.height() == 64, "requested size honoured");
            check(terrain.cellCount() == 96 * 64, "cell count matches");
            check(terrain.floats(Layer.HEIGHT).length == 96 * 64, "heightmap length matches");
            check(terrain.mask(Layer.OCEAN).length == 96 * 64, "ocean mask length matches");
        }
    }

    private static void layersHaveSaneValues() {
        try (Terrain terrain = TerrainGenerator.generate(
                TerrainConfig.defaults().size(128, 128).seed(1))) {
            for (float h : terrain.floats(Layer.HEIGHT)) {
                check(h >= 0.0f && h <= 1.0f, "elevation within [0, 1]");
            }
            for (float a : terrain.floats(Layer.FLOW_ACCUMULATION)) {
                check(a >= 1.0f, "every cell drains at least itself");
            }
            byte[] ocean = terrain.mask(Layer.OCEAN);
            byte[] lakes = terrain.mask(Layer.LAKES);
            for (int i = 0; i < ocean.length; i++) {
                check(ocean[i] == 0 || ocean[i] == 1, "ocean mask is 0 or 1");
                check(!(ocean[i] == 1 && lakes[i] == 1), "a cell is never both ocean and lake");
            }
            check(terrain.isOcean(0, 0), "map corner is sea");
        }
    }

    /**
     * The heightmap must be byte-identical to what Rust produces. This
     * checksum is the same one pinned in tests/determinism.rs, so if the
     * FFI ever hands back the wrong buffer, a stale one, or the wrong
     * layout, this fails.
     */
    private static void matchesTheRustGoldenChecksum() {
        final long expected = 0x6f375144bc69ba10L;
        try (Terrain terrain = TerrainGenerator.generate(
                TerrainConfig.defaults().size(128, 128).seed(1))) {
            long got = fnv1a(terrain.floats(Layer.HEIGHT));
            check(got == expected,
                    String.format("heightmap matches the Rust golden checksum (expected %#x, got %#x)",
                            expected, got));
        }
    }

    private static void generationIsDeterministic() {
        TerrainConfig config = TerrainConfig.defaults().size(64, 64).seed(11).archipelago(3);
        try (Terrain a = TerrainGenerator.generate(config);
                Terrain b = TerrainGenerator.generate(config)) {
            check(Arrays.equals(a.floats(Layer.HEIGHT), b.floats(Layer.HEIGHT)),
                    "same config gives the same heightmap");
            check(Arrays.equals(a.mask(Layer.RIVERS), b.mask(Layer.RIVERS)),
                    "same config gives the same rivers");
        }
    }

    private static void archipelagoDiffersFromSingleIsland() {
        try (Terrain single = TerrainGenerator.generate(TerrainConfig.defaults().size(96, 96).seed(5));
                Terrain many = TerrainGenerator.generate(
                        TerrainConfig.defaults().size(96, 96).seed(5).archipelago(4))) {
            check(!Arrays.equals(single.floats(Layer.HEIGHT), many.floats(Layer.HEIGHT)),
                    "archipelago mode changes the terrain");
        }
    }

    private static void heightmapOnlyModeSkipsHydrology() {
        TerrainConfig base = TerrainConfig.defaults().size(64, 64).seed(7);
        try (Terrain full = TerrainGenerator.generate(base);
                Terrain bare = TerrainGenerator.generate(
                        TerrainConfig.defaults().size(64, 64).seed(7).noRivers())) {
            check(Arrays.equals(full.floats(Layer.HEIGHT), bare.floats(Layer.HEIGHT)),
                    "hydrology never feeds back into elevation");
            for (float f : bare.floats(Layer.FLOW)) {
                check(f == 0.0f, "flow is not computed when rivers are off");
            }
            boolean anyOcean = false;
            for (byte o : bare.mask(Layer.OCEAN)) {
                anyOcean |= o == 1;
            }
            check(anyOcean, "ocean mask is still available without hydrology");
        }
    }

    private static void invalidParametersRaiseWithTheOffendingParameter() {
        try {
            TerrainGenerator.generate(TerrainConfig.defaults().size(64, 64).seaLevel(5.0f));
            check(false, "invalid sea level should have thrown");
        } catch (TerrainException e) {
            check(e.getMessage().contains("sea level"),
                    "error names the offending parameter, got: " + e.getMessage());
        }
    }

    private static void imageOnlyLayerIsRejectedAsData() {
        try (Terrain terrain = TerrainGenerator.generate(TerrainConfig.defaults().size(32, 32))) {
            try {
                terrain.floats(Layer.SHADED);
                check(false, "SHADED should not be readable as data");
            } catch (IllegalArgumentException expected) {
                check(true, "image-only layer rejected as data");
            }
        }
    }

    private static void useAfterCloseIsRejected() {
        Terrain terrain = TerrainGenerator.generate(TerrainConfig.defaults().size(32, 32));
        terrain.close();
        terrain.close(); // idempotent
        try {
            terrain.floats(Layer.HEIGHT);
            check(false, "reading a closed terrain should have thrown");
        } catch (IllegalStateException expected) {
            check(true, "use after close is rejected instead of reading freed memory");
        }
    }

    private static void writesPngLayers() throws Exception {
        Path dir = Files.createTempDirectory("tergen-java");
        try (Terrain terrain = TerrainGenerator.generate(TerrainConfig.defaults().size(64, 64).seed(2))) {
            for (Layer layer : new Layer[] {Layer.SHADED, Layer.HEIGHT, Layer.OCEAN}) {
                Path path = dir.resolve(layer + ".png");
                terrain.savePng(layer, path);
                check(Files.size(path) > 0, layer + " png was written");
                byte[] magic = Arrays.copyOf(Files.readAllBytes(path), 4);
                check(magic[0] == (byte) 0x89 && magic[1] == 'P' && magic[2] == 'N' && magic[3] == 'G',
                        layer + " png has a PNG header");
            }
        }
    }

    // ---- helpers --------------------------------------------------------

    /** FNV-1a over the little-endian bytes of each float, as Rust does. */
    private static long fnv1a(float[] values) {
        long hash = 0xcbf29ce484222325L;
        for (float v : values) {
            int bits = Float.floatToRawIntBits(v);
            for (int shift = 0; shift < 32; shift += 8) {
                hash ^= (bits >>> shift) & 0xFF;
                hash *= 0x100000001b3L;
            }
        }
        return hash;
    }

    private static void check(boolean condition, String what) {
        checks++;
        if (!condition) {
            throw new AssertionError("FAILED: " + what);
        }
    }
}
