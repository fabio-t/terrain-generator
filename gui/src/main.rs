//! Desktop preview for the terrain generator.
//!
//! Sliders edit the same parameters as the CLI. A map is built only when
//! Generate is clicked. The work runs on a background thread because
//! hydraulic erosion is too slow to do on the UI thread.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use terrain_generator::{
    ArchipelagoParams, BreachParams, ErosionParams, IslandParams, Layer, NoiseParams, RenderStyle,
    RiverParams, Terrain, TerrainBuilder,
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([880.0, 560.0])
            .with_title("Terrain Generator"),
        ..Default::default()
    };
    eframe::run_native(
        "Terrain Generator",
        options,
        Box::new(|cc| Ok(Box::new(TerrainApp::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Land {
    Island,
    Archipelago,
    Borderless,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RiverMode {
    Density,
    MinUpstream,
    PerEdge,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Shaded,
    Greyscale,
}

impl View {
    fn style(self) -> RenderStyle {
        match self {
            View::Shaded => RenderStyle::Shaded,
            View::Greyscale => RenderStyle::Greyscale,
        }
    }

    fn label(self) -> &'static str {
        match self {
            View::Shaded => "Shaded",
            View::Greyscale => "Heightmap",
        }
    }
}

#[derive(Clone, PartialEq)]
struct GenParams {
    width: usize,
    height: usize,
    seed: u32,
    sea_level: f32,
    octaves: usize,
    frequency: f64,
    lacunarity: f64,
    persistence: f64,
    land: Land,
    island_size: f32,
    coast_noise: f32,
    islands: usize,
    islands_size: f32,
    islands_variation: f32,
    islands_spread: f32,
    islands_spacing: f32,
    erosion: bool,
    droplets_auto: bool,
    droplets: usize,
    breach: bool,
    breach_fraction: f32,
    rivers: bool,
    river_mode: RiverMode,
    river_density: f32,
    river_min_upstream: f32,
    river_per_edge: f32,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            seed: 0,
            sea_level: 0.08,
            octaves: 8,
            frequency: 2.5,
            lacunarity: 2.0,
            persistence: 0.5,
            land: Land::Island,
            island_size: 1.0,
            coast_noise: 1.0,
            islands: 5,
            islands_size: 0.35,
            islands_variation: 0.35,
            islands_spread: 0.85,
            islands_spacing: 0.22,
            erosion: true,
            droplets_auto: true,
            droplets: 262_144,
            breach: false,
            breach_fraction: 0.005,
            rivers: true,
            river_mode: RiverMode::Density,
            river_density: 0.01,
            river_min_upstream: 400.0,
            river_per_edge: 4.0,
        }
    }
}

impl GenParams {
    fn builder(&self) -> TerrainBuilder {
        let mut builder = Terrain::builder()
            .size(self.width, self.height)
            .seed(self.seed)
            .sea_level(self.sea_level)
            .noise(NoiseParams {
                octaves: self.octaves,
                frequency: self.frequency,
                lacunarity: self.lacunarity,
                persistence: self.persistence,
            });

        builder = match self.land {
            Land::Island => builder.island(IslandParams {
                size: self.island_size,
                coast_noise: self.coast_noise,
            }),
            Land::Archipelago => builder.archipelago(ArchipelagoParams {
                islands: self.islands,
                size: self.islands_size,
                size_variation: self.islands_variation,
                spread: self.islands_spread,
                min_spacing: self.islands_spacing,
                coast_noise: self.coast_noise,
            }),
            Land::Borderless => builder.no_island(),
        };

        builder = if self.erosion {
            builder.erosion(ErosionParams {
                droplets: if self.droplets_auto {
                    None
                } else {
                    Some(self.droplets)
                },
                ..Default::default()
            })
        } else {
            builder.no_erosion()
        };

        if self.breach {
            builder = builder.breach_basins(BreachParams {
                max_lake_fraction: self.breach_fraction,
            });
        }

        if self.rivers {
            builder = match self.river_mode {
                RiverMode::Density => builder.rivers(RiverParams::density(self.river_density)),
                RiverMode::MinUpstream => {
                    builder.rivers(RiverParams::min_upstream_cells(self.river_min_upstream))
                }
                RiverMode::PerEdge => builder.rivers(RiverParams::per_edge(self.river_per_edge)),
            };
        } else {
            builder = builder.no_rivers();
        }

        builder
    }
}

struct Job {
    id: u64,
    params: GenParams,
    view: View,
}

struct Ready {
    id: u64,
    params: GenParams,
    view: View,
    terrain: Terrain,
    rgb: Vec<u8>,
    elapsed: Duration,
}

#[derive(Clone, Copy)]
struct GenProgress {
    fraction: f32,
    stage: &'static str,
}

enum WorkerMsg {
    Progress {
        id: u64,
        fraction: f32,
        stage: &'static str,
    },
    // boxed: a finished map is ~400 bytes against ~30 for a progress tick,
    // and every progress message would otherwise be that size too
    Ready(Box<Ready>),
    Failed {
        id: u64,
        error: String,
    },
}

fn worker(jobs: Receiver<Job>, out: Sender<WorkerMsg>) {
    while let Ok(first) = jobs.recv() {
        let mut job = first;
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let started = Instant::now();
        let progress_tx = out.clone();
        let id = job.id;
        // Leave the last sliver for shading the image, so the bar does not
        // read 100% while the preview is still being drawn.
        let generated = job
            .params
            .builder()
            .try_generate_with_progress(|fraction, stage| {
                let _ = progress_tx.send(WorkerMsg::Progress {
                    id,
                    fraction: fraction * 0.94,
                    stage,
                });
            });
        let msg = match generated {
            Ok(terrain) => {
                let _ = progress_tx.send(WorkerMsg::Progress {
                    id,
                    fraction: 0.97,
                    stage: "Rendering",
                });
                let rgb = terrain.to_rgb(job.view.style());
                WorkerMsg::Ready(Box::new(Ready {
                    id: job.id,
                    params: job.params,
                    view: job.view,
                    terrain,
                    rgb,
                    elapsed: started.elapsed(),
                }))
            }
            Err(err) => WorkerMsg::Failed {
                id: job.id,
                error: err.to_string(),
            },
        };
        if out.send(msg).is_err() {
            break;
        }
    }
}

struct TerrainApp {
    params: GenParams,
    view: View,
    link_square: bool,
    force_now: bool,
    jobs: Sender<Job>,
    results: Receiver<WorkerMsg>,
    next_id: u64,
    /// Parameters of the job most recently sent. `None` when nothing is running.
    submitted: Option<GenParams>,
    started: Option<Instant>,
    shown: Option<GenParams>,
    shown_elapsed: Duration,
    rendered_view: View,
    terrain: Option<Terrain>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
    notice: String,
    readout: Option<String>,
    progress: Option<GenProgress>,
}

impl TerrainApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let (jobs, job_rx) = mpsc::channel();
        let (result_tx, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("terrain-generate".into())
            .spawn(move || worker(job_rx, result_tx))
            .expect("spawn terrain generator thread");
        Self {
            params: GenParams::default(),
            view: View::Shaded,
            link_square: true,
            force_now: false,
            jobs,
            results,
            next_id: 0,
            submitted: None,
            started: None,
            shown: None,
            shown_elapsed: Duration::ZERO,
            rendered_view: View::Shaded,
            terrain: None,
            texture: None,
            error: None,
            notice: String::new(),
            readout: None,
            progress: None,
        }
    }

    fn submit(&mut self) {
        self.next_id += 1;
        let job = Job {
            id: self.next_id,
            params: self.params.clone(),
            view: self.view,
        };
        self.submitted = Some(job.params.clone());
        self.started = Some(Instant::now());
        self.error = None;
        if self.progress.is_none() {
            self.progress = Some(GenProgress {
                fraction: 0.0,
                stage: "Starting",
            });
        }
        let _ = self.jobs.send(job);
    }

    fn kick(&mut self) {
        if !self.force_now {
            return;
        }
        self.force_now = false;
        if self.submitted.as_ref() == Some(&self.params) {
            return;
        }
        self.submit();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.results.try_recv() {
            match msg {
                WorkerMsg::Progress {
                    id,
                    fraction,
                    stage,
                } => {
                    // a superseded job keeps reporting until the worker
                    // reaches the newer one; showing its progress would make
                    // the bar run ahead and then jump back to zero
                    if id == self.next_id {
                        self.progress = Some(GenProgress { fraction, stage });
                    }
                }
                WorkerMsg::Ready(ready) => {
                    if ready.id != self.next_id {
                        continue;
                    }
                    self.install(ctx, *ready);
                }
                WorkerMsg::Failed { id, error } => {
                    if id != self.next_id {
                        continue;
                    }
                    self.submitted = None;
                    self.started = None;
                    self.progress = None;
                    self.error = Some(error);
                }
            }
        }
    }

    fn install(&mut self, ctx: &egui::Context, ready: Ready) {
        let rgb = if ready.view == self.view {
            ready.rgb
        } else {
            ready.terrain.to_rgb(self.view.style())
        };
        let width = ready.terrain.width();
        let height = ready.terrain.height();
        self.upload(ctx, width, height, &rgb);
        self.rendered_view = self.view;
        self.shown_elapsed = ready.elapsed;
        self.shown = Some(ready.params);
        self.terrain = Some(ready.terrain);
        self.submitted = None;
        self.started = None;
        self.progress = None;
        self.error = None;
    }

    fn sync_view(&mut self, ctx: &egui::Context) {
        if self.rendered_view == self.view {
            return;
        }
        let Some(terrain) = &self.terrain else {
            return;
        };
        let rgb = terrain.to_rgb(self.view.style());
        let (width, height) = (terrain.width(), terrain.height());
        self.upload(ctx, width, height, &rgb);
        self.rendered_view = self.view;
    }

    fn upload(&mut self, ctx: &egui::Context, width: usize, height: usize, rgb: &[u8]) {
        let image = egui::ColorImage::from_rgb([width, height], rgb);
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture("terrain", image, egui::TextureOptions::LINEAR));
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_sized([110.0, 28.0], egui::Button::new("Generate"))
                .clicked()
            {
                self.force_now = true;
            }
            if ui
                .add_sized([72.0, 28.0], egui::Button::new("Reset"))
                .clicked()
            {
                self.params = GenParams::default();
                self.view = View::Shaded;
                self.link_square = true;
            }
        });
        ui.label(
            egui::RichText::new("Settings apply when you click Generate.")
                .small()
                .weak(),
        );

        self.size_and_seed(ui);
        self.shape(ui);
        self.noise(ui);
        self.erosion(ui);
        self.water(ui);
        self.view_and_export(ui);
    }

    fn size_and_seed(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Map").strong());
        ui.horizontal(|ui| {
            for size in [256, 512, 1024, 2048] {
                if ui
                    .selectable_label(
                        self.params.width == size && self.params.height == size,
                        format!("{size}"),
                    )
                    .clicked()
                {
                    self.params.width = size;
                    self.params.height = size;
                }
            }
        });

        let before_w = self.params.width;
        let before_h = self.params.height;
        egui::Grid::new("size")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Width");
                ui.add(
                    egui::DragValue::new(&mut self.params.width)
                        .range(32..=4096)
                        .speed(8.0),
                );
                ui.end_row();
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut self.params.height)
                        .range(32..=4096)
                        .speed(8.0),
                );
                ui.end_row();
            });
        if self.link_square && self.params.width != before_w {
            self.params.height = self.params.width;
        } else if self.link_square && self.params.height != before_h {
            self.params.width = self.params.height;
        }
        ui.checkbox(&mut self.link_square, "Square map");

        ui.horizontal(|ui| {
            ui.label("Seed");
            ui.add(
                egui::DragValue::new(&mut self.params.seed)
                    .range(0..=u32::MAX)
                    .speed(1.0),
            );
            if ui.button("−").clicked() {
                self.params.seed = self.params.seed.saturating_sub(1);
            }
            if ui.button("+").clicked() {
                self.params.seed = self.params.seed.saturating_add(1);
            }
            if ui.button("Random").clicked() {
                self.params.seed = random_seed(self.params.seed);
            }
        });
        ui.separator();
    }

    fn shape(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Shape").strong());
        slider(ui, "Sea level", &mut self.params.sea_level, 0.0..=0.99, 2);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.params.land, Land::Island, "Island");
            ui.selectable_value(&mut self.params.land, Land::Archipelago, "Archipelago");
            ui.selectable_value(&mut self.params.land, Land::Borderless, "Borderless");
        });
        match self.params.land {
            Land::Island => {
                slider(
                    ui,
                    "Island size",
                    &mut self.params.island_size,
                    0.15..=1.2,
                    2,
                );
                slider(
                    ui,
                    "Coast noise",
                    &mut self.params.coast_noise,
                    0.0..=2.0,
                    2,
                );
            }
            Land::Archipelago => {
                ui.horizontal(|ui| {
                    ui.label("Islands");
                    ui.add(egui::DragValue::new(&mut self.params.islands).range(1..=24));
                });
                slider(
                    ui,
                    "Island size",
                    &mut self.params.islands_size,
                    0.08..=0.7,
                    2,
                );
                slider(
                    ui,
                    "Size variation",
                    &mut self.params.islands_variation,
                    0.0..=0.95,
                    2,
                );
                slider(ui, "Spread", &mut self.params.islands_spread, 0.0..=1.0, 2);
                slider(
                    ui,
                    "Spacing",
                    &mut self.params.islands_spacing,
                    0.0..=0.6,
                    2,
                );
                slider(
                    ui,
                    "Coast noise",
                    &mut self.params.coast_noise,
                    0.0..=2.0,
                    2,
                );
            }
            Land::Borderless => {
                ui.label(
                    egui::RichText::new("Terrain runs to the edges of the map.")
                        .small()
                        .weak(),
                );
            }
        }
        ui.separator();
    }

    fn noise(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Noise").strong());
        slider_f64(ui, "Frequency", &mut self.params.frequency, 0.5..=8.0, 2);
        ui.horizontal(|ui| {
            ui.label("Octaves");
            ui.add(egui::Slider::new(&mut self.params.octaves, 1..=16));
        });
        egui::CollapsingHeader::new("Detail")
            .id_salt("noise-detail")
            .show(ui, |ui| {
                slider_f64(ui, "Lacunarity", &mut self.params.lacunarity, 1.2..=3.5, 2);
                slider_f64(
                    ui,
                    "Persistence",
                    &mut self.params.persistence,
                    0.05..=0.95,
                    2,
                );
            });
        ui.separator();
    }

    fn erosion(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Erosion").strong());
        ui.checkbox(&mut self.params.erosion, "Hydraulic erosion");
        if self.params.erosion {
            ui.checkbox(&mut self.params.droplets_auto, "One droplet per cell");
            if !self.params.droplets_auto {
                ui.add(
                    egui::Slider::new(&mut self.params.droplets, 1_000..=4_000_000)
                        .logarithmic(true)
                        .text("droplets"),
                );
            }
        }
        ui.separator();
    }

    fn water(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Water").strong());
        ui.checkbox(&mut self.params.rivers, "Rivers and lakes");
        if self.params.rivers {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.params.river_mode, RiverMode::Density, "Density");
                ui.selectable_value(&mut self.params.river_mode, RiverMode::MinUpstream, "Cells");
                ui.selectable_value(&mut self.params.river_mode, RiverMode::PerEdge, "Per edge");
            });
            match self.params.river_mode {
                RiverMode::Density => {
                    slider(
                        ui,
                        "River cover",
                        &mut self.params.river_density,
                        0.0..=0.08,
                        3,
                    );
                }
                RiverMode::MinUpstream => {
                    ui.add(
                        egui::Slider::new(&mut self.params.river_min_upstream, 1.0..=50_000.0)
                            .logarithmic(true)
                            .text("min cells"),
                    );
                }
                RiverMode::PerEdge => {
                    ui.add(
                        egui::Slider::new(&mut self.params.river_per_edge, 0.5..=64.0)
                            .logarithmic(true)
                            .text("× edge"),
                    );
                }
            }
        }
        ui.checkbox(&mut self.params.breach, "Drain oversized basins");
        if self.params.breach {
            ui.add(
                egui::Slider::new(&mut self.params.breach_fraction, 0.0005..=0.05)
                    .logarithmic(true)
                    .text("max lake"),
            );
        }
        ui.separator();
    }

    fn view_and_export(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("View").strong());
        ui.horizontal(|ui| {
            for view in [View::Shaded, View::Greyscale] {
                ui.selectable_value(&mut self.view, view, view.label());
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Export").strong());
        let has_map = self.terrain.is_some();
        if ui
            .add_enabled(has_map, egui::Button::new("Save picture…"))
            .clicked()
        {
            self.save_picture();
        }
        if ui
            .add_enabled(has_map, egui::Button::new("Save 16-bit heightmap…"))
            .clicked()
        {
            self.save_height();
        }
        if ui
            .add_enabled(has_map, egui::Button::new("Save all layers…"))
            .clicked()
        {
            self.save_layers();
        }
        ui.label(
            egui::RichText::new(
                "All layers writes shaded, height, flow, rivers, lakes, and ocean.",
            )
            .small()
            .weak(),
        );
        if !self.notice.is_empty() {
            ui.label(egui::RichText::new(&self.notice).small());
        }

        ui.add_space(6.0);
        egui::CollapsingHeader::new("CLI command")
            .id_salt("cli")
            .show(ui, |ui| {
                let mut command = cli_line(&self.params, self.view);
                ui.add(
                    egui::TextEdit::multiline(&mut command)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .interactive(false),
                );
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(cli_line(&self.params, self.view));
                    self.notice = "Copied the CLI command.".into();
                }
            });
    }

    fn save_picture(&mut self) {
        let seed = self.seed();
        let Some(path) = save_dialog("Save picture", &format!("map{seed}.png")) else {
            return;
        };
        let Some(terrain) = &self.terrain else {
            return;
        };
        self.notice = match terrain.save_png(&path, self.view.style()) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(err) => format!("Could not save picture: {err}"),
        };
    }

    fn save_height(&mut self) {
        let seed = self.seed();
        let Some(path) = save_dialog("Save 16-bit heightmap", &format!("map{seed}_height.png"))
        else {
            return;
        };
        let Some(terrain) = &self.terrain else {
            return;
        };
        self.notice = match terrain.save_layer_png(Layer::Height, &path) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(err) => format!("Could not save heightmap: {err}"),
        };
    }

    fn save_layers(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Choose a folder for the layers")
            .pick_folder()
        else {
            return;
        };
        let Some(terrain) = &self.terrain else {
            return;
        };
        let seed = self.seed();
        for layer in Layer::ALL {
            let path = dir.join(format!("map{seed}_{}.png", layer.name()));
            if let Err(err) = terrain.save_layer_png(layer, &path) {
                self.notice = format!("Could not save {}: {err}", path.display());
                return;
            }
        }
        self.notice = format!("Saved {} layers to {}", Layer::ALL.len(), dir.display());
    }

    fn seed(&self) -> u32 {
        self.shown
            .as_ref()
            .map(|p| p.seed)
            .unwrap_or(self.params.seed)
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let available = ui.max_rect();
        let bar_height = if self.progress.is_some() { 28.0 } else { 0.0 };
        let status_height = 28.0 + bar_height;
        let image_rect = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right(), available.bottom() - status_height),
        );
        let status_rect = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - status_height),
            available.max,
        );

        if let (Some(texture), Some(terrain)) = (&self.texture, &self.terrain) {
            let size = fit_image(image_rect.size(), terrain.width(), terrain.height());
            let rect = egui::Rect::from_center_size(image_rect.center(), size);
            let response = ui.put(
                rect,
                egui::Image::from_texture(texture)
                    .fit_to_exact_size(size)
                    .sense(egui::Sense::hover()),
            );
            if let Some(pointer) = response.hover_pos() {
                self.readout = cell_readout(terrain, response.rect, pointer);
            }
        } else {
            let message = if self.submitted.is_some() {
                "Generating…"
            } else {
                "Click Generate to build a map."
            };
            ui.scope_builder(egui::UiBuilder::new().max_rect(image_rect), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(message);
                });
            });
        }

        let (text, color) = self.status_line();
        let readout = self.readout.clone();
        let progress = self.progress;
        let started = self.started;
        ui.scope_builder(egui::UiBuilder::new().max_rect(status_rect), |ui| {
            if let Some(progress) = progress {
                progress_bar(ui, progress, started);
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(text).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(readout) = &readout {
                        ui.label(readout);
                    }
                });
            });
        });
    }

    fn status_line(&self) -> (String, egui::Color32) {
        if let Some(started) = self.started {
            return (
                format!("Generating… {:.1}s", started.elapsed().as_secs_f32()),
                egui::Color32::from_rgb(140, 180, 230),
            );
        }
        if let Some(error) = &self.error {
            return (error.clone(), egui::Color32::from_rgb(230, 120, 120));
        }
        if let Some(params) = &self.shown {
            let pending = if params != &self.params {
                " · click Generate to apply changes"
            } else {
                ""
            };
            (
                format!(
                    "{}×{}  seed {}  {:.2}s{pending}",
                    params.width,
                    params.height,
                    params.seed,
                    self.shown_elapsed.as_secs_f32()
                ),
                egui::Color32::from_rgb(170, 170, 170),
            )
        } else {
            ("Click Generate".into(), egui::Color32::GRAY)
        }
    }
}

