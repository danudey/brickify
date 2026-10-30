use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, Rect, Sense, Stroke, TextureHandle, pos2, vec2};

use crate::geometry::{self, Tri};
use crate::mosaic::{Layout, Mosaic, PartKind, Settings};
use crate::palette::{self, COLOURS};
use crate::source::{self, SourceImage};
use crate::{export, platform, view3d};

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Source,
    Bricks,
    Model3d,
}

/// Background of the image, brick and 3D views.
pub const VIEW_BG: Color32 = Color32::from_gray(40);
/// Text colours, chosen for at least 7:1 contrast against `VIEW_BG` and the side panel.
const TEXT: Color32 = Color32::from_gray(225);
const WEAK_TEXT: Color32 = Color32::from_gray(190);

struct Loaded {
    name: String,
    image: SourceImage,
    texture: TextureHandle,
}

pub struct BrickApp {
    loaded: Option<Loaded>,
    settings: Settings,
    built_with: Option<Settings>,
    mosaic: Option<Mosaic>,
    /// From [`Mosaic::connectivity`]: (groups, islands, loose part flags).
    connectivity: (usize, usize, Vec<bool>),
    tris: Vec<Tri>,
    studs: bool,
    tris_have_studs: bool,
    tab: Tab,
    camera: view3d::Camera,
    status: String,
    /// Files picked asynchronously (web) or dropped onto the window arrive here.
    incoming: Arc<Mutex<Option<(String, Vec<u8>)>>>,
}

impl BrickApp {
    pub fn new(ctx: &egui::Context, initial: Option<(String, Vec<u8>)>, settings: Settings) -> Self {
        // The views paint dark backgrounds, so always use the dark theme (not the system's),
        // with brighter text than egui's default for comfortable contrast.
        ctx.set_theme(egui::Theme::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.fg_stroke.color = TEXT;
        visuals.widgets.inactive.fg_stroke.color = TEXT;
        visuals.weak_text_color = Some(WEAK_TEXT);
        ctx.set_visuals_of(egui::Theme::Dark, visuals);
        Self {
            loaded: None,
            settings,
            built_with: None,
            mosaic: None,
            connectivity: (0, 0, Vec::new()),
            tris: Vec::new(),
            studs: true,
            tris_have_studs: true,
            tab: Tab::Bricks,
            camera: view3d::Camera::flat(),
            status: "Open an SVG or PNG to begin (or drop one onto the window).".into(),
            incoming: Arc::new(Mutex::new(initial)),
        }
    }

    fn open(&mut self, ctx: &egui::Context, name: String, bytes: &[u8]) {
        match source::load(&name, bytes) {
            Ok(image) => {
                let colour = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.rgba,
                );
                let texture = ctx.load_texture(&name, colour, egui::TextureOptions::LINEAR);
                self.status = format!("Loaded {name} ({}×{} px).", image.width, image.height);
                self.loaded = Some(Loaded { name, image, texture });
                self.built_with = None;
            }
            Err(e) => self.status = e,
        }
    }

    fn rebuild_if_needed(&mut self) {
        let Some(loaded) = &self.loaded else { return };
        if self.built_with.as_ref() != Some(&self.settings) {
            if self.built_with.as_ref().is_none_or(|b| b.layout != self.settings.layout) {
                self.camera = match self.settings.layout {
                    Layout::Flat => view3d::Camera::flat(),
                    Layout::Upright => view3d::Camera::upright(),
                };
            }
            let mosaic = Mosaic::build(&loaded.image, &self.settings);
            self.connectivity = mosaic.connectivity();
            self.mosaic = Some(mosaic);
            self.built_with = Some(self.settings.clone());
            self.tris.clear();
        }
        if let Some(m) = &self.mosaic
            && (self.tris.is_empty() || self.tris_have_studs != self.studs)
        {
            self.tris = geometry::build(m, self.studs);
            self.tris_have_studs = self.studs;
        }
    }

