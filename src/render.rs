//! Rendering of generated terrain to RGB buffers and PNG files.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::Terrain;

/// How a [`Terrain`] is turned into an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderStyle {
    /// Biome-like colours (ocean, beach, grass, forest, rock, snow) with
    /// hillshading, rivers drawn in blue.
    #[default]
    Shaded,
    /// Plain greyscale heightmap (black = lowest, white = highest).
    Greyscale,
}

/// Errors that can occur while writing an image.
#[derive(Debug)]
pub enum RenderError {
    Io(std::io::Error),
    Png(png::EncodingError),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Io(e) => write!(f, "io error: {e}"),
            RenderError::Png(e) => write!(f, "png encoding error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
    }
}

impl From<png::EncodingError> for RenderError {
    fn from(e: png::EncodingError) -> Self {
        RenderError::Png(e)
    }
}

type Rgb = [f32; 3];

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Piecewise-linear colour ramp over `(position, colour)` stops.
fn ramp(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return lerp(c0, c1, f.clamp(0.0, 1.0));
        }
    }
    stops.last().unwrap().1
}

const WATER_RAMP: &[(f32, Rgb)] = &[
    (0.0, [0.05, 0.15, 0.30]), // deep ocean
    (0.7, [0.15, 0.35, 0.55]),
    (1.0, [0.30, 0.55, 0.75]), // shallows
];

const LAND_RAMP: &[(f32, Rgb)] = &[
    (0.00, [0.80, 0.73, 0.55]), // beach sand
    (0.04, [0.55, 0.65, 0.35]), // lowland grass
    (0.30, [0.25, 0.45, 0.22]), // forest
    (0.55, [0.45, 0.42, 0.36]), // bare rock
    (0.75, [0.60, 0.58, 0.55]), // high rock
    (0.90, [0.93, 0.93, 0.95]), // snow line
    (1.00, [1.00, 1.00, 1.00]), // peaks
];

const RIVER_COLOR: Rgb = [0.20, 0.45, 0.70];
const LAKE_COLOR: Rgb = [0.17, 0.40, 0.62];

/// Renders `terrain` to a row-major RGB byte buffer (`width * height * 3`).
pub fn to_rgb(terrain: &Terrain, style: RenderStyle) -> Vec<u8> {
    let w = terrain.width;
    let h = terrain.height;
    let sea = terrain.sea_level;
    let mut data = Vec::with_capacity(w * h * 3);

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let v = terrain.heightmap[idx];

            let rgb = match style {
                RenderStyle::Greyscale => {
                    let g = v.clamp(0.0, 1.0);
                    [g, g, g]
                }
                RenderStyle::Shaded => {
                    if terrain.rivers[idx] == 1 {
                        // slightly darker in steep terrain so rivers read as
                        // carved into the landscape
                        let shade = 0.8 + 0.2 * hillshade(terrain, x, y);
                        [
                            RIVER_COLOR[0] * shade,
                            RIVER_COLOR[1] * shade,
                            RIVER_COLOR[2] * shade,
                        ]
                    } else if terrain.lakes[idx] == 1 {
                        LAKE_COLOR
                    } else if v <= sea {
                        ramp(WATER_RAMP, if sea > 0.0 { v / sea } else { 0.0 })
                    } else {
                        let t = (v - sea) / (1.0 - sea);
                        let base = ramp(LAND_RAMP, t);
                        let shade = 0.55 + 0.45 * hillshade(terrain, x, y);
                        [base[0] * shade, base[1] * shade, base[2] * shade]
                    }
                }
            };

            data.push((rgb[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            data.push((rgb[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            data.push((rgb[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
    }

    data
}

/// Lambertian hillshade in `[0, 1]` from central differences, with a
/// north-western light source.
fn hillshade(terrain: &Terrain, x: usize, y: usize) -> f32 {
    let w = terrain.width;
    let h = terrain.height;
    let hm = &terrain.heightmap;

    let xl = x.saturating_sub(1);
    let xr = (x + 1).min(w - 1);
    let yu = y.saturating_sub(1);
    let yd = (y + 1).min(h - 1);

    // vertical exaggeration keeps the relief legible at any resolution
    let z_scale = 0.15 * w.max(h) as f32;

    let dzdx = (hm[y * w + xr] - hm[y * w + xl]) / (xr - xl).max(1) as f32 * z_scale;
    let dzdy = (hm[yd * w + x] - hm[yu * w + x]) / (yd - yu).max(1) as f32 * z_scale;

    // light from the north-west, 45 degrees above the horizon
    let light = [-0.5f32, -0.5, std::f32::consts::FRAC_1_SQRT_2];

    let norm = 1.0 / (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
    let dot = (-dzdx * light[0] - dzdy * light[1] + light[2]) * norm;

    dot.clamp(0.0, 1.0)
}

/// Renders `terrain` and writes it as a PNG file.
pub fn save_png<P: AsRef<Path>>(
    terrain: &Terrain,
    path: P,
    style: RenderStyle,
) -> Result<(), RenderError> {
    let data = to_rgb(terrain, style);

    let file = File::create(path)?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, terrain.width as u32, terrain.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);

    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(&data)?;

    Ok(())
}