impl eframe::App for TerrainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.readout = None;
        self.poll(ctx);
        self.sync_view(ctx);

        egui::SidePanel::left("controls")
            .resizable(true)
            .default_width(340.0)
            .width_range(300.0..=460.0)
            .show(ctx, |ui| {
                ui.heading("Terrain");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.controls(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.preview(ui);
        });

        self.kick();
        if self.submitted.is_some() || self.view != self.rendered_view {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }
}

fn progress_bar(ui: &mut egui::Ui, progress: GenProgress, started: Option<Instant>) {
    let elapsed = started.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
    let percent = (progress.fraction.clamp(0.0, 1.0) * 100.0).round();
    ui.add(
        egui::ProgressBar::new(progress.fraction)
            .text(format!(
                "{}   {percent:.0}%   {elapsed:.1}s",
                progress.stage
            ))
            .desired_height(20.0)
            .fill(egui::Color32::from_rgb(64, 128, 196)),
    );
}

fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) {
    ui.add(
        egui::Slider::new(value, range)
            .text(label)
            .fixed_decimals(decimals),
    );
}

fn slider_f64(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    decimals: usize,
) {
    ui.add(
        egui::Slider::new(value, range)
            .text(label)
            .fixed_decimals(decimals),
    );
}

fn fit_image(available: egui::Vec2, width: usize, height: usize) -> egui::Vec2 {
    let aspect = width as f32 / height as f32;
    if available.x / available.y > aspect {
        egui::vec2(available.y * aspect, available.y)
    } else {
        egui::vec2(available.x, available.x / aspect)
    }
}

