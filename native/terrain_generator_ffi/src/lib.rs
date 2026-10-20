//! A C ABI for `terrain-generator`.
//!
//! This crate exposes the generator through a plain C interface, so any
//! language with an FFI can drive it: Java (via the Foreign Function &
//! Memory API), C, C++, C#, Python, Go, Zig. There is no language-specific
//! dependency here — the Java bindings in `java/` are ordinary Java source
//! files that call these symbols.
//!
//! # Conventions
//!
//! * A terrain is an opaque handle ([`TgTerrain`]) owned by Rust. Free it
//!   with [`tg_terrain_free`]; every other call borrows it.
//! * Functions that can fail return a [`TgStatus`] code. On
//!   [`TG_INVALID_PARAMS`] a human-readable description of the offending
//!   parameter is available from [`tg_last_error`] on the same thread.
//! * Layer accessors return a pointer *into* the terrain's own buffers.
//!   They are valid until the terrain is freed and must not be written to.
//! * Every entry point catches panics, so a bug in the generator surfaces
//!   as [`TG_INTERNAL_ERROR`] rather than unwinding into foreign frames.
//!
//! # ABI stability
//!
//! [`tg_abi_version`] returns the version of this interface. It is bumped
//! whenever the layout of [`TgConfig`], the meaning of a layer id, or a
//! function signature changes. Callers should check it once at startup.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};

use terrain_generator::{
    ArchipelagoParams, BreachParams, ErosionParams, IslandParams, Layer, NoiseParams, RiverParams,
    Terrain,
};

/// Version of this C interface. Bumped on any breaking ABI change.
pub const TG_ABI_VERSION: u32 = 1;

/// Status code returned by fallible functions.
pub type TgStatus = c_int;

/// Success.
pub const TG_OK: TgStatus = 0;
/// A parameter was out of range; see [`tg_last_error`].
pub const TG_INVALID_PARAMS: TgStatus = 1;
/// A required pointer argument was null.
pub const TG_NULL_POINTER: TgStatus = 2;
/// The layer id does not exist, or has the wrong element type for the
/// accessor that was called.
pub const TG_BAD_LAYER: TgStatus = 3;
/// Writing the file failed; see [`tg_last_error`].
pub const TG_IO_ERROR: TgStatus = 4;
/// A path or string argument was not valid UTF-8.
pub const TG_BAD_UTF8: TgStatus = 5;
/// The generator panicked. This is a bug; see [`tg_last_error`].
pub const TG_INTERNAL_ERROR: TgStatus = 6;

/// Layer ids, shared by the accessors and by [`tg_terrain_save_png`].
pub const TG_LAYER_HEIGHT: u32 = 0;
/// Normalised flow, `f32`.
pub const TG_LAYER_FLOW: u32 = 1;
/// Raw upstream catchment in cells, `f32`.
pub const TG_LAYER_FLOW_ACCUMULATION: u32 = 2;
/// River mask, `u8`.
pub const TG_LAYER_RIVERS: u32 = 3;
/// Lake mask, `u8`.
pub const TG_LAYER_LAKES: u32 = 4;
/// Ocean mask, `u8`.
pub const TG_LAYER_OCEAN: u32 = 5;
/// Shaded relief. Valid only for [`tg_terrain_save_png`].
pub const TG_LAYER_SHADED: u32 = 6;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_error(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.into());
}

/// Runs `f`, converting a panic into [`TG_INTERNAL_ERROR`]. Unwinding into
/// foreign frames is undefined behaviour, so nothing may escape.
fn guard(f: impl FnOnce() -> TgStatus) -> TgStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => {
            set_error("the terrain generator panicked; this is a bug");
            TG_INTERNAL_ERROR
        }
    }
}

/// An opaque generated terrain. Free with [`tg_terrain_free`].
pub struct TgTerrain {
    inner: Terrain,
}

