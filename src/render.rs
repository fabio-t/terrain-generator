//! Rendering of generated terrain to RGB buffers and PNG files.
//!
//! [`to_rgb`] has no dependencies and is always available; [`save_png`]
//! (and [`RenderError`]) require the `png` cargo feature.

use crate::Terrain;

/// A single output layer of a [`Terrain`], for writing one file per layer.
///
/// [`Shaded`](Self::Shaded) is a picture; the rest are *data* layers meant
/// to be read back by other tools, so they are written losslessly:
/// elevation and flow as 16-bit greyscale (8 bits would quantise elevation
/// into visible terraces), masks as 8-bit black/white.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The shaded relief render — biome colours, hillshading, water.
    Shaded,
    /// Elevation, 16-bit greyscale, `0` = 0.0 and `65535` = 1.0.
    Height,
    /// Normalised flow accumulation, 16-bit greyscale.
    Flow,
    /// River mask, 8-bit: `255` where a river runs.
    Rivers,
    /// Lake mask, 8-bit: `255` where a lake stands.
    Lakes,
    /// Ocean mask, 8-bit: `255` where the cell is sea.
    Ocean,
}

impl Layer {
    /// Every layer, in a stable order.
    pub const ALL: [Layer; 6] = [
        Layer::Shaded,
        Layer::Height,
        Layer::Flow,
        Layer::Rivers,
        Layer::Lakes,
        Layer::Ocean,
    ];

