mod app;
mod export;
mod geometry;
mod mosaic;
mod palette;
mod platform;
mod source;
mod view3d;

use mosaic::Settings;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mosaic::{Layout, Parts};
    const USAGE: &str = "usage: brickify [IMAGE] [--width STUDS] [--layout flat|upright] \
                         [--parts plate|brick|hybrid] [--smooth] [--no-backing] [--border STUDS] \
                         [--export OUT.ldr|.obj|.csv]";
    let mut settings = Settings::default();
    let mut input = None;
    let mut export_to = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--width" => settings.width = value()?.parse()?,
            "--layout" => {
                settings.layout = match value()?.as_str() {
                    "upright" => Layout::Upright,
                    _ => Layout::Flat,
                }
            }
            "--parts" => {
                let parts = value()?;
                settings.parts = match parts.as_str() {
                    "brick" => Parts::Bricks,
                    "hybrid" => Parts::Hybrid,
                    _ => Parts::Plates,
                };
                // "tile" is kept as shorthand for plates with a smooth top.
                settings.smooth_top |= parts == "tile";
            }
            "--no-backing" => settings.backing = None,
            "--smooth" => settings.smooth_top = true,
            "--border" => {
                settings.border = Some(0);
                settings.border_width = value()?.parse()?;
            }
            "--export" => export_to = Some(value()?),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => input = Some(arg),
        }
    }
    let initial = match &input {
        Some(path) => {
            let name = std::path::Path::new(path)
                .file_name()
                .map_or(path.clone(), |n| n.to_string_lossy().into_owned());
            Some((name, std::fs::read(path)?))
        }
        None => None,
    };

    if let Some(out) = export_to {
        let (name, bytes) = initial.ok_or("--export needs an input image")?;
        let image = source::load(&name, &bytes)?;
        let m = mosaic::Mosaic::build(&image, &settings);
        let title = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
        let text = match out.rsplit_once('.').map(|(_, ext)| ext) {
            Some("obj") => export::obj(&geometry::build(&m, true)),
            Some("csv") => export::parts_csv(&m),
            _ => export::ldraw(&m, title),
        };
        std::fs::write(&out, text)?;
        let pieces: usize = m.parts_list().iter().map(|p| p.2).sum();
        let (groups, islands, _) = m.connectivity();
        println!(
            "{}×{} grid, {pieces} pieces, {groups} connected groups for {islands} islands → {out}",
            m.cols, m.rows
        );
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Brickify",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::BrickApp::new(
                &cc.egui_ctx,
                initial,
                settings,
            )))
        }),
    )?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("brickify_canvas"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("page should contain a canvas with id brickify_canvas");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    Ok(Box::new(app::BrickApp::new(
                        &cc.egui_ctx,
                        None,
                        Settings::default(),
                    )))
                }),
            )
            .await;
        if let Some(loading) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("loading"))
        {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_inner_html(&format!("Brickify failed to start: {e:?}")),
            }
        }
    });
}
