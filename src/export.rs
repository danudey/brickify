//! LDraw, OBJ and parts-list exports.

use std::fmt::Write;

use crate::geometry::{self, Tri};
use crate::mosaic::{Layout, Mosaic};
use crate::palette;

/// LDraw model. Each row (or layer) is a separate build step.
pub fn ldraw(m: &Mosaic, title: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "0 {title}");
    let _ = writeln!(out, "0 Name: {title}.ldr");
    let _ = writeln!(out, "0 Author: Brickify");
    let _ = writeln!(out, "0 !LICENSE Redistributable under CCAL version 2.0");

    let mut bricks: Vec<_> = m.bricks.iter().collect();
    match m.layout {
        // Backing first, then the picture from back to front.
        Layout::Flat => bricks.sort_by_key(|b| (std::cmp::Reverse(b.layer), b.row, b.col)),
        // Bottom row first.
        Layout::Upright => bricks.sort_by_key(|b| (std::cmp::Reverse(b.row), b.z, b.col)),
    }
    let mut step = None;
    for b in bricks {
        let key = (b.layer, b.row);
        if step.is_some_and(|s| s != key) {
            let _ = writeln!(out, "0 STEP");
        }
        step = Some(key);
        let (lo, hi) = geometry::brick_bounds(m, b);
        let (x, y, z) = ((lo[0] + hi[0]) / 2.0, lo[1], (lo[2] + hi[2]) / 2.0);
        // Parts are modelled with their long side along X; turn them when it runs along Z.
        let along_z = match m.layout {
            Layout::Flat => b.h > b.w,
            Layout::Upright => b.d > b.w,
        };
        let matrix = if along_z { "0 0 1 0 1 0 -1 0 0" } else { "1 0 0 0 1 0 0 0 1" };
        let _ = writeln!(out, "1 {} {x} {y} {z} {matrix} {}", b.colour, b.part);
    }
    let _ = writeln!(out, "0 STEP");
    out
}

/// Wavefront OBJ with per-vertex colours, in millimetres with +Y up.
pub fn obj(tris: &[Tri]) -> String {
    let mm = 0.4; // millimetres per LDraw unit
    let mut out = String::from("# Brickify model\n");
    for t in tris {
        let [r, g, b] = t.rgb.map(|c| c as f32 / 255.0);
        for v in t.v {
            // Rotate 180 degrees about X: LDraw is +Y down, OBJ is +Y up.
            let _ = writeln!(out, "v {:.2} {:.2} {:.2} {r:.3} {g:.3} {b:.3}", v[0] * mm, -v[1] * mm, -v[2] * mm);
        }
    }
    for t in tris {
        let _ = writeln!(out, "vn {:.3} {:.3} {:.3}", t.normal[0], -t.normal[1], -t.normal[2]);
    }
    for i in 0..tris.len() {
        let (a, n) = (i * 3 + 1, i + 1);
        let _ = writeln!(out, "f {a}//{n} {}//{n} {}//{n}", a + 1, a + 2);
    }
    out
}

/// Parts list as CSV.
pub fn parts_csv(m: &Mosaic) -> String {
    let mut out = String::from("part,colour_code,colour,quantity\n");
    for (part, colour, n) in m.parts_list() {
        let name = palette::by_code(colour).map_or("?", |c| c.name);
        let _ = writeln!(out, "{},{colour},{name},{n}", part.trim_end_matches(".dat"));
    }
    out
}