fn cell_readout(terrain: &Terrain, rect: egui::Rect, pointer: egui::Pos2) -> Option<String> {
    if !rect.contains(pointer) || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let u = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 0.999);
    let v = ((pointer.y - rect.top()) / rect.height()).clamp(0.0, 0.999);
    let x = (u * terrain.width() as f32) as usize;
    let y = (v * terrain.height() as f32) as usize;
    let kind = if terrain.is_river(x, y) {
        "river"
    } else if terrain.is_lake(x, y) {
        "lake"
    } else if terrain.is_ocean(x, y) {
        "ocean"
    } else {
        "land"
    };
    Some(format!(
        "{x}, {y}   height {:.3}   {kind}",
        terrain.height_at(x, y)
    ))
}

fn save_dialog(title: &str, file_name: &str) -> Option<PathBuf> {
    let path = rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(file_name)
        .add_filter("PNG image", &["png"])
        .save_file()?;
    Some(ensure_png(path))
}

fn ensure_png(path: PathBuf) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("png") => path,
        _ => path.with_extension("png"),
    }
}

fn random_seed(current: u32) -> u32 {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(1);
    let mut x = current ^ tick ^ 0xA5A5_1234;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

fn cli_line(params: &GenParams, view: View) -> String {
    let mut args = vec![
        "terrain-generator".to_string(),
        format!("-W {}", params.width),
        format!("-H {}", params.height),
        format!("-s {}", params.seed),
        format!("--sea-level {}", trim_f32(params.sea_level)),
        format!("--octaves {}", params.octaves),
        format!("--frequency {}", trim_f64(params.frequency)),
        format!("--lacunarity {}", trim_f64(params.lacunarity)),
        format!("--persistence {}", trim_f64(params.persistence)),
    ];
    match params.land {
        Land::Borderless => args.push("--no-island".into()),
        Land::Island => {
            args.push(format!("--island {}", trim_f32(params.island_size)));
            args.push(format!("--coast-noise {}", trim_f32(params.coast_noise)));
        }
        Land::Archipelago => {
            args.push(format!("--archipelago {}", params.islands));
            args.push(format!("--islands-size {}", trim_f32(params.islands_size)));
            args.push(format!(
                "--islands-variation {}",
                trim_f32(params.islands_variation)
            ));
            args.push(format!(
                "--islands-spread {}",
                trim_f32(params.islands_spread)
            ));
            args.push(format!(
                "--islands-spacing {}",
                trim_f32(params.islands_spacing)
            ));
            args.push(format!("--coast-noise {}", trim_f32(params.coast_noise)));
        }
    }
    if params.erosion {
        if !params.droplets_auto {
            args.push(format!("--droplets {}", params.droplets));
        }
    } else {
        args.push("--no-erosion".into());
    }
    if params.breach {
        args.push(format!(
            "--breach-basins {}",
            trim_f32(params.breach_fraction)
        ));
    }
    if params.rivers {
        match params.river_mode {
            RiverMode::Density => args.push(format!("--rivers {}", trim_f32(params.river_density))),
            RiverMode::MinUpstream => args.push(format!(
                "--river-min-upstream {}",
                trim_f32(params.river_min_upstream)
            )),
            RiverMode::PerEdge => args.push(format!(
                "--river-per-edge {}",
                trim_f32(params.river_per_edge)
            )),
        }
    } else {
        args.push("--no-rivers".into());
    }
    if view == View::Greyscale {
        args.push("--grey".into());
    }
    args.join(" ")
}

fn trim_f32(value: f32) -> String {
    trim_numeric(&format!("{value:.4}"))
}

fn trim_f64(value: f64) -> String {
    trim_numeric(&format!("{value:.4}"))
}

fn trim_numeric(text: &str) -> String {
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_drive_the_generator() {
        let mut params = GenParams::default();
        params.width = 32;
        params.height = 24;
        params.erosion = false;
        params.rivers = false;

        let island = params.builder().try_generate().unwrap();
        assert_eq!((island.width(), island.height()), (32, 24));

        params.land = Land::Borderless;
        assert!(params.builder().try_generate().is_ok());

        params.land = Land::Archipelago;
        params.islands = 3;
        params.erosion = true;
        params.droplets_auto = false;
        params.droplets = 200;
        params.rivers = true;
        params.breach = true;
        let eroded = params.builder().try_generate().unwrap();
        assert_eq!(eroded.to_rgb(RenderStyle::Shaded).len(), 32 * 24 * 3);
    }

    #[test]
    fn cli_command_uses_the_real_flags() {
        let command = cli_line(&GenParams::default(), View::Shaded);
        assert!(command.contains("-W 512"));
        assert!(command.contains("-H 512"));
        assert!(command.contains("--sea-level 0.08"));
        assert!(command.contains("--island 1"));
        assert!(!command.contains("--grey"));
        assert!(!command.contains("--no-erosion"));

        let mut params = GenParams::default();
        params.land = Land::Borderless;
        params.erosion = false;
        params.rivers = false;
        let command = cli_line(&params, View::Greyscale);
        assert!(command.contains("--no-island"));
        assert!(command.contains("--no-erosion"));
        assert!(command.contains("--no-rivers"));
        assert!(command.contains("--grey"));
    }
}
