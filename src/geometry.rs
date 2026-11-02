//! World-space geometry of a mosaic, in LDraw units and axes (+Y down, -Z towards the viewer).

use crate::mosaic::{Brick, Layout, Mosaic, STUD};
use crate::palette;

/// Gap left between neighbouring parts so their seams are visible.
const GAP: f32 = 0.4;
const STUD_RADIUS: f32 = 6.0;
const STUD_HEIGHT: f32 = 4.0;
const STUD_SIDES: usize = 8;
/// Depth of an upright wall.
const WALL_DEPTH: f32 = STUD;

#[derive(Clone, Copy)]
pub struct Tri {
    pub v: [[f32; 3]; 3],
    pub normal: [f32; 3],
    pub rgb: [u8; 3],
}

/// Bounds of a part's body (studs excluded): (min, max).
pub fn brick_bounds(m: &Mosaic, b: &Brick) -> ([f32; 3], [f32; 3]) {
    let x0 = b.col as f32 * STUD;
    let x1 = (b.col + b.w) as f32 * STUD;
    match m.layout {
        Layout::Flat => {
            let backing = if m.bricks.iter().any(|b| b.layer == 1) {
                8.0
            } else {
                0.0
            };
            let (top, bottom) = if b.layer == 0 {
                (-(m.kind.height() + backing), -backing)
            } else {
                (-backing, 0.0)
            };
            // Row 0 (the top of the image) is furthest from the viewer.
            let z0 = (m.rows - b.row - b.h) as f32 * STUD;
            let z1 = (m.rows - b.row) as f32 * STUD;
            ([x0, top, z0], [x1, bottom, z1])
        }
        Layout::Upright => {
            // Grid rows are one `m.kind` tall; a part can span several (hybrid bricks span 3).
            let row_height = m.kind.height();
            let top = -((m.rows - b.row) as f32) * row_height;
            let front = -WALL_DEPTH / 2.0 + b.z as f32 * STUD;
            (
                [x0, top, front],
                [x1, top + b.h as f32 * row_height, front + b.d as f32 * STUD],
            )
        }
    }
}

