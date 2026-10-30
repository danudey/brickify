//! File open/save that works natively (dialogs) and on the web (picker + browser download).

use std::sync::{Arc, Mutex};

use eframe::egui;

pub type Incoming = Arc<Mutex<Option<(String, Vec<u8>)>>>;

const IMAGE_EXTENSIONS: &[&str] = &["svg", "png", "jpg", "jpeg"];

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_file(ctx: egui::Context, incoming: Incoming) {
    let Some(path) = rfd::FileDialog::new().add_filter("Images", IMAGE_EXTENSIONS).pick_file() else {
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            *incoming.lock().unwrap() = Some((file_name(&path), bytes));
            ctx.request_repaint();
        }
        Err(e) => log::error!("Could not read {}: {e}", path.display()),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn pick_file(ctx: egui::Context, incoming: Incoming) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(file) = rfd::AsyncFileDialog::new().add_filter("Images", IMAGE_EXTENSIONS).pick_file().await
        else {
            return;
        };
        let bytes = file.read().await;
        *incoming.lock().unwrap() = Some((file.file_name(), bytes));
        ctx.request_repaint();
    });
}

pub type Dropped = Arc<dyn egui::DroppedFile + Send + Sync>;

fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or("image".into(), |n| n.to_string_lossy().into_owned())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_dropped(ctx: egui::Context, file: Dropped, incoming: Incoming) {
    match file.bytes() {
        Ok(bytes) => {
            *incoming.lock().unwrap() = Some((file_name(file.path()), bytes));
            ctx.request_repaint();
        }
        Err(e) => log::error!("Could not read dropped file: {e}"),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn read_dropped(ctx: egui::Context, file: Dropped, incoming: Incoming) {
    wasm_bindgen_futures::spawn_local(async move {
        match file.bytes_async().await {
            Ok(bytes) => {
                *incoming.lock().unwrap() = Some((file_name(file.path()), bytes));
                ctx.request_repaint();
            }
            Err(e) => log::error!("Could not read dropped file: {e}"),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_file(name: &str, bytes: &[u8]) {
    if let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file()
        && let Err(e) = std::fs::write(&path, bytes)
    {
        log::error!("Could not write {}: {e}", path.display());
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_file(name: &str, bytes: &[u8]) {
    use wasm_bindgen::JsCast;
    let result = (|| -> Result<(), wasm_bindgen::JsValue> {
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::of1(&array);
        let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        web_sys::Url::revoke_object_url(&url)?;
        Ok(())
    })();
    if let Err(e) = result {
        log::error!("Download failed: {e:?}");
    }
}
