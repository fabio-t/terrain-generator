//! The [`Terrain`] type and its builder — the main entry point of the crate.

use crate::grid::Grid;
use crate::render::{self, RenderStyle};
use crate::{base, erosion, hydrology};

/// Fractal (fBm) noise parameters for the base heightmap.
#[derive(Clone, Debug)]
pub struct NoiseParams {
    /// Number of noise octaves. More octaves add finer detail. Default: `8`.
    pub octaves: usize,
    /// Number of large features across the map (resolution-independent).
    /// Default: `2.5`.
    pub frequency: f64,
    /// Frequency multiplier between octaves. Default: `2.0`.
    pub lacunarity: f64,
    /// Amplitude multiplier between octaves, in `(0, 1)`. Default: `0.5`.
    pub persistence: f64,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            octaves: 8,
            frequency: 2.5,
            lacunarity: 2.0,
            persistence: 0.5,
        }
    }
}

impl NoiseParams {
    fn validate(&self) {
        assert!(self.octaves > 0, "noise octaves must be positive");
        assert!(
            self.frequency > 0.0 && self.frequency.is_finite(),
            "noise frequency must be positive and finite"
        );
        assert!(
            self.lacunarity > 0.0 && self.lacunarity.is_finite(),
            "noise lacunarity must be positive and finite"
        );
        assert!(
            self.persistence > 0.0 && self.persistence.is_finite(),
            "noise persistence must be positive and finite"
        );
    }
}

/// Radial island mask parameters.
#[derive(Clone, Debug)]
pub struct IslandParams {
    /// Island diameter as a fraction of the shorter map dimension, in
    /// `(0, ~1.2]`. Default: `1.0`.
    pub size: f32,
    /// Strength of the coastline perturbation noise, relative to the
    /// radial gradient: `0` gives a perfectly round island, `1` lets the
    /// noise carve bays, peninsulas and offshore islets. Default: `1.0`.
    pub coast_noise: f32,
}

impl Default for IslandParams {
    fn default() -> Self {
        Self {
            size: 1.0,
            coast_noise: 1.0,
        }
    }
}

impl IslandParams {
    fn validate(&self) {
        assert!(
            self.size > 0.0 && self.size.is_finite(),
            "island size must be positive and finite"
        );
        assert!(
            self.coast_noise >= 0.0 && self.coast_noise.is_finite(),
            "island coast noise must be non-negative and finite"
        );
    }
}

/// Hydraulic erosion parameters. The defaults are good for maps between
/// 256² and 2048²; usually only [`droplets`](Self::droplets) needs tuning.
#[derive(Clone, Debug)]
pub struct ErosionParams {
    /// Number of simulated droplets. `None` (default) uses one droplet per
    /// cell, which is a good baseline.
    pub droplets: Option<usize>,
    /// Maximum steps a droplet lives. Default: `30`.
    pub max_lifetime: usize,
    /// How much a droplet keeps its momentum on slope changes, in `[0, 1)`.
    /// Default: `0.05`.
    pub inertia: f32,
    /// Sediment capacity multiplier. Higher values carve deeper valleys.
    /// Default: `4.0`.
    pub capacity: f32,
    /// Minimum sediment capacity, prevents dead droplets on flats.
    /// Default: `0.01`.
    pub min_capacity: f32,
    /// Fraction of free capacity eroded per step, in `(0, 1]`. Default: `0.3`.
    pub erode_speed: f32,
    /// Fraction of surplus sediment deposited per step, in `(0, 1]`.
    /// Default: `0.3`.
    pub deposit_speed: f32,
    /// Fraction of water lost per step, in `[0, 1)`. Default: `0.01`.
    pub evaporate_speed: f32,
    /// Acceleration applied on descent. Default: `4.0`.
    pub gravity: f32,
    /// Radius (in cells) of the erosion brush. Default: `3`.
    pub radius: usize,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            droplets: None,
            max_lifetime: 30,
            inertia: 0.05,
            capacity: 4.0,
            min_capacity: 0.01,
            erode_speed: 0.3,
            deposit_speed: 0.3,
            evaporate_speed: 0.01,
            gravity: 4.0,
            radius: 3,
        }
    }
}