/// Generation parameters.
///
/// Fill this with [`tg_config_default`] first, then override what you
/// need: new fields are appended over time, and starting from the defaults
/// keeps callers source-compatible.
///
/// Several fields double as switches, so that the struct stays a plain
/// value type with no optionals:
///
/// * `island_size <= 0` disables single-island shaping.
/// * `archipelago_islands > 0` selects archipelago mode instead.
/// * `droplets < 0` means one droplet per cell; `0` disables erosion.
/// * `breach_max_lake_fraction <= 0` disables basin breaching.
/// * `river_density <= 0` disables rivers *unless*
///   `river_min_upstream_cells > 0`, which selects the absolute threshold.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TgConfig {
    pub width: u64,
    pub height: u64,
    pub seed: u32,
    pub sea_level: f32,

    pub octaves: u64,
    pub frequency: f64,
    pub lacunarity: f64,
    pub persistence: f64,

    pub island_size: f32,
    pub coast_noise: f32,

    pub archipelago_islands: u64,
    pub islands_size: f32,
    pub islands_variation: f32,
    pub islands_spread: f32,
    pub islands_spacing: f32,

    pub droplets: i64,
    pub breach_max_lake_fraction: f32,

    pub river_density: f32,
    pub river_min_upstream_cells: f32,
}

impl Default for TgConfig {
    fn default() -> Self {
        let island = IslandParams::default();
        let arch = ArchipelagoParams::default();
        let noise = NoiseParams::default();
        Self {
            width: 512,
            height: 512,
            seed: 0,
            sea_level: 0.08,
            octaves: noise.octaves as u64,
            frequency: noise.frequency,
            lacunarity: noise.lacunarity,
            persistence: noise.persistence,
            island_size: island.size,
            coast_noise: island.coast_noise,
            archipelago_islands: 0,
            islands_size: arch.size,
            islands_variation: arch.size_variation,
            islands_spread: arch.spread,
            islands_spacing: arch.min_spacing,
            droplets: -1,
            breach_max_lake_fraction: 0.0,
            river_density: 0.01,
            river_min_upstream_cells: 0.0,
        }
    }
}

/// Returns the ABI version of this library.
#[no_mangle]
pub extern "C" fn tg_abi_version() -> u32 {
    TG_ABI_VERSION
}

/// Size of [`TgConfig`] in bytes.
///
/// Bindings that mirror the struct layout should assert this against their
/// own view of it at startup: a mismatch means the binding was built
/// against a different version of this library, and every field after the
/// divergence would be silently misread.
#[no_mangle]
pub extern "C" fn tg_config_size() -> u64 {
    std::mem::size_of::<TgConfig>() as u64
}

/// Writes the default configuration into `out`.
///
/// # Safety
/// `out` must be null or point to writable storage for one [`TgConfig`].
#[no_mangle]
pub unsafe extern "C" fn tg_config_default(out: *mut TgConfig) -> TgStatus {
    guard(|| {
        if out.is_null() {
            set_error("tg_config_default: out is null");
            return TG_NULL_POINTER;
        }
        // SAFETY: checked non-null; the caller owns writable TgConfig storage
        unsafe { *out = TgConfig::default() };
        TG_OK
    })
}

/// Copies the last error message for this thread into `buf` as a
/// NUL-terminated string, and returns the number of bytes that *would* be
/// needed including the terminator.
///
/// Passing a null `buf` or a `len` of 0 queries the required size without
/// writing anything.
///
/// # Safety
/// `buf` must be null, or point to at least `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn tg_last_error(buf: *mut c_char, len: u64) -> u64 {
    LAST_ERROR.with(|e| {
        let msg = e.borrow();
        let bytes = msg.as_bytes();
        let needed = bytes.len() as u64 + 1;

        if buf.is_null() || len == 0 {
            return needed;
        }

        let copy = bytes.len().min(len as usize - 1);
        // SAFETY: buf is non-null and the caller promises `len` writable
        // bytes; `copy` leaves room for the terminator
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy);
            *buf.add(copy) = 0;
        }
        needed
    })
}

