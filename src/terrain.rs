//! The [`Terrain`] type and its builder — the main entry point of the crate.

use crate::grid::Grid;
use crate::render::{self, RenderStyle};
use crate::{base, breach, erosion, hydrology};

/// The most cells a map can have: `width * height` must not exceed it.
///
/// That is `65535 x 65535`, or 16 GiB for each float layer. Hydrology and
/// basin breaching index cells with `u32`, which sets the limit.
pub const MAX_CELLS: usize = u32::MAX as usize;

/// An invalid generation parameter.
///
/// Returned by [`TerrainBuilder::try_generate`]; [`TerrainBuilder::generate`]
/// panics with the same message instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamError {
    /// The offending parameter, e.g. `"sea level"` or `"noise octaves"`.
    pub param: &'static str,
    /// Human-readable description of the valid range.
    pub message: String,
}

impl ParamError {
    fn new(param: &'static str, message: impl Into<String>) -> Self {
        Self {
            param,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.param, self.message)
    }
}

impl std::error::Error for ParamError {}

fn check(ok: bool, param: &'static str, message: &str) -> Result<(), ParamError> {
    if ok {
        Ok(())
    } else {
        Err(ParamError::new(param, message))
    }
}

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
    fn validate(&self) -> Result<(), ParamError> {
        check(self.octaves > 0, "noise octaves", "must be positive")?;
        check(
            self.frequency > 0.0 && self.frequency.is_finite(),
            "noise frequency",
            "must be positive and finite",
        )?;
        check(
            self.lacunarity > 0.0 && self.lacunarity.is_finite(),
            "noise lacunarity",
            "must be positive and finite",
        )?;
        check(
            self.persistence > 0.0 && self.persistence.is_finite(),
            "noise persistence",
            "must be positive and finite",
        )
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
    fn validate(&self) -> Result<(), ParamError> {
        check(
            self.size > 0.0 && self.size.is_finite(),
            "island size",
            "must be positive and finite",
        )?;
        check(
            self.coast_noise >= 0.0 && self.coast_noise.is_finite(),
            "island coast noise",
            "must be non-negative and finite",
        )
    }
}

/// Archipelago mask parameters: several radial islands instead of one.
///
/// Placement is derived from the generation seed on its own random stream,
/// so enabling archipelago mode cannot change the output of any
/// single-island or mask-less generation.
///
/// # Why radial islands rather than a noise threshold
///
/// A common alternative is to threshold a noise field and call whatever
/// stays above the cut an island. That yields organic chains, but the
/// caller gets no say in how many islands appear or how big they are — the
/// count falls out of the noise, and small threshold changes make it jump
/// around. Placing explicit radial falloffs keeps `islands` and `size` as
/// real, predictable controls, and reuses the same coastline noise that
/// shapes single islands, so an archipelago's islands look like the
/// single-island ones rather than like a different generator. The trade-off
/// is that neighbours can merge into one landmass when they overlap, which
/// is why the count is approximate — see
/// [`min_spacing`](Self::min_spacing).
#[derive(Clone, Debug)]
pub struct ArchipelagoParams {
    /// How many islands to place. Default: `5`.
    pub islands: usize,
    /// Mean island diameter as a fraction of the shorter map dimension.
    /// Default: `0.35`.
    pub size: f32,
    /// Per-island random size variation, in `[0, 1)`: `0.35` means radii
    /// vary by ±35% around [`size`](Self::size). Default: `0.35`.
    pub size_variation: f32,
    /// How much of the map centres may spread over, in `[0, 1]`: `1.0`
    /// uses the whole map, smaller values cluster islands centrally.
    /// Default: `0.85`.
    pub spread: f32,
    /// Minimum distance between island centres, as a fraction of the
    /// shorter map dimension. Best-effort — see the type docs.
    /// Default: `0.22`.
    pub min_spacing: f32,
    /// Coastline perturbation strength, as in
    /// [`IslandParams::coast_noise`]. Default: `1.0`.
    pub coast_noise: f32,
}

impl Default for ArchipelagoParams {
    fn default() -> Self {
        Self {
            islands: 5,
            size: 0.35,
            size_variation: 0.35,
            spread: 0.85,
            min_spacing: 0.22,
            coast_noise: 1.0,
        }
    }
}

