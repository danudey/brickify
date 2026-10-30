//! Interactive 3D preview: projects triangles on the CPU and draws them, back to front, as an egui mesh.

use eframe::egui::{self, Color32, Mesh, Pos2, Sense, epaint::Vertex};

use crate::geometry::{self, Tri, dot};

pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    /// Distance as a multiple of the model's size.
    pub zoom: f32,
}

impl Camera {
    pub fn flat() -> Self {
        Self { yaw: 0.0, pitch: 0.9, zoom: 1.4 }
    }

    pub fn upright() -> Self {
        Self { yaw: 0.35, pitch: 0.25, zoom: 1.4 }
    }
}

pub fn show(ui: &mut egui::Ui, tris: &[Tri], cam: &mut Camera) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
    if response.dragged() {
        let d = response.drag_delta();
        cam.yaw -= d.x * 0.01;
        cam.pitch = (cam.pitch + d.y * 0.01).clamp(-1.55, 1.55);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        cam.zoom = (cam.zoom * (-scroll * 0.002).exp()).clamp(0.3, 10.0);
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, crate::app::VIEW_BG);
    if tris.is_empty() {
        return;
    }

    let (lo, hi) = geometry::bounds(tris);
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = (0..3).map(|i| hi[i] - lo[i]).fold(0.0f32, f32::max) / 2.0;
    let fov = 35f32.to_radians();
    let dist = radius / (fov / 2.0).tan() * cam.zoom + radius;
    let focal = rect.width().min(rect.height()) / 2.0 / (fov / 2.0).tan();
    let (sy, cy) = cam.yaw.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    // Rotate about Y (yaw), then X (pitch); the eye sits at (0, 0, -dist).
    let rotate = |p: [f32; 3]| {
        let x = p[0] * cy - p[2] * sy;
        let z = p[0] * sy + p[2] * cy;
        [x, p[1] * cp - z * sp, p[1] * sp + z * cp]
    };
    let light = normalize([0.35, -1.0, -0.55]);

    let mut visible: Vec<(f32, [Pos2; 3], Color32)> = Vec::with_capacity(tris.len() / 2);
    for t in tris {
        let v = t.v.map(|p| {
            let q = rotate([p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]);
            [q[0], q[1], q[2] + dist]
        });
        let n = rotate(t.normal);
        if dot(n, v[0]) >= 0.0 || v.iter().any(|p| p[2] < 1.0) {
            continue; // facing away, or behind the camera
        }
        let shade = 0.45 + 0.55 * dot(t.normal, light).max(0.0);
        let colour = Color32::from_rgb(
            (t.rgb[0] as f32 * shade) as u8,
            (t.rgb[1] as f32 * shade) as u8,
            (t.rgb[2] as f32 * shade) as u8,
        );
        let screen = v.map(|p| rect.center() + egui::vec2(p[0], p[1]) * (focal / p[2]));
        let depth = v[0][2] + v[1][2] + v[2][2];
        visible.push((depth, screen, colour));
    }
    visible.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

    let mut mesh = Mesh::default();
    for (_, pts, colour) in visible {
        let base = mesh.vertices.len() as u32;
        for pos in pts {
            mesh.vertices.push(Vertex { pos, uv: egui::epaint::WHITE_UV, color: colour });
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt();
    v.map(|c| c / len)
}