impl ErosionParams {
    fn validate(&self) {
        assert!(
            self.max_lifetime > 0,
            "erosion max lifetime must be positive"
        );
        assert!(
            (0.0..1.0).contains(&self.inertia),
            "erosion inertia must be in range [0,1)"
        );
        assert!(self.capacity > 0.0, "erosion capacity must be positive");
        assert!(
            self.min_capacity > 0.0,
            "erosion min capacity must be positive"
        );
        assert!(
            self.erode_speed > 0.0 && self.erode_speed <= 1.0,
            "erosion erode speed must be in range (0,1]"
        );
        assert!(
            self.deposit_speed > 0.0 && self.deposit_speed <= 1.0,
            "erosion deposit speed must be in range (0,1]"
        );
        assert!(
            (0.0..1.0).contains(&self.evaporate_speed),
            "erosion evaporate speed must be in range [0,1)"
        );
        assert!(self.gravity > 0.0, "erosion gravity must be positive");
        assert!(self.radius > 0, "erosion radius must be positive");
    }
}

/// River extraction parameters.
#[derive(Clone, Debug)]
pub struct RiverParams {
    /// Approximate fraction of land cells covered by rivers, in `[0, 1]`.
    /// Default: `0.01`.
    pub density: f32,
}

impl Default for RiverParams {
    fn default() -> Self {
        Self { density: 0.01 }
    }
}

impl RiverParams {
    fn validate(&self) {
        assert!(
            (0.0..=1.0).contains(&self.density),
            "river density must be in range [0,1]"
        );
    }
}

/// A generated terrain: heightmap, flow map, river and lake masks.
///
/// All per-cell buffers are [`Grid`]s, indexed as `grid[(x, y)]`; their
/// flat row-major storage is available through [`Grid::as_slice`] for
/// zero-copy hand-off to FFI, GPUs or file formats.
pub struct Terrain {
    /// Sea level the terrain was generated with, in `[0, 1]`.
    pub sea_level: f32,
    /// Elevation per cell, in `[0, 1]`.
    pub heightmap: Grid<f32>,
    /// Log-normalised water flow accumulation per cell, in `[0, 1]`.
    /// Useful as a moisture proxy for biome placement.
    pub flow: Grid<f32>,
    /// River mask per cell: `1` where a river runs, `0` elsewhere.
    pub rivers: Grid<u8>,
    /// Lake mask per cell: `1` where an inland depression holds standing
    /// water, `0` elsewhere. Rivers vanish beneath lakes and continue from
    /// their outlet.
    pub lakes: Grid<u8>,
}

impl Terrain {
    /// Starts building a terrain with default parameters.
    pub fn builder() -> TerrainBuilder {
        TerrainBuilder::default()
    }

    /// Map width in cells.
    pub fn width(&self) -> usize {
        self.heightmap.width()
    }

    /// Map height in cells.
    pub fn height(&self) -> usize {
        self.heightmap.height()
    }

    /// Elevation at `(x, y)`, in `[0, 1]`.
    pub fn height_at(&self, x: usize, y: usize) -> f32 {
        self.heightmap[(x, y)]
    }

    /// Normalised flow accumulation at `(x, y)`, in `[0, 1]`.
    pub fn flow_at(&self, x: usize, y: usize) -> f32 {
        self.flow[(x, y)]
    }

    /// Whether a river runs through `(x, y)`.
    pub fn is_river(&self, x: usize, y: usize) -> bool {
        self.rivers[(x, y)] == 1
    }

    /// Whether `(x, y)` lies in a lake.
    pub fn is_lake(&self, x: usize, y: usize) -> bool {
        self.lakes[(x, y)] == 1
    }

