use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use terrain_generator::{
    ErosionParams, IslandParams, NoiseParams, RenderStyle, RiverParams, Terrain,
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
    #[arg(long, default_value_t = 0.9)]
    island: f32,

    /// Strength of the coastline perturbation noise
    #[arg(long, default_value_t = 0.35)]
    coast_noise: f32,

    /// Disable island shaping (terrain reaches the borders)
    #[arg(long)]
    no_island: bool,

    /// Number of erosion droplets [default: one per cell]
    #[arg(long)]
    droplets: Option<usize>,

    /// Disable hydraulic erosion
    #[arg(long)]
    no_erosion: bool,

    /// Approximate fraction of land covered by rivers
    #[arg(long, default_value_t = 0.01)]
    rivers: f32,

    /// Disable river extraction
    #[arg(long)]
    no_rivers: bool,

    /// Render a greyscale heightmap instead of shaded colours
    #[arg(long)]
    grey: bool,
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
    } else {
        builder.rivers(RiverParams {
            density: args.rivers,
        })
    };

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
        let terrain = builder.clone().seed(seed).generate();

        let path = args.output.join(format!("{}{}.png", args.prefix, seed));
        if let Err(e) = terrain.save_png(&path, style) {
            eprintln!("failed to write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }

        println!(
            "seed {seed}: {} ({:.1}s)",
            path.display(),
            start.elapsed().as_secs_f32()
        );
    }

    ExitCode::SUCCESS
}