impl ArchipelagoParams {
    /// An archipelago of `islands` islands, other settings default.
    pub fn with_islands(islands: usize) -> Self {
        Self {
            islands,
            ..Default::default()
        }
    }

    fn validate(&self) -> Result<(), ParamError> {
        check(self.islands > 0, "archipelago islands", "must be positive")?;
        check(
            self.size > 0.0 && self.size.is_finite(),
            "archipelago size",
            "must be positive and finite",
        )?;
        check(
            (0.0..1.0).contains(&self.size_variation),
            "archipelago size variation",
            "must be in range [0,1)",
        )?;
        check(
            (0.0..=1.0).contains(&self.spread),
            "archipelago spread",
            "must be in range [0,1]",
        )?;
        check(
            self.min_spacing >= 0.0 && self.min_spacing.is_finite(),
            "archipelago min spacing",
            "must be non-negative and finite",
        )?;
        check(
            self.coast_noise >= 0.0 && self.coast_noise.is_finite(),
            "archipelago coast noise",
            "must be non-negative and finite",
        )
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
    fn validate(&self) -> Result<(), ParamError> {
        check(
            self.max_lifetime > 0,
            "erosion max lifetime",
            "must be positive",
        )?;
        check(
            (0.0..1.0).contains(&self.inertia),
            "erosion inertia",
            "must be in range [0,1)",
        )?;
        check(self.capacity > 0.0, "erosion capacity", "must be positive")?;
        check(
            self.min_capacity > 0.0,
            "erosion min capacity",
            "must be positive",
        )?;
        check(
            self.erode_speed > 0.0 && self.erode_speed <= 1.0,
            "erosion erode speed",
            "must be in range (0,1]",
        )?;
        check(
            self.deposit_speed > 0.0 && self.deposit_speed <= 1.0,
            "erosion deposit speed",
            "must be in range (0,1]",
        )?;
        check(
            (0.0..1.0).contains(&self.evaporate_speed),
            "erosion evaporate speed",
            "must be in range [0,1)",
        )?;
        check(self.gravity > 0.0, "erosion gravity", "must be positive")?;
        check(self.radius > 0, "erosion radius", "must be positive")
    }
}

/// How the river network is cut out of the flow accumulation map.
///
/// Which one to use:
///
/// * [`Density`](Self::Density) asks for an *area share* — "about 1% of the
///   land should be river". It adapts to any terrain, but it does not
///   transfer between map sizes: catchments grow with area while channels
///   grow with length, so the same share yields a visually denser network
///   on a small map than on a large one. Good for one-off renders.
/// * [`MinUpstreamCells`](Self::MinUpstreamCells) is absolute — "a channel
///   starts once 500 cells drain through". Physically meaningful and
///   comparable across maps *of the same cell size*, so it is the right
///   choice when cells have a fixed real-world size (a game world tiled
///   from several maps).
/// * [`PerEdge`](Self::PerEdge) scales the absolute threshold with the map
///   edge, i.e. required upstream cells = `factor * max(width, height)`.
///   Use it when a map always represents "one island", whatever its
///   resolution, and you want the same-looking network at every resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RiverThreshold {
    /// Approximate fraction of land cells covered by rivers, in `[0, 1]`.
    Density(f32),
    /// Minimum upstream catchment, in cells, for a cell to be a river.
    MinUpstreamCells(f32),
    /// Minimum upstream catchment as a multiple of the longer map edge.
    PerEdge(f32),
}

/// River extraction parameters.
#[derive(Clone, Debug)]
pub struct RiverParams {
    /// How the river network is thresholded out of flow accumulation.
    pub threshold: RiverThreshold,
}

impl Default for RiverParams {
    fn default() -> Self {
        Self {
            threshold: RiverThreshold::Density(0.01),
        }
    }
}

impl RiverParams {
    /// Rivers covering approximately `density` of the land area.
    pub fn density(density: f32) -> Self {
        Self {
            threshold: RiverThreshold::Density(density),
        }
    }

    /// Rivers wherever at least `cells` of catchment drain through.
    pub fn min_upstream_cells(cells: f32) -> Self {
        Self {
            threshold: RiverThreshold::MinUpstreamCells(cells),
        }
    }

    /// Rivers wherever at least `factor * max(width, height)` cells of
    /// catchment drain through.
    pub fn per_edge(factor: f32) -> Self {
        Self {
            threshold: RiverThreshold::PerEdge(factor),
        }
    }