    /// Whether `(x, y)` is covered by water: at or below sea level, in a
    /// lake, or on a river.
    pub fn is_water(&self, x: usize, y: usize) -> bool {
        self.height_at(x, y) <= self.sea_level || self.is_lake(x, y) || self.is_river(x, y)
    }

    /// Renders the terrain to a row-major RGB byte buffer
    /// (`width * height * 3`).
    pub fn to_rgb(&self, style: RenderStyle) -> Vec<u8> {
        render::to_rgb(self, style)
    }

    /// Renders the terrain and writes it as a PNG file.
    ///
    /// Requires the `png` cargo feature (enabled by default).
    #[cfg(feature = "png")]
    pub fn save_png<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        style: RenderStyle,
    ) -> Result<(), render::RenderError> {
        render::save_png(self, path, style)
    }
}

/// Builder for [`Terrain`]. Obtained via [`Terrain::builder`].
///
/// Island shaping, erosion and rivers are all enabled by default; disable
/// them with [`no_island`](Self::no_island), [`no_erosion`](Self::no_erosion)
/// and [`no_rivers`](Self::no_rivers).
///
/// ```
/// use terrain_generator::Terrain;
///
/// let terrain = Terrain::builder().size(128, 128).seed(42).generate();
/// assert_eq!(terrain.heightmap.len(), 128 * 128);
/// ```
#[derive(Clone, Debug)]
pub struct TerrainBuilder {
    width: usize,
    height: usize,
    seed: u32,
    sea_level: f32,
    noise: NoiseParams,
    island: Option<IslandParams>,
    erosion: Option<ErosionParams>,
    rivers: Option<RiverParams>,
}

impl Default for TerrainBuilder {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            seed: 0,
            sea_level: 0.08,
            noise: NoiseParams::default(),
            island: Some(IslandParams::default()),
            erosion: Some(ErosionParams::default()),
            rivers: Some(RiverParams::default()),
        }
    }
}

