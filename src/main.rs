use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use terrain_generator::{
    ArchipelagoParams, BreachParams, ErosionParams, IslandParams, Layer, NoiseParams, RenderStyle,
    RiverParams, Terrain,
};

/// Procedural terrain generator: fractal noise, hydraulic erosion and rivers.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Map width in cells
    #[arg(short = 'W', long, default_value_t = 512)]
    width: usize,

    /// Map height in cells
    #[arg(short = 'H', long, default_value_t = 512)]
    height: usize,

    /// First seed to generate
    #[arg(short, long, default_value_t = 0)]
    seed: u32,

    /// Number of maps to generate (seeds seed..seed+count)
    #[arg(short = 'n', long, default_value_t = 1)]
    count: u32,

    /// Output directory
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Output file prefix; files are named <prefix><seed>.png
    #[arg(long, default_value = "map")]
    prefix: String,

    /// Sea level in [0, 1)
    #[arg(long, default_value_t = 0.08)]
    sea_level: f32,

    /// Noise octaves
    #[arg(long, default_value_t = 8)]
    octaves: usize,

    /// Number of large features across the map
    #[arg(long, default_value_t = 2.5)]
    frequency: f64,

    /// Frequency multiplier between octaves
    #[arg(long, default_value_t = 2.0)]
    lacunarity: f64,

    /// Amplitude multiplier between octaves
    #[arg(long, default_value_t = 0.5)]
    persistence: f64,

    /// Island diameter as a fraction of the shorter map dimension
    #[arg(long, default_value_t = 1.0)]
    island: f32,

    /// Strength of the coastline perturbation noise (0 = round island)
    #[arg(long, default_value_t = 1.0)]
    coast_noise: f32,

    /// Disable island shaping (terrain reaches the borders)
    #[arg(long)]
    no_island: bool,

    /// Generate an archipelago of N islands instead of a single island
    #[arg(long, value_name = "N", conflicts_with = "no_island")]
    archipelago: Option<usize>,

    /// Mean island diameter for --archipelago, as a fraction of the
    /// shorter map side
    #[arg(long, default_value_t = 0.35)]
    islands_size: f32,

    /// Per-island size variation for --archipelago
    #[arg(long, default_value_t = 0.35)]
    islands_variation: f32,

    /// How far island centres spread over the map for --archipelago
    #[arg(long, default_value_t = 0.85)]
    islands_spread: f32,

    /// Minimum spacing between island centres for --archipelago, as a
    /// fraction of the shorter map side
    #[arg(long, default_value_t = 0.22)]
    islands_spacing: f32,

    /// Number of erosion droplets [default: one per cell]
    #[arg(long)]
    droplets: Option<usize>,

    /// Disable hydraulic erosion
    #[arg(long)]
    no_erosion: bool,

    /// Approximate fraction of land covered by rivers
    #[arg(long, default_value_t = 0.01)]
    rivers: f32,

    /// Rivers where at least this many cells of catchment drain through
    /// (absolute alternative to --rivers)
    #[arg(long, conflicts_with = "rivers")]
    river_min_upstream: Option<f32>,

    /// Rivers where catchment reaches this multiple of the longer map edge
    /// (resolution-independent alternative to --rivers)
    #[arg(long, conflicts_with_all = ["rivers", "river_min_upstream"])]
    river_per_edge: Option<f32>,

    /// Disable river extraction
    #[arg(long)]
    no_rivers: bool,

    /// Carve outlets from closed basins bigger than this fraction of the
    /// map, so they drain instead of becoming inland seas
    #[arg(long)]
    breach_basins: Option<f32>,

    /// Render a greyscale heightmap instead of shaded colours
    #[arg(long, conflicts_with = "layers")]
    grey: bool,

    /// Write one file per layer from a single generation, e.g.
    /// --layers height,flow,rivers,lakes,ocean. Files are named
    /// <prefix><seed>_<layer>.png. Elevation and flow are written as
    /// 16-bit greyscale so no precision is lost; masks as 8-bit
    /// black/white. Valid layers: shaded, height, flow, rivers, lakes,
    /// ocean
    #[arg(long, value_delimiter = ',', value_name = "LIST")]
    layers: Vec<Layer>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let mut builder = Terrain::builder()
        .size(args.width, args.height)
        .sea_level(args.sea_level)
        .noise(NoiseParams {
            octaves: args.octaves,
            frequency: args.frequency,
            lacunarity: args.lacunarity,
            persistence: args.persistence,
        });

    builder = if args.no_island {
        builder.no_island()
    } else if let Some(islands) = args.archipelago {
        builder.archipelago(ArchipelagoParams {
            islands,
            size: args.islands_size,
            size_variation: args.islands_variation,
            spread: args.islands_spread,
            min_spacing: args.islands_spacing,
            coast_noise: args.coast_noise,
        })
    } else {
        builder.island(IslandParams {
            size: args.island,
            coast_noise: args.coast_noise,
        })
    };

    builder = if args.no_erosion {
        builder.no_erosion()
    } else {
        builder.erosion(ErosionParams {
            droplets: args.droplets,
            ..Default::default()
        })
    };

    builder = if args.no_rivers {
        builder.no_rivers()
    } else if let Some(cells) = args.river_min_upstream {
        builder.rivers(RiverParams::min_upstream_cells(cells))
    } else if let Some(factor) = args.river_per_edge {
        builder.rivers(RiverParams::per_edge(factor))
    } else {
        builder.rivers(RiverParams::density(args.rivers))
    };

    if let Some(max_lake_fraction) = args.breach_basins {
        builder = builder.breach_basins(BreachParams { max_lake_fraction });
    }

    let style = if args.grey {
        RenderStyle::Greyscale
    } else {
        RenderStyle::Shaded
    };

    if let Err(e) = std::fs::create_dir_all(&args.output) {
        eprintln!(
            "cannot create output directory {}: {e}",
            args.output.display()
        );
        return ExitCode::FAILURE;
    }

    for seed in args.seed..args.seed.saturating_add(args.count) {
        let start = Instant::now();
        let terrain = match builder.clone().seed(seed).try_generate() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("invalid parameters: {e}");
                return ExitCode::FAILURE;
            }
        };

        let mut written = Vec::new();

        if args.layers.is_empty() {
            // default: one shaded (or greyscale) image, as before
            let path = args.output.join(format!("{}{}.png", args.prefix, seed));
            if let Err(e) = terrain.save_png(&path, style) {
                eprintln!("failed to write {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
            written.push(path);
        } else {
            for layer in &args.layers {
                let path = args
                    .output
                    .join(format!("{}{}_{}.png", args.prefix, seed, layer));
                if let Err(e) = terrain.save_layer_png(*layer, &path) {
                    eprintln!("failed to write {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
                written.push(path);
            }
        }

        let names: Vec<String> = written.iter().map(|p| p.display().to_string()).collect();
        println!(
            "seed {seed}: {} ({:.1}s)",
            names.join(", "),
            start.elapsed().as_secs_f32()
        );
    }

    ExitCode::SUCCESS
}