    /// The lowercase name used by [`FromStr`](std::str::FromStr) and
    /// suitable as a filename suffix.
    pub fn name(self) -> &'static str {
        match self {
            Layer::Shaded => "shaded",
            Layer::Height => "height",
            Layer::Flow => "flow",
            Layer::Rivers => "rivers",
            Layer::Lakes => "lakes",
            Layer::Ocean => "ocean",
        }
    }
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Returned when a string does not name a [`Layer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownLayer(pub String);

impl std::fmt::Display for UnknownLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown layer '{}' (expected one of: shaded, height, flow, rivers, lakes, ocean)",
            self.0
        )
    }
}

impl std::error::Error for UnknownLayer {}

impl std::str::FromStr for Layer {
    type Err = UnknownLayer;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Layer::ALL
            .into_iter()
            .find(|l| l.name() == s)
            .ok_or_else(|| UnknownLayer(s.to_string()))
    }
}

/// How a [`Terrain`] is turned into an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RenderStyle {
    /// Biome-like colours (ocean, beach, grass, forest, rock, snow) with
    /// hillshading, rivers and lakes drawn in blue.
    #[default]
    Shaded,
    /// Plain greyscale heightmap (black = lowest, white = highest).
    Greyscale,
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
    let w = terrain.width();
    let h = terrain.height();
    let sea = terrain.sea_level;
    let mut data = Vec::with_capacity(w * h * 3);

    for y in 0..h {
        for x in 0..w {
            let v = terrain.heightmap[(x, y)];

            let rgb = match style {
                RenderStyle::Greyscale => {
                    let g = v.clamp(0.0, 1.0);
                    [g, g, g]
                }
                RenderStyle::Shaded => {
                    if terrain.rivers[(x, y)] == 1 {
                        // slightly darker in steep terrain so rivers read as
                        // carved into the landscape
                        let shade = 0.8 + 0.2 * hillshade(terrain, x, y);
                        [
                            RIVER_COLOR[0] * shade,
                            RIVER_COLOR[1] * shade,
                            RIVER_COLOR[2] * shade,
                        ]
                    } else if terrain.lakes[(x, y)] == 1 {
                        LAKE_COLOR
                    } else if terrain.ocean[(x, y)] == 1 {
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
    let w = terrain.width();
    let h = terrain.height();
    let hm = &terrain.heightmap;

    let xl = x.saturating_sub(1);
    let xr = (x + 1).min(w - 1);
    let yu = y.saturating_sub(1);
    let yd = (y + 1).min(h - 1);

    // vertical exaggeration keeps the relief legible at any resolution
    let z_scale = 0.15 * w.max(h) as f32;

    let dzdx = (hm[(xr, y)] - hm[(xl, y)]) / (xr - xl).max(1) as f32 * z_scale;
    let dzdy = (hm[(x, yd)] - hm[(x, yu)]) / (yd - yu).max(1) as f32 * z_scale;

    // light from the north-west, 45 degrees above the horizon
    let light = [-0.5f32, -0.5, std::f32::consts::FRAC_1_SQRT_2];

    let norm = 1.0 / (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
    let dot = (-dzdx * light[0] - dzdy * light[1] + light[2]) * norm;

    dot.clamp(0.0, 1.0)
}

#[cfg(feature = "png")]
mod png_output {
    use std::fs::File;
    use std::io::BufWriter;
    use std::path::Path;

    use super::{to_rgb, RenderStyle};
    use crate::Terrain;

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

    /// Writes one [`Layer`](super::Layer) of `terrain` as a PNG file.
    ///
    /// Data layers are written losslessly — see [`Layer`](super::Layer).
    pub fn save_layer_png<P: AsRef<Path>>(
        terrain: &Terrain,
        layer: super::Layer,
        path: P,
    ) -> Result<(), RenderError> {
        use super::Layer;

        let (w, h) = (terrain.width() as u32, terrain.height() as u32);

        match layer {
            Layer::Shaded => save_png(terrain, path, RenderStyle::Shaded),
            Layer::Height => write_gray16(&terrain.heightmap, w, h, path),
            Layer::Flow => write_gray16(&terrain.flow, w, h, path),
            Layer::Rivers => write_mask(&terrain.rivers, w, h, path),
            Layer::Lakes => write_mask(&terrain.lakes, w, h, path),
            Layer::Ocean => write_mask(&terrain.ocean, w, h, path),
        }
    }

    /// 16-bit greyscale from a `[0, 1]` grid, big-endian as PNG requires.
    fn write_gray16<P: AsRef<Path>>(
        grid: &crate::Grid<f32>,
        w: u32,
        h: u32,
        path: P,
    ) -> Result<(), RenderError> {
        let mut data = Vec::with_capacity(grid.len() * 2);
        for v in grid.iter() {
            let q = (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
            data.extend_from_slice(&q.to_be_bytes());
        }
        write_png(
            path,
            w,
            h,
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            &data,
        )
    }

    /// 8-bit black/white from a 0/1 mask grid.
    fn write_mask<P: AsRef<Path>>(
        grid: &crate::Grid<u8>,
        w: u32,
        h: u32,
        path: P,
    ) -> Result<(), RenderError> {
        let data: Vec<u8> = grid.iter().map(|v| if *v == 0 { 0 } else { 255 }).collect();
        write_png(
            path,
            w,
            h,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &data,
        )
    }

    fn write_png<P: AsRef<Path>>(
        path: P,
        w: u32,
        h: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        data: &[u8],
    ) -> Result<(), RenderError> {
        let file = File::create(path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), w, h);
        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder.write_header()?.write_image_data(data)?;
        Ok(())
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

        let mut encoder =
            png::Encoder::new(writer, terrain.width() as u32, terrain.height() as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);

        let mut png_writer = encoder.write_header()?;
        png_writer.write_image_data(&data)?;

        Ok(())
    }
}

#[cfg(feature = "png")]
pub use png_output::{save_layer_png, save_png, RenderError};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Terrain;

    fn small_terrain() -> Terrain {
        Terrain::builder().size(32, 32).seed(1).generate()
    }

    #[test]
    fn rgb_buffer_has_expected_size() {
        let t = small_terrain();
        assert_eq!(to_rgb(&t, RenderStyle::Shaded).len(), 32 * 32 * 3);
        assert_eq!(to_rgb(&t, RenderStyle::Greyscale).len(), 32 * 32 * 3);
    }

    #[test]
    fn rendering_is_deterministic() {
        let a = to_rgb(&small_terrain(), RenderStyle::Shaded);
        let b = to_rgb(&small_terrain(), RenderStyle::Shaded);
        assert_eq!(a, b);
    }

    #[test]
    fn greyscale_encodes_elevation() {
        let t = small_terrain();
        let rgb = to_rgb(&t, RenderStyle::Greyscale);
        let expected = (t.heightmap[(0, 0)].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        assert_eq!(rgb[0], expected);
        assert_eq!(rgb[1], expected);
        assert_eq!(rgb[2], expected);
    }
}