    fn validate(&self) -> Result<(), ParamError> {
        match self.threshold {
            RiverThreshold::Density(d) => check(
                (0.0..=1.0).contains(&d),
                "river density",
                "must be in range [0,1]",
            ),
            RiverThreshold::MinUpstreamCells(c) => check(
                c >= 0.0 && c.is_finite(),
                "river min upstream cells",
                "must be non-negative and finite",
            ),
            RiverThreshold::PerEdge(f) => check(
                f >= 0.0 && f.is_finite(),
                "river per-edge factor",
                "must be non-negative and finite",
            ),
        }
    }
}

/// Basin breaching parameters. See
/// [`TerrainBuilder::breach_basins`] for what the stage does.
#[derive(Clone, Debug)]
pub struct BreachParams {
    /// Largest closed basin, as a fraction of total map cells, that may
    /// remain a lake. Bigger basins get an outlet carved. Default: `0.005`
    /// (0.5% of the map — on a 512² map, about 1300 cells).
    pub max_lake_fraction: f32,
}

impl Default for BreachParams {
    fn default() -> Self {
        Self {
            max_lake_fraction: 0.005,
        }
    }
}

impl BreachParams {
    fn validate(&self) -> Result<(), ParamError> {
        check(
            (0.0..=1.0).contains(&self.max_lake_fraction),
            "breach max lake fraction",
            "must be in range [0,1]",
        )
    }
}