/// Builds triangles for the whole mosaic. Faces hidden against a neighbouring part are skipped.
pub fn build(m: &Mosaic, studs: bool) -> Vec<Tri> {
    let mut tris = Vec::new();
    let has_backing = m.bricks.iter().any(|b| b.layer == 1);
    let filled = |c: i64, r: i64| {
        c >= 0
            && r >= 0
            && c < m.cols as i64
            && r < m.rows as i64
            && m.cell(c as u32, r as u32).is_some()
    };
    // Upright walls can be two deep, so track occupancy per (cell, depth slot).
    let mut slots = vec![false; (m.cols * m.rows * 2) as usize];
    if m.layout == Layout::Upright {
        for b in &m.bricks {
            for r in b.row..b.row + b.h {
                for c in b.col..b.col + b.w {
                    for z in b.z..b.z + b.d {
                        slots[((r * m.cols + c) * 2 + z) as usize] = true;
                    }
                }
            }
        }
    }
    let occupied = |c: i64, r: i64, z: i64| {
        c >= 0
            && r >= 0
            && (0..2).contains(&z)
            && c < m.cols as i64
            && r < m.rows as i64
            && slots[((r * m.cols as i64 + c) * 2 + z) as usize]
    };
    for b in &m.bricks {
        let rgb = palette::by_code(b.colour).map_or([255, 0, 255], |c| c.rgb);
        let (lo, hi) = brick_bounds(m, b);
        let (x0, x1) = (lo[0] + GAP, hi[0] - GAP);
        let (y0, y1) = (lo[1], hi[1]);
        let (c0, c1) = (b.col as i64, (b.col + b.w) as i64);
        let (r0, r1) = (b.row as i64, (b.row + b.h) as i64);
        let mut faces = Vec::new();
        let mut stud_bases = Vec::new();
        let mut quad = |a: [f32; 3], bb: [f32; 3], c: [f32; 3], d: [f32; 3], n: [f32; 3]| {
            faces.push(([a, bb, c, d], n));
        };
        match m.layout {
            Layout::Flat => {
                let (z0, z1) = (lo[2] + GAP, hi[2] - GAP);
                let z_of = |r: i64| (m.rows as i64 - r) as f32 * STUD;
                // Top and bottom, one quad per cell so depth sorting stays accurate.
                for r in r0..r1 {
                    let (za, zb) = ((z_of(r + 1)).max(z0), z_of(r).min(z1));
                    for c in c0..c1 {
                        let (xa, xb) = ((c as f32 * STUD).max(x0), ((c + 1) as f32 * STUD).min(x1));
                        if b.layer == 0 {
                            quad(
                                [xa, y0, za],
                                [xb, y0, za],
                                [xb, y0, zb],
                                [xa, y0, zb],
                                [0.0, -1.0, 0.0],
                            );
                        }
                        if b.layer == 1 || !has_backing {
                            quad(
                                [xa, y1, za],
                                [xb, y1, za],
                                [xb, y1, zb],
                                [xa, y1, zb],
                                [0.0, 1.0, 0.0],
                            );
                        }
                    }
                }
                // Sides facing empty cells.
                for c in c0..c1 {
                    let (xa, xb) = ((c as f32 * STUD).max(x0), ((c + 1) as f32 * STUD).min(x1));
                    if !filled(c, r0 - 1) {
                        quad(
                            [xa, y0, z1],
                            [xb, y0, z1],
                            [xb, y1, z1],
                            [xa, y1, z1],
                            [0.0, 0.0, 1.0],
                        );
                    }
                    if !filled(c, r1) {
                        quad(
                            [xa, y0, z0],
                            [xb, y0, z0],
                            [xb, y1, z0],
                            [xa, y1, z0],
                            [0.0, 0.0, -1.0],
                        );
                    }
                }
                for r in r0..r1 {
                    let (za, zb) = ((z_of(r + 1)).max(z0), z_of(r).min(z1));
                    if !filled(c0 - 1, r) {
                        quad(
                            [x0, y0, za],
                            [x0, y0, zb],
                            [x0, y1, zb],
                            [x0, y1, za],
                            [-1.0, 0.0, 0.0],
                        );
                    }
                    if !filled(c1, r) {
                        quad(
                            [x1, y0, za],
                            [x1, y0, zb],
                            [x1, y1, zb],
                            [x1, y1, za],
                            [1.0, 0.0, 0.0],
                        );
                    }
                }
                if studs && b.kind.has_studs() && b.layer == 0 {
                    for r in r0..r1 {
                        for c in c0..c1 {
                            let centre = [(c as f32 + 0.5) * STUD, y0, z_of(r) - STUD / 2.0];
                            stud_bases.push(centre);
                        }
                    }
                }
            }
            Layout::Upright => {
                let (z0, z1) = (lo[2], hi[2]);
                let (s0, s1) = (b.z as i64, (b.z + b.d) as i64);
                let slot_z = |z: i64| -WALL_DEPTH / 2.0 + z as f32 * STUD;
                let row_height = m.kind.height();
                // Per grid row, so faces beside a taller neighbour are culled row by row.
                let row_y = |r: i64| {
                    (
                        y0 + (r - r0) as f32 * row_height,
                        y0 + (r - r0 + 1) as f32 * row_height,
                    )
                };
                for c in c0..c1 {
                    let (xa, xb) = ((c as f32 * STUD).max(x0), ((c + 1) as f32 * STUD).min(x1));
                    for r in r0..r1 {
                        let (ya, yb) = row_y(r);
                        if !occupied(c, r, s0 - 1) {
                            quad(
                                [xa, ya, z0],
                                [xb, ya, z0],
                                [xb, yb, z0],
                                [xa, yb, z0],
                                [0.0, 0.0, -1.0],
                            );
                        }
                        if !occupied(c, r, s1) {
                            quad(
                                [xa, ya, z1],
                                [xb, ya, z1],
                                [xb, yb, z1],
                                [xa, yb, z1],
                                [0.0, 0.0, 1.0],
                            );
                        }
                    }
                    for z in s0..s1 {
                        let (za, zb) = (slot_z(z), slot_z(z + 1));
                        if !occupied(c, r0 - 1, z) {
                            quad(
                                [xa, y0, za],
                                [xb, y0, za],
                                [xb, y0, zb],
                                [xa, y0, zb],
                                [0.0, -1.0, 0.0],
                            );
                            if studs && b.kind.has_studs() {
                                stud_bases.push([(c as f32 + 0.5) * STUD, y0, (za + zb) / 2.0]);
                            }
                        }
                        if !occupied(c, r1, z) {
                            quad(
                                [xa, y1, za],
                                [xb, y1, za],
                                [xb, y1, zb],
                                [xa, y1, zb],
                                [0.0, 1.0, 0.0],
                            );
                        }
                    }
                }
                for r in r0..r1 {
                    let (ya, yb) = row_y(r);
                    for z in s0..s1 {
                        let (za, zb) = (slot_z(z), slot_z(z + 1));
                        if !occupied(c0 - 1, r, z) {
                            quad(
                                [x0, ya, za],
                                [x0, ya, zb],
                                [x0, yb, zb],
                                [x0, yb, za],
                                [-1.0, 0.0, 0.0],
                            );
                        }
                        if !occupied(c1, r, z) {
                            quad(
                                [x1, ya, za],
                                [x1, ya, zb],
                                [x1, yb, zb],
                                [x1, yb, za],
                                [1.0, 0.0, 0.0],
                            );
                        }
                    }
                }
            }
        }
        for (q, n) in faces {
            push_quad(&mut tris, q, n, rgb);
        }
        for base in stud_bases {
            push_stud(&mut tris, base, rgb);
        }
    }
    tris
}

