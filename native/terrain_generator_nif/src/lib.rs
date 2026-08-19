//! Rustler NIF bindings for the `terrain-generator` crate.
//!
//! The Elixir side of these bindings lives in `lib/terrain_generator.ex`;
//! see `docs/elixir_api.md` for the full API documentation.
//!
//! Both NIFs are scheduled on dirty CPU schedulers because terrain
//! generation takes seconds, far beyond the 1ms budget of a regular NIF.

use rustler::types::atom;
use rustler::{Atom, Binary, Env, NewBinary, NifMap, NifResult};
use terrain_generator::{
    ErosionParams, IslandParams, NoiseParams, RenderStyle, RiverParams, Terrain,
};

/// Generation options, decoded from an Elixir map. The Elixir wrapper is
/// responsible for filling every key with a default before calling in.
#[derive(NifMap)]
struct Options {
    width: u64,
    height: u64,
    seed: u32,
    sea_level: f64,
    octaves: u64,
    frequency: f64,
    lacunarity: f64,
    persistence: f64,
    /// island diameter fraction; `<= 0` disables island shaping
    island: f64,
    coast_noise: f64,
    /// number of erosion droplets; `< 0` = one per cell, `0` disables erosion
    droplets: i64,
    /// fraction of land covered by rivers; `<= 0` disables rivers
    rivers: f64,
}

fn build(opts: &Options) -> Terrain {
    let mut builder = Terrain::builder()
        .size(opts.width as usize, opts.height as usize)
        .seed(opts.seed)
        .sea_level(opts.sea_level as f32)
        .noise(NoiseParams {
            octaves: opts.octaves as usize,
            frequency: opts.frequency,
            lacunarity: opts.lacunarity,
            persistence: opts.persistence,
        });

    builder = if opts.island > 0.0 {
        builder.island(IslandParams {
            size: opts.island as f32,
            coast_noise: opts.coast_noise as f32,
        })
    } else {
        builder.no_island()
    };

    builder = match opts.droplets {
        d if d < 0 => builder.erosion(ErosionParams::default()),
        0 => builder.no_erosion(),
        d => builder.erosion(ErosionParams {
            droplets: Some(d as usize),
            ..Default::default()
        }),
    };

    builder = if opts.rivers > 0.0 {
        builder.rivers(RiverParams {
            density: opts.rivers as f32,
        })
    } else {
        builder.no_rivers()
    };

    builder.generate()
}

fn f32_binary<'a>(env: Env<'a>, data: &[f32]) -> Binary<'a> {
    let mut bin = NewBinary::new(env, data.len() * 4);
    let slice = bin.as_mut_slice();
    for (chunk, v) in slice.chunks_exact_mut(4).zip(data) {
        chunk.copy_from_slice(&v.to_le_bytes());
    }
    bin.into()
}

fn u8_binary<'a>(env: Env<'a>, data: &[u8]) -> Binary<'a> {
    let mut bin = NewBinary::new(env, data.len());
    bin.as_mut_slice().copy_from_slice(data);
    bin.into()
}

/// Generates a terrain and returns
/// `{:ok, {width, height, heightmap, flow, rivers, lakes}}` where
/// `heightmap` and `flow` are binaries of little-endian 32-bit floats and
/// `rivers`/`lakes` are binaries of `0`/`1` bytes, all row-major.
#[allow(clippy::type_complexity)]
#[rustler::nif(schedule = "DirtyCpu")]
fn generate<'a>(
    env: Env<'a>,
    opts: Options,
) -> NifResult<(
    Atom,
    (u64, u64, Binary<'a>, Binary<'a>, Binary<'a>, Binary<'a>),
)> {
    let terrain = build(&opts);

    Ok((
        atom::ok(),
        (
            terrain.width as u64,
            terrain.height as u64,
            f32_binary(env, &terrain.heightmap),
            f32_binary(env, &terrain.flow),
            u8_binary(env, &terrain.rivers),
            u8_binary(env, &terrain.lakes),
        ),
    ))
}

/// Generates a terrain and writes it as a PNG file. `style` is the atom
/// `:shaded` or `:greyscale`. Returns `:ok` or `{:error, reason}`.
#[rustler::nif(schedule = "DirtyCpu")]
fn render_png(opts: Options, path: String, style: Atom) -> NifResult<(Atom, String)> {
    let terrain = build(&opts);

    let style = if style == atoms::greyscale() {
        RenderStyle::Greyscale
    } else {
        RenderStyle::Shaded
    };

    match terrain.save_png(&path, style) {
        Ok(()) => Ok((atom::ok(), String::new())),
        Err(e) => Ok((atom::error(), e.to_string())),
    }
}

mod atoms {
    rustler::atoms! {
        greyscale,
        shaded,
    }
}

rustler::init!("Elixir.TerrainGenerator.Native");