/// A generated terrain: elevation plus the hydrology layers derived from it.
///
/// All per-cell buffers are [`Grid`]s, indexed as `grid[(x, y)]`; their
/// flat row-major storage is available through [`Grid::as_slice`] for
/// zero-copy hand-off to FFI, GPUs or file formats.
///
/// The layers are independent and raw on purpose: this crate decides what
/// the terrain *is*, not what it means. Consumers that classify water for
/// their own rules (salinity, drinkability, navigability) should build on
/// [`ocean`](Self::ocean), [`lakes`](Self::lakes) and
/// [`flow_accumulation`](Self::flow_accumulation) rather than on rendered
/// colours or on elevation thresholds.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// Sea level the terrain was generated with, in `[0, 1]`.
    pub sea_level: f32,
    /// Elevation per cell, in `[0, 1]`.
    pub heightmap: Grid<f32>,
    /// Raw D8 flow accumulation per cell, in cells of upstream catchment
    /// (each cell contributes 1, including itself). Use this to size
    /// channels — river width typically scales with the square root of
    /// catchment area. All-zero when river extraction is disabled.
    pub flow_accumulation: Grid<f32>,
    /// [`flow_accumulation`](Self::flow_accumulation) log-normalised into
    /// `[0, 1]`, which is the more useful form as a moisture proxy for
    /// biome placement. All-zero when river extraction is disabled.
    pub flow: Grid<f32>,
    /// River mask per cell: `1` where a river runs, `0` elsewhere.
    pub rivers: Grid<u8>,
    /// Lake mask per cell: `1` where an inland depression holds standing
    /// water, `0` elsewhere. Rivers vanish beneath lakes and continue from
    /// their outlet. A basin whose floor lies below sea level but which is
    /// cut off from the sea counts as a lake, not ocean.
    pub lakes: Grid<u8>,
    /// Ocean mask per cell: `1` where the cell is at or below sea level
    /// *and* connected to the map border through other such cells. Always
    /// computed, including when river extraction is disabled, because it
    /// is a fact about the heightmap rather than a hydrology result.
    pub ocean: Grid<u8>,
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

    /// Raw upstream catchment at `(x, y)`, in cells.
    pub fn flow_accumulation_at(&self, x: usize, y: usize) -> f32 {
        self.flow_accumulation[(x, y)]
    }

    /// Whether a river runs through `(x, y)`.
    pub fn is_river(&self, x: usize, y: usize) -> bool {
        self.rivers[(x, y)] == 1
    }

    /// Whether `(x, y)` lies in a lake.
    pub fn is_lake(&self, x: usize, y: usize) -> bool {
        self.lakes[(x, y)] == 1
    }

    /// Whether `(x, y)` is ocean: at or below sea level and connected to
    /// the map border. Enclosed low ground is *not* ocean.
    pub fn is_ocean(&self, x: usize, y: usize) -> bool {
        self.ocean[(x, y)] == 1
    }

    /// Whether `(x, y)` is covered by water of any kind: ocean, lake or
    /// river.
    ///
    /// Note this is a union of the three masks, not an elevation test — a
    /// closed basin below sea level counts as lake, not ocean.
    pub fn is_water(&self, x: usize, y: usize) -> bool {
        self.is_ocean(x, y) || self.is_lake(x, y) || self.is_river(x, y)
    }

    /// Renders the terrain to a row-major RGB byte buffer
    /// (`width * height * 3`).
    pub fn to_rgb(&self, style: RenderStyle) -> Vec<u8> {
        render::to_rgb(self, style)
    }

    /// Writes a single [`Layer`](crate::Layer) as a PNG file: the shaded
    /// render, or one of the raw data layers (elevation and flow as 16-bit
    /// greyscale, masks as 8-bit black/white).
    ///
    /// Requires the `png` cargo feature (enabled by default).
    #[cfg(feature = "png")]
    pub fn save_layer_png<P: AsRef<std::path::Path>>(
        &self,
        layer: crate::Layer,
        path: P,
    ) -> Result<(), render::RenderError> {
        render::save_layer_png(self, layer, path)
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
/// and [`no_rivers`](Self::no_rivers). Basin breaching is opt-in via
/// [`breach_basins`](Self::breach_basins).
///
/// ```
/// use terrain_generator::Terrain;
///
/// let terrain = Terrain::builder().size(128, 128).seed(42).generate();
/// assert_eq!(terrain.heightmap.len(), 128 * 128);
/// ```
///
/// # Heightmap-only generation
///
/// Consumers that run their own hydrology should turn river extraction off.
/// This skips depression filling, flow routing and river extraction
/// entirely — it is the intended, supported path, not a degraded mode:
///
/// ```
/// use terrain_generator::Terrain;
///
/// let terrain = Terrain::builder().size(128, 128).seed(7).no_rivers().generate();
/// let elevation: &[f32] = terrain.heightmap.as_slice(); // row-major, [0, 1]
/// assert!(terrain.flow.iter().all(|f| *f == 0.0));      // not computed
/// # let _ = elevation;
/// ```
///
/// The [`ocean`](Terrain::ocean) mask is still computed in that mode (it is
/// a linear-time flood fill over the heightmap, not hydrology), so
/// "is this cell sea or an inland basin" remains answerable.
#[derive(Clone, Debug)]
pub struct TerrainBuilder {
    width: usize,
    height: usize,
    seed: u32,
    sea_level: f32,
    noise: NoiseParams,
    island: Option<IslandParams>,
    archipelago: Option<ArchipelagoParams>,
    erosion: Option<ErosionParams>,
    breach: Option<BreachParams>,
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
            archipelago: None,
            erosion: Some(ErosionParams::default()),
            breach: None,
            rivers: Some(RiverParams::default()),
        }
    }
}

impl TerrainBuilder {
    /// Sets the map dimensions in cells. Default: `512 x 512`.
    ///
    /// Both must be positive, and `width * height` at most
    /// [`MAX_CELLS`].
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

    /// Sets the sea level, in `[0, 1)`. Cells at or below it are ocean if
    /// they connect to the map border. Default: `0.08`.
    pub fn sea_level(mut self, sea_level: f32) -> Self {
        self.sea_level = sea_level;
        self
    }

    /// Sets the fractal noise parameters for the base heightmap.
    pub fn noise(mut self, noise: NoiseParams) -> Self {
        self.noise = noise;
        self
    }

    /// Enables single-island shaping with the given parameters (enabled
    /// with defaults unless [`no_island`](Self::no_island) or
    /// [`archipelago`](Self::archipelago) is called).
    pub fn island(mut self, island: IslandParams) -> Self {
        self.island = Some(island);
        self.archipelago = None;
        self
    }