    fn base_name(&self) -> String {
        self.loaded
            .as_ref()
            .map(|l| l.name.rsplit_once('.').map_or(l.name.as_str(), |(stem, _)| stem).to_string())
            .unwrap_or_else(|| "model".into())
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Brickify");
        if ui.button("Open image…").clicked() {
            platform::pick_file(ui.ctx().clone(), self.incoming.clone());
        }
        ui.label(&self.status);
        ui.separator();

        let s = &mut self.settings;
        ui.label("Size");
        ui.horizontal(|ui| {
            if ui.button("−").clicked() {
                s.width = s.width.saturating_sub(1).max(4);
            }
            ui.add(egui::Slider::new(&mut s.width, 4..=256).suffix(" studs wide").logarithmic(true));
            if ui.button("+").clicked() {
                s.width = (s.width + 1).min(256);
            }
        });
        if let Some(m) = &self.mosaic {
            let (w, h, d) = m.size_mm();
            ui.label(format!(
                "{} × {} grid · {:.1} × {:.1} × {:.1} cm",
                m.cols,
                m.rows,
                w / 10.0,
                h / 10.0,
                d / 10.0
            ));
        }
        ui.add_space(6.0);

        ui.label("Build");
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.layout, Layout::Flat, "Flat mosaic");
            ui.radio_value(&mut s.layout, Layout::Upright, "Upright wall");
        });
        if s.layout == Layout::Upright && s.kind == PartKind::Tile {
            s.kind = PartKind::Brick;
        }
        ui.horizontal(|ui| {
            let kinds: &[PartKind] = match s.layout {
                Layout::Flat => &[PartKind::Plate, PartKind::Tile, PartKind::Brick],
                Layout::Upright => &[PartKind::Brick, PartKind::Plate],
            };
            for &k in kinds {
                ui.radio_value(&mut s.kind, k, k.label());
            }
        });
        ui.add(egui::Slider::new(&mut s.alpha_threshold, 0.05..=0.95).text("min. coverage"))
            .on_hover_text("How much of a stud's area must be covered by the image to place a part there.");

        optional_colour(ui, "Fill background", "bg", &mut s.background, 15);
        let border_tip = match s.layout {
            Layout::Upright => "An outline around the picture. Parts in a wall grip the rows above and below, \
                                so the border holds loose parts and colour changes together.",
            Layout::Flat => "A frame around the picture. In a flat mosaic, parts side by side don't grip \
                             each other, so only backing plates hold the picture together.",
        };
        optional_colour(ui, "Border", "border", &mut s.border, 0).on_hover_text(border_tip);
        if s.border.is_some() {
            ui.add(egui::Slider::new(&mut s.border_width, 1..=4).text("border studs"));
        }
        match s.layout {
            Layout::Flat => optional_colour(ui, "Backing plates", "backing", &mut s.backing, 0)
                .on_hover_text("A second layer of plates underneath that holds the mosaic together."),
            Layout::Upright => optional_colour(ui, "Back layer", "backing", &mut s.backing, 0).on_hover_text(
                "Makes the wall two studs deep, with hidden parts behind the picture that bridge colour changes.",
            ),
        };
        if self.mosaic.is_some() {
            let (groups, islands) = (self.connectivity.0, self.connectivity.1);
            if groups <= islands {
                ui.colored_label(Color32::from_rgb(110, 200, 110), "Every part is connected");
            } else {
                ui.colored_label(
                    Color32::from_rgb(240, 160, 60),
                    format!("Warning: falls apart into {groups} pieces ({islands} expected)"),
                )
                .on_hover_text(
                    "Parts only hold together where they overlap a part above or below. Loose parts are \
                     outlined in red in the Bricks view. Turn on the backing / back layer, or try another size.",
                );
            }
            if islands > 1 {
                ui.label(
                    egui::RichText::new(format!(
                        "The picture has {islands} separate areas. Fill the background to join them."
                    ))
                    .small(),
                );
            }
        }

        egui::CollapsingHeader::new(format!("Colours ({} enabled)", s.enabled_colours.len())).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.small_button("All").clicked() {
                    s.enabled_colours = COLOURS.iter().map(|c| c.code).collect();
                }
                if ui.small_button("Common").clicked() {
                    s.enabled_colours = palette::default_enabled();
                }
                if ui.small_button("None").clicked() {
                    s.enabled_colours.clear();
                }
            });
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for c in COLOURS {
                    let mut on = s.enabled_colours.contains(&c.code);
                    ui.horizontal(|ui| {
                        swatch(ui, c.code);
                        if ui.checkbox(&mut on, c.name).changed() {
                            if on {
                                s.enabled_colours.push(c.code);
                            } else {
                                s.enabled_colours.retain(|&x| x != c.code);
                            }
                        }
                    });
                }
            });
        });

        ui.separator();
        ui.checkbox(&mut self.studs, "Show studs in 3D");
        ui.label("Export");
        let ready = self.mosaic.is_some();
        ui.horizontal_wrapped(|ui| {
            let name = self.base_name();
            if ui.add_enabled(ready, egui::Button::new("LDraw (.ldr)")).clicked()
                && let Some(m) = &self.mosaic
            {
                platform::save_file(&format!("{name}.ldr"), export::ldraw(m, &name).as_bytes());
            }
            if ui.add_enabled(ready, egui::Button::new("3D model (.obj)")).clicked() {
                platform::save_file(&format!("{name}.obj"), export::obj(&self.tris).as_bytes());
            }
            if ui.add_enabled(ready, egui::Button::new("Parts list (.csv)")).clicked()
                && let Some(m) = &self.mosaic
            {
                platform::save_file(&format!("{name}-parts.csv"), export::parts_csv(m).as_bytes());
            }
        });

        if let Some(m) = &self.mosaic {
            let parts = m.parts_list();
            let total: usize = parts.iter().map(|p| p.2).sum();
            egui::CollapsingHeader::new(format!("Parts list ({total} pieces)"))
                .default_open(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("parts").striped(true).show(ui, |ui| {
                            for (part, colour, n) in parts {
                                swatch(ui, colour);
                                ui.label(palette::by_code(colour).map_or("?", |c| c.name));
                                ui.label(part.trim_end_matches(".dat"));
                                ui.label(format!("× {n}"));
                                ui.end_row();
                            }
                        });
                    });
                });
        }
    }

    fn bricks_2d(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, VIEW_BG);
        let Some(m) = &self.mosaic else { return };
        let aspect = self.settings.cell_aspect();
        let model = vec2(m.cols as f32, m.rows as f32 * aspect);
        let scale = (rect.width() / model.x).min(rect.height() / model.y) * 0.95;
        let origin = rect.center() - model * scale / 2.0;
        let cell = vec2(scale, scale * aspect);
        let loose = &self.connectivity.2;
        let loose_cells: std::collections::HashSet<(u32, u32)> = m
            .bricks
            .iter()
            .zip(loose)
            .filter(|(_, l)| **l)
            .flat_map(|(b, _)| (b.row..b.row + b.h).flat_map(move |r| (b.col..b.col + b.w).map(move |c| (c, r))))
            .collect();
        for b in m.bricks.iter().filter(|b| b.layer == 0 && b.z == 0) {
            let rgb = palette::by_code(b.colour).map_or([255, 0, 255], |c| c.rgb);
            let fill = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let min = origin + vec2(b.col as f32 * cell.x, b.row as f32 * cell.y);
            let r = Rect::from_min_size(min, vec2(b.w as f32 * cell.x, b.h as f32 * cell.y));
            // Very dark parts (like LEGO black) get a lighter edge so they stand out from the background.
            let dark = rgb.iter().map(|&v| v as u32).sum::<u32>() < 3 * 70;
            let edge = if dark { Stroke::new(1.0, Color32::from_gray(95)) } else { Stroke::NONE };
            painter.rect(r.shrink(0.5), scale * 0.08, fill, edge, egui::StrokeKind::Inside);
            if loose_cells.contains(&(b.col, b.row)) {
                let warn = Stroke::new(2.0, Color32::from_rgb(255, 60, 60));
                painter.rect_stroke(r.shrink(1.0), scale * 0.08, warn, egui::StrokeKind::Inside);
            }
            if m.layout == Layout::Flat && b.kind.has_studs() && scale > 5.0 {
                let ring = Stroke::new((scale * 0.06).max(1.0), darken(fill, 0.75));
                for rr in b.row..b.row + b.h {
                    for cc in b.col..b.col + b.w {
                        let c = origin + vec2((cc as f32 + 0.5) * cell.x, (rr as f32 + 0.5) * cell.y);
                        painter.circle(c, scale * 0.3, fill, ring);
                    }
                }
            }
        }
    }

    /// Shown before any image is loaded: a drop zone with instructions.
    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, VIEW_BG);
        let dragging = ui.input(|i| !i.raw.hovered_files.is_empty());
        let zone = rect.shrink(24.0);
        let (border, fill) = if dragging {
            (Color32::from_rgb(250, 200, 10), Color32::from_rgba_unmultiplied(250, 200, 10, 18))
        } else {
            (Color32::from_gray(120), Color32::TRANSPARENT)
        };
        painter.rect(zone, 16.0, fill, Stroke::new(2.0, border), egui::StrokeKind::Inside);

        let content = Rect::from_center_size(zone.center(), vec2(zone.width().min(420.0), 220.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
            ui.vertical_centered(|ui| {
                brick_icon(ui);
                ui.add_space(10.0);
                let title = if dragging { "Drop to brickify it" } else { "Drop an image here" };
                ui.label(egui::RichText::new(title).size(24.0).strong().color(Color32::WHITE));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("SVG, PNG or JPEG. Logos and simple artwork work best.").color(TEXT));
                ui.add_space(12.0);
                let button = egui::Button::new(egui::RichText::new("Choose an image…").size(16.0));
                if ui.add(button).clicked() {
                    platform::pick_file(ui.ctx().clone(), self.incoming.clone());
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Then set the size and parts on the left, and export to LDraw.").color(WEAK_TEXT),
                );
            });
        });
    }

    fn source_view(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, VIEW_BG);
        let Some(l) = &self.loaded else { return };
        let size = vec2(l.image.width as f32, l.image.height as f32);
        let scale = (rect.width() / size.x).min(rect.height() / size.y) * 0.95;
        let r = Rect::from_center_size(rect.center(), size * scale);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(l.texture.id(), r, uv, Color32::WHITE);
    }
}