/// Adds a quad as two triangles, wound counter-clockwise when seen from the normal's side.
fn push_quad(tris: &mut Vec<Tri>, q: [[f32; 3]; 4], normal: [f32; 3], rgb: [u8; 3]) {
    let facing = dot(cross(sub(q[1], q[0]), sub(q[2], q[0])), normal);
    let q = if facing < 0.0 {
        [q[0], q[3], q[2], q[1]]
    } else {
        q
    };
    tris.push(Tri {
        v: [q[0], q[1], q[2]],
        normal,
        rgb,
    });
    tris.push(Tri {
        v: [q[0], q[2], q[3]],
        normal,
        rgb,
    });
}

fn push_stud(tris: &mut Vec<Tri>, base: [f32; 3], rgb: [u8; 3]) {
    let top = base[1] - STUD_HEIGHT;
    let ring = |i: usize| {
        let a = i as f32 / STUD_SIDES as f32 * std::f32::consts::TAU;
        (
            base[0] + STUD_RADIUS * a.cos(),
            base[2] + STUD_RADIUS * a.sin(),
            a,
        )
    };
    for i in 0..STUD_SIDES {
        let (xa, za, aa) = ring(i);
        let (xb, zb, ab) = ring(i + 1);
        let mid = (aa + ab) / 2.0;
        let n = [mid.cos(), 0.0, mid.sin()];
        push_quad(
            tris,
            [
                [xa, top, za],
                [xb, top, zb],
                [xb, base[1], zb],
                [xa, base[1], za],
            ],
            n,
            rgb,
        );
        let mut cap = [[base[0], top, base[2]], [xa, top, za], [xb, top, zb]];
        if dot(
            cross(sub(cap[1], cap[0]), sub(cap[2], cap[0])),
            [0.0, -1.0, 0.0],
        ) < 0.0
        {
            cap.swap(1, 2);
        }
        tris.push(Tri {
            v: cap,
            normal: [0.0, -1.0, 0.0],
            rgb,
        });
    }
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Axis-aligned bounds of a set of triangles.
pub fn bounds(tris: &[Tri]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for t in tris {
        for v in t.v {
            for i in 0..3 {
                lo[i] = lo[i].min(v[i]);
                hi[i] = hi[i].max(v[i]);
            }
        }
    }
    (lo, hi)
}
