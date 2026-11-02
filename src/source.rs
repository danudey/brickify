//! Loading SVG / raster images into a straight-alpha RGBA buffer.

use resvg::{tiny_skia, usvg};

/// Longest side, in pixels, an SVG is rasterised to before sampling.
const SVG_RENDER_SIZE: f32 = 2048.0;

pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA, row-major.
    pub rgba: Vec<u8>,
}

pub fn load(name: &str, bytes: &[u8]) -> Result<SourceImage, String> {
    let lower = name.to_ascii_lowercase();
    let looks_like_svg = lower.ends_with(".svg")
        || lower.ends_with(".svgz")
        || bytes.starts_with(b"<?xml")
        || bytes.starts_with(b"<svg");
    if looks_like_svg {
        load_svg(bytes)
    } else {
        load_raster(bytes)
    }
}

fn load_svg(bytes: &[u8]) -> Result<SourceImage, String> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|e| format!("Could not parse SVG: {e}"))?;
    let size = tree.size();
    let scale = SVG_RENDER_SIZE / size.width().max(size.height());
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("SVG has an invalid size")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Ok(SourceImage {
        width,
        height,
        rgba,
    })
}

fn load_raster(bytes: &[u8]) -> Result<SourceImage, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("Could not decode image: {e}"))?
        .into_rgba8();
    Ok(SourceImage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}