/// Generates a terrain. On success writes an owned handle to `out`, which
/// the caller must release with [`tg_terrain_free`].
///
/// # Safety
/// `config` must be null or point to a readable [`TgConfig`]; `out` must
/// be null or point to writable storage for one pointer.
#[no_mangle]
pub unsafe extern "C" fn tg_generate(
    config: *const TgConfig,
    out: *mut *mut TgTerrain,
) -> TgStatus {
    guard(|| {
        if config.is_null() || out.is_null() {
            set_error("tg_generate: config or out is null");
            return TG_NULL_POINTER;
        }
        // SAFETY: checked non-null; TgConfig is a plain value type
        let cfg = unsafe { *config };

        let mut builder = Terrain::builder()
            .size(cfg.width as usize, cfg.height as usize)
            .seed(cfg.seed)
            .sea_level(cfg.sea_level)
            .noise(NoiseParams {
                octaves: cfg.octaves as usize,
                frequency: cfg.frequency,
                lacunarity: cfg.lacunarity,
                persistence: cfg.persistence,
            });

        builder = if cfg.archipelago_islands > 0 {
            builder.archipelago(ArchipelagoParams {
                islands: cfg.archipelago_islands as usize,
                size: cfg.islands_size,
                size_variation: cfg.islands_variation,
                spread: cfg.islands_spread,
                min_spacing: cfg.islands_spacing,
                coast_noise: cfg.coast_noise,
            })
        } else if cfg.island_size > 0.0 {
            builder.island(IslandParams {
                size: cfg.island_size,
                coast_noise: cfg.coast_noise,
            })
        } else {
            builder.no_island()
        };

        builder = match cfg.droplets {
            d if d < 0 => builder.erosion(ErosionParams::default()),
            0 => builder.no_erosion(),
            d => builder.erosion(ErosionParams {
                droplets: Some(d as usize),
                ..Default::default()
            }),
        };

        if cfg.breach_max_lake_fraction > 0.0 {
            builder = builder.breach_basins(BreachParams {
                max_lake_fraction: cfg.breach_max_lake_fraction,
            });
        }

        builder = if cfg.river_min_upstream_cells > 0.0 {
            builder.rivers(RiverParams::min_upstream_cells(
                cfg.river_min_upstream_cells,
            ))
        } else if cfg.river_density > 0.0 {
            builder.rivers(RiverParams::density(cfg.river_density))
        } else {
            builder.no_rivers()
        };

        match builder.try_generate() {
            Ok(inner) => {
                let handle = Box::into_raw(Box::new(TgTerrain { inner }));
                // SAFETY: checked non-null above
                unsafe { *out = handle };
                TG_OK
            }
            Err(e) => {
                set_error(e.to_string());
                TG_INVALID_PARAMS
            }
        }
    })
}

/// Frees a terrain. Passing null is a no-op; passing the same handle twice
/// is undefined behaviour.
///
/// # Safety
/// `terrain` must come from [`tg_generate`] and must not be used after.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_free(terrain: *mut TgTerrain) {
    if terrain.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the caller guarantees this came from Box::into_raw
        drop(unsafe { Box::from_raw(terrain) });
    }));
}

/// Map width in cells, or 0 if `terrain` is null.
///
/// # Safety
/// `terrain` must be a live handle from [`tg_generate`], or null.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_width(terrain: *const TgTerrain) -> u64 {
    // SAFETY: caller guarantees liveness; null is handled
    match unsafe { terrain.as_ref() } {
        Some(t) => t.inner.width() as u64,
        None => 0,
    }
}

/// Map height in cells, or 0 if `terrain` is null.
///
/// # Safety
/// `terrain` must be a live handle from [`tg_generate`], or null.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_height(terrain: *const TgTerrain) -> u64 {
    // SAFETY: caller guarantees liveness; null is handled
    match unsafe { terrain.as_ref() } {
        Some(t) => t.inner.height() as u64,
        None => 0,
    }
}

/// Sea level the terrain was generated with, or a negative value if
/// `terrain` is null.
///
/// # Safety
/// `terrain` must be a live handle from [`tg_generate`], or null.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_sea_level(terrain: *const TgTerrain) -> f32 {
    // SAFETY: caller guarantees liveness; null is handled
    match unsafe { terrain.as_ref() } {
        Some(t) => t.inner.sea_level,
        None => -1.0,
    }
}

/// Borrows a `f32` layer: `TG_LAYER_HEIGHT`, `TG_LAYER_FLOW` or
/// `TG_LAYER_FLOW_ACCUMULATION`.
///
/// Writes `width * height` to `out_len` and returns a read-only pointer
/// into the terrain, valid until [`tg_terrain_free`]. Returns null on
/// error.
///
/// # Safety
/// `terrain` must be a live handle; `out_len` must be writable or null.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_layer_f32(
    terrain: *const TgTerrain,
    layer: u32,
    out_len: *mut u64,
) -> *const f32 {
    // SAFETY: caller guarantees liveness; null is handled
    let Some(t) = (unsafe { terrain.as_ref() }) else {
        set_error("tg_terrain_layer_f32: terrain is null");
        return std::ptr::null();
    };

    let grid = match layer {
        TG_LAYER_HEIGHT => &t.inner.heightmap,
        TG_LAYER_FLOW => &t.inner.flow,
        TG_LAYER_FLOW_ACCUMULATION => &t.inner.flow_accumulation,
        _ => {
            set_error("tg_terrain_layer_f32: layer id is not a float layer");
            return std::ptr::null();
        }
    };

    if !out_len.is_null() {
        // SAFETY: checked non-null
        unsafe { *out_len = grid.len() as u64 };
    }
    grid.as_slice().as_ptr()
}

