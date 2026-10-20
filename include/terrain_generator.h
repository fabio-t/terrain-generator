/*
 * C ABI for terrain-generator.
 *
 * Implemented by the `terrain_generator_ffi` crate. Any language with an
 * FFI can use it; the Java bindings in `java/` bind these symbols through
 * the Foreign Function & Memory API, and this header also feeds `jextract`
 * if you prefer generated bindings.
 *
 * Conventions:
 *   - A terrain is an opaque handle owned by the library. Release it with
 *     tg_terrain_free(); every other call borrows it.
 *   - Fallible functions return TgStatus. On failure, tg_last_error()
 *     yields a human-readable message for the calling thread.
 *   - Layer accessors return a pointer into the terrain's own buffers:
 *     read-only, and valid until the terrain is freed.
 *   - No function unwinds; a panic inside the generator is reported as
 *     TG_INTERNAL_ERROR.
 */
#ifndef TERRAIN_GENERATOR_H
#define TERRAIN_GENERATOR_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int TgStatus;

#define TG_OK              0
#define TG_INVALID_PARAMS  1
#define TG_NULL_POINTER    2
#define TG_BAD_LAYER       3
#define TG_IO_ERROR        4
#define TG_BAD_UTF8        5
#define TG_INTERNAL_ERROR  6

/* Layer ids. HEIGHT/FLOW/FLOW_ACCUMULATION are float layers;
 * RIVERS/LAKES/OCEAN are 0/1 masks; SHADED is image-only. */
#define TG_LAYER_HEIGHT             0u
#define TG_LAYER_FLOW               1u
#define TG_LAYER_FLOW_ACCUMULATION  2u
#define TG_LAYER_RIVERS             3u
#define TG_LAYER_LAKES              4u
#define TG_LAYER_OCEAN              5u
#define TG_LAYER_SHADED             6u

typedef struct TgTerrain TgTerrain;

/*
 * Generation parameters. Initialise with tg_config_default(), then
 * override what you need — fields are appended over time.
 *
 * Switch-like fields:
 *   island_size <= 0              : no single-island mask
 *   archipelago_islands > 0       : archipelago mode instead
 *   droplets < 0                  : one droplet per cell; 0 disables erosion
 *   breach_max_lake_fraction <= 0 : no basin breaching
 *   river_min_upstream_cells > 0  : absolute river threshold, overrides
 *                                   river_density
 *   river_density <= 0            : no rivers (unless the above is set)
 */
typedef struct TgConfig {
    /* width * height must be at most 4294967295 (2^32 - 1) cells */
    uint64_t width;
    uint64_t height;
    uint32_t seed;
    float    sea_level;

    uint64_t octaves;
    double   frequency;
    double   lacunarity;
    double   persistence;

    float    island_size;
    float    coast_noise;

    uint64_t archipelago_islands;
    float    islands_size;
    float    islands_variation;
    float    islands_spread;
    float    islands_spacing;

    int64_t  droplets;
    float    breach_max_lake_fraction;

    float    river_density;
    float    river_min_upstream_cells;
} TgConfig;

/* Interface version; bumped on any breaking ABI change. */
uint32_t tg_abi_version(void);

/* sizeof(TgConfig) as the library sees it. Bindings that mirror the
 * struct should assert this at startup. */
uint64_t tg_config_size(void);

/* Fills *out with the default configuration. */
TgStatus tg_config_default(TgConfig *out);

/* Copies the calling thread's last error into buf as a NUL-terminated
 * string; returns the bytes needed including the terminator. Pass a NULL
 * buf (or len 0) to query the size. */
uint64_t tg_last_error(char *buf, uint64_t len);

/* Generates a terrain; on success writes an owned handle to *out. */
TgStatus tg_generate(const TgConfig *config, TgTerrain **out);

/* Releases a terrain. Passing NULL is a no-op. */
void tg_terrain_free(TgTerrain *terrain);

uint64_t tg_terrain_width(const TgTerrain *terrain);
uint64_t tg_terrain_height(const TgTerrain *terrain);
float    tg_terrain_sea_level(const TgTerrain *terrain);

/* Borrow a float layer (HEIGHT, FLOW, FLOW_ACCUMULATION). Writes the cell
 * count to *out_len. Returns NULL on error. */
const float *tg_terrain_layer_f32(const TgTerrain *terrain, uint32_t layer, uint64_t *out_len);

/* Borrow a mask layer (RIVERS, LAKES, OCEAN), values 0 or 1. */
const uint8_t *tg_terrain_layer_u8(const TgTerrain *terrain, uint32_t layer, uint64_t *out_len);

/* Writes one layer to a UTF-8 path as a PNG. */
TgStatus tg_terrain_save_png(const TgTerrain *terrain, uint32_t layer, const char *path);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* TERRAIN_GENERATOR_H */