    /// Shapes the land into several islands instead of one. Replaces
    /// single-island shaping.
    ///
    /// Erosion and hydrology are mask-agnostic and need no changes for
    /// this: each landmass gets its own drainage, and the sea between
    /// islands stays sea.
    ///
    /// ```
    /// use terrain_generator::{ArchipelagoParams, Terrain};
    ///
    /// let terrain = Terrain::builder()
    ///     .size(256, 256)
    ///     .seed(3)
    ///     .archipelago(ArchipelagoParams::with_islands(4))
    ///     .generate();
    /// # let _ = terrain;
    /// ```
    pub fn archipelago(mut self, archipelago: ArchipelagoParams) -> Self {
        self.archipelago = Some(archipelago);
        self.island = None;
        self
    }

    /// Disables land shaping entirely: terrain reaches the map borders.
    pub fn no_island(mut self) -> Self {
        self.island = None;
        self.archipelago = None;
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

    /// Enables basin breaching between erosion and hydrology: closed
    /// basins bigger than
    /// [`max_lake_fraction`](BreachParams::max_lake_fraction) get an outlet
    /// carved so they drain instead of becoming inland seas. Off by
    /// default.
    pub fn breach_basins(mut self, breach: BreachParams) -> Self {
        self.breach = Some(breach);
        self
    }

    /// Disables basin breaching (the default).
    pub fn no_breach(mut self) -> Self {
        self.breach = None;
        self
    }

    /// Enables river extraction with the given parameters (enabled with
    /// defaults unless [`no_rivers`](Self::no_rivers) is called).
    pub fn rivers(mut self, rivers: RiverParams) -> Self {
        self.rivers = Some(rivers);
        self
    }

    /// Disables flow routing, river and lake extraction: the `flow`,
    /// `flow_accumulation`, `rivers` and `lakes` layers come back all-zero
    /// and none of that work is done. The [`ocean`](Terrain::ocean) mask is
    /// still computed. See the type-level docs for the heightmap-only
    /// contract.
    pub fn no_rivers(mut self) -> Self {
        self.rivers = None;
        self
    }

    /// Runs the full pipeline: noise, island mask, erosion, optional basin
    /// breaching, then flow routing.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if any parameter is out of range:
    /// zero dimensions, a sea level outside `[0, 1)`, non-positive noise
    /// parameters, or erosion/river parameters outside the ranges
    /// documented on their fields. Use
    /// [`try_generate`](Self::try_generate) to get a [`ParamError`] instead
    /// — preferable when the parameters come from user input.
    pub fn generate(&self) -> Terrain {
        match self.try_generate() {
            Ok(terrain) => terrain,
            Err(e) => panic!("{e}"),
        }
    }

    /// Same as [`generate`](Self::generate) but reports invalid parameters
    /// as a [`ParamError`] instead of panicking.
    ///
    /// ```
    /// use terrain_generator::Terrain;
    ///
    /// let err = Terrain::builder().sea_level(1.5).try_generate().unwrap_err();
    /// assert_eq!(err.param, "sea level");
    /// ```
    pub fn try_generate(&self) -> Result<Terrain, ParamError> {
        self.try_generate_with_progress(|_, _| {})
    }

    /// Same as [`try_generate`](Self::try_generate), and reports how far the
    /// pipeline has got. `fraction` moves from `0.0` to `1.0` and never
    /// backwards. `stage` is a short label such as `"Erosion"` or `"Rivers"`.
    ///
    /// Erosion is almost all of the cost, so most of the fraction is spent
    /// there, updated about a hundred times. The callback is not required
    /// to be fast, but it should not do heavy work.
    pub fn try_generate_with_progress(
        &self,
        mut report: impl FnMut(f32, &'static str),
    ) -> Result<Terrain, ParamError> {
        self.validate()?;

        // Weights, not timings. Erosion dominates a normal map; the other
        // stages are short and only need a visible slice of the bar.
        let noise_w = 1.0;
        let erosion_w = if self.erosion.is_some() { 20.0 } else { 0.0 };
        let breach_w = if self.breach.is_some() { 2.0 } else { 0.0 };
        let river_w = if self.rivers.is_some() { 2.0 } else { 0.0 };
        let total = noise_w + erosion_w + breach_w + river_w;
        let mut cursor = 0.0f32;
        let mut take = |weight: f32| {
            let start = cursor;
            cursor += weight / total;
            (start, cursor)
        };

        let mask = match (&self.archipelago, &self.island) {
            (Some(arch), _) => Some(base::Mask::Archipelago(arch)),
            (None, Some(island)) => Some(base::Mask::Island(island)),
            (None, None) => None,
        };

        let (n0, n1) = take(noise_w);
        report(n0, "Heightmap");
        let mut heightmap = base::generate(self.width, self.height, self.seed, &self.noise, mask);
        report(n1, "Heightmap");

        if let Some(erosion_params) = &self.erosion {
            let (e0, e1) = take(erosion_w);
            erosion::erode_with_progress(
                &mut heightmap,
                erosion_params,
                self.seed,
                &mut |done, droplets| {
                    let t = if droplets == 0 {
                        1.0
                    } else {
                        done as f32 / droplets as f32
                    };
                    report(e0 + (e1 - e0) * t, "Erosion");
                },
            );
        }

        if let Some(breach_params) = &self.breach {
            let (b0, b1) = take(breach_w);
            report(b0, "Basins");
            // breaching reshapes the coastline as little as possible, but
            // it needs to know what is already sea
            let ocean = hydrology::ocean_mask(&heightmap, self.sea_level);
            breach::breach(&mut heightmap, &ocean, breach_params);
            report(b1, "Basins");
        }

        let ocean = hydrology::ocean_mask(&heightmap, self.sea_level);

        let (flow_accumulation, flow, rivers, lakes) = match &self.rivers {
            Some(river_params) => {
                let (r0, r1) = take(river_w);
                report(r0, "Rivers");
                let hydro = hydrology::compute(&heightmap, &ocean, river_params);
                report(r1, "Rivers");
                (hydro.accumulation, hydro.flow, hydro.rivers, hydro.lakes)
            }
            None => (
                Grid::new(self.width, self.height, 0.0),
                Grid::new(self.width, self.height, 0.0),
                Grid::new(self.width, self.height, 0),
                Grid::new(self.width, self.height, 0),
            ),
        };

        report(1.0, "Done");

        Ok(Terrain {
            sea_level: self.sea_level,
            heightmap,
            flow_accumulation,
            flow,
            rivers,
            lakes,
            ocean,
        })
    }

    fn validate(&self) -> Result<(), ParamError> {
        check(
            self.width > 0 && self.height > 0,
            "width and height",
            "must be positive",
        )?;
        // Hydrology and basin breaching index cells with u32, and breaching
        // keeps u32::MAX free to mean "no cell". A bigger map would wrap
        // those indices, and in a release build `width * height` itself can
        // wrap, so either would panic mid-generation instead of failing
        // here. The limit allows 65535 x 65535, 16 GiB per float layer.
        check(
            self.width
                .checked_mul(self.height)
                .is_some_and(|cells| cells <= MAX_CELLS),
            "width and height",
            "must give at most 4294967295 cells (2^32 - 1)",
        )?;
        check(
            (0.0..1.0).contains(&self.sea_level),
            "sea level",
            "must be in range [0,1)",
        )?;
        self.noise.validate()?;
        if let Some(island) = &self.island {
            island.validate()?;
        }
        if let Some(archipelago) = &self.archipelago {
            archipelago.validate()?;
        }
        if let Some(erosion) = &self.erosion {
            erosion.validate()?;
        }
        if let Some(breach) = &self.breach {
            breach.validate()?;
        }
        if let Some(rivers) = &self.rivers {
            rivers.validate()?;
        }
        Ok(())
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
        assert_eq!(a.flow_accumulation, b.flow_accumulation);
        assert_eq!(a.ocean, b.ocean);
        assert_eq!(a.lakes, b.lakes);
    }

    #[test]
    fn different_seeds_differ() {
        let a = Terrain::builder().size(64, 64).seed(1).generate();
        let b = Terrain::builder().size(64, 64).seed(2).generate();
        assert_ne!(a.heightmap, b.heightmap);
    }

    #[test]
    fn rivers_are_not_on_water() {
        let t = Terrain::builder()
            .size(128, 128)
            .seed(3)
            .rivers(RiverParams::density(0.05))
            .generate();
        for y in 0..t.height() {
            for x in 0..t.width() {
                if t.is_river(x, y) {
                    assert!(!t.is_ocean(x, y) && !t.is_lake(x, y));
                }
            }
        }
    }

    #[test]
    fn lakes_are_never_ocean() {
        let t = Terrain::builder().size(128, 128).seed(3).generate();
        for y in 0..t.height() {
            for x in 0..t.width() {
                if t.is_lake(x, y) {
                    assert!(!t.is_ocean(x, y));
                }
            }
        }
    }

    #[test]
    fn ocean_reaches_the_border_but_not_the_peaks() {
        let t = Terrain::builder().size(128, 128).seed(3).generate();
        assert!(t.is_ocean(0, 0), "map corner should be sea");
        // the highest cell is certainly not ocean
        let (mut best, mut best_v) = ((0, 0), -1.0f32);
        for y in 0..t.height() {
            for x in 0..t.width() {
                if t.height_at(x, y) > best_v {
                    best_v = t.height_at(x, y);
                    best = (x, y);
                }
            }
        }
        assert!(!t.is_ocean(best.0, best.1));
    }

    #[test]
    fn heightmap_only_mode_skips_hydrology() {
        let t = Terrain::builder()
            .size(64, 64)
            .seed(5)
            .no_rivers()
            .generate();
        assert!(t.flow.iter().all(|v| *v == 0.0));
        assert!(t.flow_accumulation.iter().all(|v| *v == 0.0));
        assert!(t.rivers.iter().all(|v| *v == 0));
        assert!(t.lakes.iter().all(|v| *v == 0));
        // ocean is still available
        assert!(t.ocean.iter().any(|v| *v == 1));
        // and the heightmap is the same as with hydrology on
        let full = Terrain::builder().size(64, 64).seed(5).generate();
        assert_eq!(t.heightmap, full.heightmap);
    }

    #[test]
    fn raw_accumulation_backs_the_normalised_flow() {
        let t = Terrain::builder().size(96, 96).seed(8).generate();
        assert!(t.flow_accumulation.iter().all(|a| *a >= 1.0));
        // both orderings agree: normalisation is monotonic
        let idx_max_acc = (0..t.flow_accumulation.len())
            .max_by(|a, b| {
                t.flow_accumulation.as_slice()[*a].total_cmp(&t.flow_accumulation.as_slice()[*b])
            })
            .unwrap();
        let idx_max_flow = (0..t.flow.len())
            .max_by(|a, b| t.flow.as_slice()[*a].total_cmp(&t.flow.as_slice()[*b]))
            .unwrap();
        assert_eq!(idx_max_acc, idx_max_flow);
    }

    #[test]
    fn output_stays_in_unit_range() {
        let t = Terrain::builder().size(64, 64).seed(4).generate();
        assert!(t.heightmap.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(t.flow.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn try_generate_reports_errors_instead_of_panicking() {
        let err = Terrain::builder()
            .sea_level(1.5)
            .try_generate()
            .unwrap_err();
        assert_eq!(err.param, "sea level");
        assert!(err.to_string().contains("[0,1)"));

        let err = Terrain::builder().size(0, 8).try_generate().unwrap_err();
        assert_eq!(err.param, "width and height");

        let err = Terrain::builder()
            .size(16, 16)
            .rivers(RiverParams::density(2.0))
            .try_generate()
            .unwrap_err();
        assert_eq!(err.param, "river density");

        assert!(Terrain::builder().size(16, 16).try_generate().is_ok());

        // rejected before anything is allocated, so these cost nothing
        let err = Terrain::builder()
            .size(65536, 65536)
            .try_generate()
            .unwrap_err();
        assert_eq!(err.param, "width and height");
        let err = Terrain::builder()
            .size(usize::MAX, 2)
            .try_generate()
            .unwrap_err();
        assert_eq!(err.param, "width and height");
    }

    #[test]
    fn progress_advances_to_the_end() {
        let mut last = -1.0f32;
        let mut saw_erosion = false;
        let terrain = Terrain::builder()
            .size(32, 32)
            .erosion(ErosionParams {
                droplets: Some(300),
                ..Default::default()
            })
            .try_generate_with_progress(|fraction, stage| {
                assert!(
                    (0.0..=1.0).contains(&fraction),
                    "fraction {fraction} out of range"
                );
                assert!(
                    fraction + 1e-4 >= last,
                    "progress went backwards from {last} to {fraction}"
                );
                last = fraction;
                if stage == "Erosion" {
                    saw_erosion = true;
                }
            })
            .unwrap();
        assert!(saw_erosion);
        assert!((last - 1.0).abs() < 1e-4);
        assert_eq!(terrain.width(), 32);
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