/// Borrows a `u8` mask layer: `TG_LAYER_RIVERS`, `TG_LAYER_LAKES` or
/// `TG_LAYER_OCEAN`. Values are 0 or 1.
///
/// # Safety
/// `terrain` must be a live handle; `out_len` must be writable or null.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_layer_u8(
    terrain: *const TgTerrain,
    layer: u32,
    out_len: *mut u64,
) -> *const u8 {
    // SAFETY: caller guarantees liveness; null is handled
    let Some(t) = (unsafe { terrain.as_ref() }) else {
        set_error("tg_terrain_layer_u8: terrain is null");
        return std::ptr::null();
    };

    let grid = match layer {
        TG_LAYER_RIVERS => &t.inner.rivers,
        TG_LAYER_LAKES => &t.inner.lakes,
        TG_LAYER_OCEAN => &t.inner.ocean,
        _ => {
            set_error("tg_terrain_layer_u8: layer id is not a mask layer");
            return std::ptr::null();
        }
    };

    if !out_len.is_null() {
        // SAFETY: checked non-null
        unsafe { *out_len = grid.len() as u64 };
    }
    grid.as_slice().as_ptr()
}

/// Writes one layer to `path` as a PNG. `TG_LAYER_SHADED` writes the
/// shaded render; the data layers are written losslessly (elevation and
/// flow as 16-bit greyscale, masks as 8-bit).
///
/// # Safety
/// `terrain` must be a live handle and `path` a NUL-terminated UTF-8
/// string.
#[no_mangle]
pub unsafe extern "C" fn tg_terrain_save_png(
    terrain: *const TgTerrain,
    layer: u32,
    path: *const c_char,
) -> TgStatus {
    guard(|| {
        // SAFETY: caller guarantees liveness; null is handled
        let Some(t) = (unsafe { terrain.as_ref() }) else {
            set_error("tg_terrain_save_png: terrain is null");
            return TG_NULL_POINTER;
        };
        if path.is_null() {
            set_error("tg_terrain_save_png: path is null");
            return TG_NULL_POINTER;
        }

        // SAFETY: caller guarantees a NUL-terminated string
        let path = match unsafe { CStr::from_ptr(path) }.to_str() {
            Ok(p) => p,
            Err(_) => {
                set_error("tg_terrain_save_png: path is not valid UTF-8");
                return TG_BAD_UTF8;
            }
        };

        let layer = match layer {
            TG_LAYER_HEIGHT => Layer::Height,
            TG_LAYER_FLOW => Layer::Flow,
            TG_LAYER_RIVERS => Layer::Rivers,
            TG_LAYER_LAKES => Layer::Lakes,
            TG_LAYER_OCEAN => Layer::Ocean,
            TG_LAYER_SHADED => Layer::Shaded,
            _ => {
                set_error("tg_terrain_save_png: unknown layer id");
                return TG_BAD_LAYER;
            }
        };

        match t.inner.save_layer_png(layer, path) {
            Ok(()) => TG_OK,
            Err(e) => {
                set_error(e.to_string());
                TG_IO_ERROR
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate(cfg: &TgConfig) -> *mut TgTerrain {
        let mut handle = std::ptr::null_mut();
        // SAFETY: cfg is a live reference and handle is writable storage
        assert_eq!(unsafe { tg_generate(cfg, &mut handle) }, TG_OK);
        assert!(!handle.is_null());
        handle
    }

    fn small_config() -> TgConfig {
        let mut cfg = TgConfig::default();
        // SAFETY: cfg is writable storage for one TgConfig
        assert_eq!(unsafe { tg_config_default(&mut cfg) }, TG_OK);
        cfg.width = 64;
        cfg.height = 48;
        cfg.seed = 3;
        cfg
    }

    #[test]
    fn generate_and_read_layers() {
        let cfg = small_config();
        let handle = generate(&cfg);

        // SAFETY: handle is live for the rest of the test
        unsafe {
            assert_eq!(tg_terrain_width(handle), 64);
            assert_eq!(tg_terrain_height(handle), 48);
            assert!((tg_terrain_sea_level(handle) - cfg.sea_level).abs() < 1e-6);

            let mut len = 0u64;
            let ptr = tg_terrain_layer_f32(handle, TG_LAYER_HEIGHT, &mut len);
            assert!(!ptr.is_null());
            assert_eq!(len, 64 * 48);
            let heights = std::slice::from_raw_parts(ptr, len as usize);
            assert!(heights.iter().all(|h| (0.0..=1.0).contains(h)));

            let mut mask_len = 0u64;
            let mask = tg_terrain_layer_u8(handle, TG_LAYER_OCEAN, &mut mask_len);
            assert!(!mask.is_null());
            let ocean = std::slice::from_raw_parts(mask, mask_len as usize);
            assert!(ocean.iter().all(|v| *v <= 1));

            tg_terrain_free(handle);
        }
    }

    #[test]
    fn layer_type_mismatch_is_rejected() {
        let handle = generate(&small_config());
        // SAFETY: handle is live
        unsafe {
            assert!(tg_terrain_layer_f32(handle, TG_LAYER_OCEAN, std::ptr::null_mut()).is_null());
            assert!(tg_terrain_layer_u8(handle, TG_LAYER_HEIGHT, std::ptr::null_mut()).is_null());
            tg_terrain_free(handle);
        }
    }

    #[test]
    fn invalid_params_report_an_error_message() {
        let mut cfg = small_config();
        cfg.sea_level = 5.0;
        let mut handle = std::ptr::null_mut();
        // SAFETY: cfg and handle are live local storage
        assert_eq!(unsafe { tg_generate(&cfg, &mut handle) }, TG_INVALID_PARAMS);
        assert!(handle.is_null());

        // SAFETY: a null buffer queries the required size
        let needed = unsafe { tg_last_error(std::ptr::null_mut(), 0) };
        assert!(needed > 1);
        let mut buf = vec![0i8; needed as usize];
        // SAFETY: buf holds `needed` writable bytes
        unsafe { tg_last_error(buf.as_mut_ptr(), needed) };
        let msg = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy();
        assert!(msg.contains("sea level"), "unexpected message: {msg}");
    }

    #[test]
    fn oversized_maps_are_rejected_not_crashed() {
        // one past the cell limit, and a product that overflows usize
        for (width, height) in [(65536, 65536), (u64::MAX, 2)] {
            let mut cfg = small_config();
            cfg.width = width;
            cfg.height = height;
            let mut handle = std::ptr::null_mut();
            // SAFETY: cfg and handle are live local storage
            assert_eq!(unsafe { tg_generate(&cfg, &mut handle) }, TG_INVALID_PARAMS);
            assert!(handle.is_null());
        }
    }

    #[test]
    fn null_arguments_are_rejected_not_crashed() {
        // SAFETY: null is explicitly handled by both functions
        assert_eq!(
            unsafe { tg_config_default(std::ptr::null_mut()) },
            TG_NULL_POINTER
        );
        let mut handle = std::ptr::null_mut();
        // SAFETY: null config is explicitly handled
        assert_eq!(
            unsafe { tg_generate(std::ptr::null(), &mut handle) },
            TG_NULL_POINTER
        );
        // SAFETY: freeing null is documented as a no-op
        unsafe { tg_terrain_free(std::ptr::null_mut()) };
        // SAFETY: null handles return sentinel values
        unsafe { assert_eq!(tg_terrain_width(std::ptr::null()), 0) };
    }

    #[test]
    fn error_message_buffer_is_truncated_safely() {
        let mut cfg = small_config();
        cfg.width = 0;
        let mut handle = std::ptr::null_mut();
        // SAFETY: cfg and handle are live local storage
        assert_eq!(unsafe { tg_generate(&cfg, &mut handle) }, TG_INVALID_PARAMS);

        let mut tiny = [0i8; 4];
        // SAFETY: tiny holds 4 writable bytes
        let needed = unsafe { tg_last_error(tiny.as_mut_ptr(), tiny.len() as u64) };
        assert!(needed > tiny.len() as u64, "message should not fit");
        // still NUL-terminated within the buffer
        assert_eq!(tiny[3], 0);
    }

    #[test]
    fn abi_version_and_config_size_are_exposed() {
        assert_eq!(tg_abi_version(), TG_ABI_VERSION);
        assert_eq!(tg_config_size(), std::mem::size_of::<TgConfig>() as u64);
    }
}