impl eframe::App for BrickApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            platform::read_dropped(ctx.clone(), file, self.incoming.clone());
        }
        let incoming = self.incoming.lock().unwrap().take();
        if let Some((name, bytes)) = incoming {
            self.open(&ctx, name, &bytes);
        }
        self.rebuild_if_needed();

        egui::Panel::left("controls").default_size(320.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if self.loaded.is_none() {
                self.empty_state(ui);
                return;
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Source, "Original");
                ui.selectable_value(&mut self.tab, Tab::Bricks, "Bricks");
                ui.selectable_value(&mut self.tab, Tab::Model3d, "3D");
                if self.tab == Tab::Model3d {
                    ui.label(egui::RichText::new("drag to rotate · scroll to zoom").weak());
                }
            });
            match self.tab {
                Tab::Source => self.source_view(ui),
                Tab::Bricks => self.bricks_2d(ui),
                Tab::Model3d => view3d::show(ui, &self.tris, &mut self.camera),
            }
        });
    }
}

/// Scales a colour's brightness, keeping it opaque. (`gamma_multiply` fades alpha instead.)
fn darken(c: Color32, factor: f32) -> Color32 {
    let f = |v: u8| (v as f32 * factor) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// A 2x4 brick seen from above, drawn rather than taken from a font.
fn brick_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(88.0, 48.0), Sense::hover());
    let painter = ui.painter();
    let red = Color32::from_rgb(201, 26, 9);
    painter.rect_filled(rect, 6.0, red);
    for row in 0..2 {
        for col in 0..4 {
            let centre = rect.min + vec2(11.0 + col as f32 * 22.0, 12.0 + row as f32 * 24.0);
            painter.circle(centre, 7.0, red, Stroke::new(2.0, darken(red, 0.65)));
        }
    }
}

fn swatch(ui: &mut egui::Ui, code: u16) {
    let rgb = palette::by_code(code).map_or([255, 0, 255], |c| c.rgb);
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    // The outline keeps dark colours visible on the dark panel.
    ui.painter().rect(
        r,
        3.0,
        Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
        Stroke::new(1.0, Color32::from_gray(150)),
        egui::StrokeKind::Inside,
    );
}

/// Checkbox plus colour picker for an optional colour setting.
fn optional_colour(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: &mut Option<u16>,
    default: u16,
) -> egui::Response {
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        let response = ui.checkbox(&mut on, label);
        if on != value.is_some() {
            *value = on.then_some(default);
        }
        if let Some(code) = value {
            swatch(ui, *code);
            let name = palette::by_code(*code).map_or("?", |c| c.name);
            egui::ComboBox::from_id_salt(id).selected_text(name).show_ui(ui, |ui| {
                for c in COLOURS {
                    ui.selectable_value(code, c.code, c.name);
                }
            });
        }
        response
    })
    .inner
}