impl TerrainBuilder {
    /// Sets the map dimensions in cells. Default: `512 x 512`.
    pub fn size(mut self, width: usize, height: usize) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Sets the generation seed. The same seed and parameters always
    /// produce the same terrain. Default: `0`.
    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = seed;
        self
    }

    /// Sets the sea level, in `[0, 1)`. Cells at or below it are ocean.
    /// Default: `0.08`.
    pub fn sea_level(mut self, sea_level: f32) -> Self {
        self.sea_level = sea_level;
        self
    }

    /// Sets the fractal noise parameters for the base heightmap.
    pub fn noise(mut self, noise: NoiseParams) -> Self {
        self.noise = noise;
        self
    }

    /// Enables island shaping with the given parameters (enabled with
    /// defaults unless [`no_island`](Self::no_island) is called).
    pub fn island(mut self, island: IslandParams) -> Self {
        self.island = Some(island);
        self
    }

    /// Disables island shaping: terrain reaches the map borders.
    pub fn no_island(mut self) -> Self {
        self.island = None;
        self
    }

    /// Enables hydraulic erosion with the given parameters (enabled with
    /// defaults unless [`no_erosion`](Self::no_erosion) is called).
    pub fn erosion(mut self, erosion: ErosionParams) -> Self {
        self.erosion = Some(erosion);
        self
    }

    /// Disables hydraulic erosion.
    pub fn no_erosion(mut self) -> Self {
        self.erosion = None;
        self
    }

    /// Enables river extraction with the given parameters (enabled with
    /// defaults unless [`no_rivers`](Self::no_rivers) is called).
    pub fn rivers(mut self, rivers: RiverParams) -> Self {
        self.rivers = Some(rivers);
        self
    }

    /// Disables river and lake extraction (the flow map will be all zeros).
    pub fn no_rivers(mut self) -> Self {
        self.rivers = None;
        self
    }

    /// Runs the full pipeline: noise, island mask, erosion, flow routing.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if any parameter is out of range:
    /// zero dimensions, a sea level outside `[0, 1)`, non-positive noise
    /// parameters, or erosion/river parameters outside the ranges
    /// documented on their fields. Validate user-supplied configuration
    /// before passing it in.
    pub fn generate(&self) -> Terrain {
        self.validate();

        let mut heightmap = base::generate(
            self.width,
            self.height,
            self.seed,
            &self.noise,
            self.island.as_ref(),
        );

        if let Some(erosion_params) = &self.erosion {
            erosion::erode(&mut heightmap, erosion_params, self.seed);
        }

        let (flow, rivers, lakes) = match &self.rivers {
            Some(river_params) => {
                let hydro = hydrology::compute(&heightmap, self.sea_level, river_params);
                (hydro.flow, hydro.rivers, hydro.lakes)
            }
            None => (
                Grid::new(self.width, self.height, 0.0),
                Grid::new(self.width, self.height, 0),
                Grid::new(self.width, self.height, 0),
            ),
        };

        Terrain {
            sea_level: self.sea_level,
            heightmap,
            flow,
            rivers,
            lakes,
        }
    }

    fn validate(&self) {
        assert!(
            self.width > 0 && self.height > 0,
            "width and height must be positive"
        );
        assert!(
            (0.0..1.0).contains(&self.sea_level),
            "sea level must be in range [0,1)"
        );
        self.noise.validate();
        if let Some(island) = &self.island {
            island.validate();
        }
        if let Some(erosion) = &self.erosion {
            erosion.validate();
        }
        if let Some(rivers) = &self.rivers {
            rivers.validate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_pipeline_is_deterministic() {
        let build = || {
            Terrain::builder()
                .size(64, 64)
                .seed(11)
                .erosion(ErosionParams {
                    droplets: Some(2000),
                    ..Default::default()
                })
                .generate()
        };
        let a = build();
        let b = build();
        assert_eq!(a.heightmap, b.heightmap);
        assert_eq!(a.rivers, b.rivers);
        assert_eq!(a.flow, b.flow);
    }

    #[test]
    fn different_seeds_differ() {
        let a = Terrain::builder().size(64, 64).seed(1).generate();
        let b = Terrain::builder().size(64, 64).seed(2).generate();
        assert_ne!(a.heightmap, b.heightmap);
    }

    #[test]
    fn rivers_are_on_land() {
        let t = Terrain::builder()
            .size(128, 128)
            .seed(3)
            .rivers(RiverParams { density: 0.05 })
            .generate();
        for y in 0..t.height() {
            for x in 0..t.width() {
                if t.is_river(x, y) {
                    assert!(t.height_at(x, y) > t.sea_level);
                }
            }
        }
    }

    #[test]
    fn lakes_are_on_land() {
        let t = Terrain::builder().size(128, 128).seed(3).generate();
        for y in 0..t.height() {
            for x in 0..t.width() {
                if t.is_lake(x, y) {
                    assert!(t.height_at(x, y) > t.sea_level);
                }
            }
        }
    }

    #[test]
    fn output_stays_in_unit_range() {
        let t = Terrain::builder().size(64, 64).seed(4).generate();
        assert!(t.heightmap.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(t.flow.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    #[should_panic(expected = "sea level")]
    fn invalid_sea_level_panics() {
        let _ = Terrain::builder().sea_level(1.5).generate();
    }

    #[test]
    #[should_panic(expected = "octaves")]
    fn invalid_noise_params_panic() {
        let _ = Terrain::builder()
            .size(16, 16)
            .noise(NoiseParams {
                octaves: 0,
                ..Default::default()
            })
            .generate();
    }

    #[test]
    #[should_panic(expected = "width and height")]
    fn zero_size_panics() {
        let _ = Terrain::builder().size(0, 64).generate();
    }
}
