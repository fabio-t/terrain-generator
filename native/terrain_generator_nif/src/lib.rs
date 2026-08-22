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
    ArchipelagoParams, BreachParams, ErosionParams, IslandParams, NoiseParams, RenderStyle,
    RiverParams, Terrain,
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
    /// number of islands; `0` keeps the single-island mask
    archipelago: i64,
    islands_size: f64,
    islands_variation: f64,
    islands_spread: f64,
    islands_spacing: f64,
    /// number of erosion droplets; `< 0` = one per cell, `0` disables erosion
    droplets: i64,
    /// largest basin kept as a lake, as a map fraction; `<= 0` disables
    /// basin breaching
    breach: f64,
    /// fraction of land covered by rivers; `<= 0` disables rivers
    rivers: f64,
    /// absolute catchment threshold in cells; `> 0` overrides `rivers`
    river_min_upstream: f64,
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

    builder = if opts.archipelago > 0 {
        builder.archipelago(ArchipelagoParams {
            islands: opts.archipelago as usize,
            size: opts.islands_size as f32,
            size_variation: opts.islands_variation as f32,
            spread: opts.islands_spread as f32,
            min_spacing: opts.islands_spacing as f32,
            coast_noise: opts.coast_noise as f32,
        })
    } else if opts.island > 0.0 {
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

    if opts.breach > 0.0 {
        builder = builder.breach_basins(BreachParams {
            max_lake_fraction: opts.breach as f32,
        });
    }

    builder = if opts.river_min_upstream > 0.0 {
        builder.rivers(RiverParams::min_upstream_cells(
            opts.river_min_upstream as f32,
        ))
    } else if opts.rivers > 0.0 {
        builder.rivers(RiverParams::density(opts.rivers as f32))
    } else {
        builder.no_rivers()
    };

    builder.generate()
}

fn f32_binary<'a>(env: Env<'a>, data: &[f32]) -> Binary<'a> {
    let mut bin = NewBinary::new(env, data.len() * 4);
    // as_chunks_mut gives fixed-size arrays, so each float is written with
    // a plain assignment rather than a length-checked copy
    let (chunks, _) = bin.as_mut_slice().as_chunks_mut::<4>();
    for (chunk, v) in chunks.iter_mut().zip(data) {
        *chunk = v.to_le_bytes();
    }
    bin.into()
}

fn u8_binary<'a>(env: Env<'a>, data: &[u8]) -> Binary<'a> {
    let mut bin = NewBinary::new(env, data.len());
    bin.as_mut_slice().copy_from_slice(data);
    bin.into()
}

/// The generated layers, encoded as an Elixir map so the shape can grow
/// without breaking positional matches on the caller's side.
#[derive(NifMap)]
struct Generated<'a> {
    width: u64,
    height: u64,
    heightmap: Binary<'a>,
    flow: Binary<'a>,
    flow_accumulation: Binary<'a>,
    rivers: Binary<'a>,
    lakes: Binary<'a>,
    ocean: Binary<'a>,
}

/// Generates a terrain and returns `{:ok, map}` where the float layers are
/// binaries of little-endian 32-bit floats and the masks binaries of `0`/`1`
/// bytes, all row-major.
#[rustler::nif(schedule = "DirtyCpu")]
fn generate<'a>(env: Env<'a>, opts: Options) -> NifResult<(Atom, Generated<'a>)> {
    let terrain = build(&opts);

    Ok((
        atom::ok(),
        Generated {
            width: terrain.width() as u64,
            height: terrain.height() as u64,
            heightmap: f32_binary(env, terrain.heightmap.as_slice()),
            flow: f32_binary(env, terrain.flow.as_slice()),
            flow_accumulation: f32_binary(env, terrain.flow_accumulation.as_slice()),
            rivers: u8_binary(env, terrain.rivers.as_slice()),
            lakes: u8_binary(env, terrain.lakes.as_slice()),
            ocean: u8_binary(env, terrain.ocean.as_slice()),
        },
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
