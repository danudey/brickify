//! Turns a source image into a grid of LEGO colours, then merges cells into real parts.

use crate::palette::Matcher;
use crate::source::SourceImage;

/// LDraw units per stud.
pub const STUD: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// Lying flat, studs facing up: a mosaic viewed from above.
    Flat,
    /// Standing up as a one-stud-deep wall.
    Upright,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartKind {
    Plate,
    Brick,
    Tile,
}

impl PartKind {
    /// Height in LDraw units.
    pub fn height(self) -> f32 {
        match self {
            PartKind::Brick => 24.0,
            PartKind::Plate | PartKind::Tile => 8.0,
        }
    }

    pub fn has_studs(self) -> bool {
        self != PartKind::Tile
    }

    pub fn label(self) -> &'static str {
        match self {
            PartKind::Plate => "Plates",
            PartKind::Brick => "Bricks",
            PartKind::Tile => "Tiles",
        }
    }

    /// Available parts as (long side, short side, LDraw file), largest area first.
    fn catalogue(self) -> &'static [(u32, u32, &'static str)] {
        match self {
            PartKind::Plate => &[
                (8, 6, "3036.dat"),
                (6, 6, "3958.dat"),
                (8, 4, "3035.dat"),
                (6, 4, "3032.dat"),
                (12, 2, "2445.dat"),
                (10, 2, "3832.dat"),
                (4, 4, "3031.dat"),
                (8, 2, "3034.dat"),
                (12, 1, "60479.dat"),
                (6, 2, "3795.dat"),
                (10, 1, "4477.dat"),
                (8, 1, "3460.dat"),
                (4, 2, "3020.dat"),
                (6, 1, "3666.dat"),
                (3, 2, "3021.dat"),
                (4, 1, "3710.dat"),
                (2, 2, "3022.dat"),
                (3, 1, "3623.dat"),
                (2, 1, "3023b.dat"),
                (1, 1, "3024.dat"),
            ],
            PartKind::Brick => &[
                (8, 2, "3007.dat"),
                (12, 1, "6112.dat"),
                (6, 2, "2456.dat"),
                (10, 1, "6111.dat"),
                (8, 1, "3008.dat"),
                (4, 2, "3001.dat"),
                (6, 1, "3009.dat"),
                (3, 2, "3002.dat"),
                (4, 1, "3010.dat"),
                (2, 2, "3003.dat"),
                (3, 1, "3622.dat"),
                (2, 1, "3004.dat"),
                (1, 1, "3005.dat"),
            ],
            PartKind::Tile => &[
                (4, 2, "87079.dat"),
                (8, 1, "4162.dat"),
                (3, 2, "26603.dat"),
                (6, 1, "6636.dat"),
                (2, 2, "3068b.dat"),
                (4, 1, "2431.dat"),
                (3, 1, "63864.dat"),
                (2, 1, "3069b.dat"),
                (1, 1, "3070b.dat"),
            ],
        }
    }

    fn part_for(self, long: u32, short: u32) -> &'static str {
        self.catalogue()
            .iter()
            .find(|p| p.0 == long && p.1 == short)
            .map(|p| p.2)
            .expect("size comes from the catalogue")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub layout: Layout,
    /// Plates or bricks. (Tiles come from `smooth_top`.)
    pub kind: PartKind,
    /// Use smooth tiles instead of plates wherever nothing sits on top, if that tile exists.
    pub smooth_top: bool,
    /// Width of the build, in studs.
    pub width: u32,
    /// Minimum fraction of a cell that must be opaque for it to get a part.
    pub alpha_threshold: f32,
    /// Fill transparent areas with this colour instead of leaving them empty.
    pub background: Option<u16>,
    /// Flat layout only: add a layer of plates underneath to hold everything together.
    pub backing: Option<u16>,
    /// Outline the picture with this colour. Holds upright walls together.
    pub border: Option<u16>,
    /// Border thickness, in studs.
    pub border_width: u32,
    pub enabled_colours: Vec<u16>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: Layout::Flat,
            kind: PartKind::Plate,
            smooth_top: false,
            width: 32,
            alpha_threshold: 0.5,
            background: None,
            backing: Some(0),
            border: None,
            border_width: 1,
            enabled_colours: crate::palette::default_enabled(),
        }
    }
}

impl Settings {
    /// Height of one grid row relative to its width.
    pub fn cell_aspect(&self) -> f32 {
        match self.layout {
            Layout::Flat => 1.0,
            Layout::Upright => self.kind.height() / STUD,
        }
    }
}

/// One placed part, in grid coordinates.
#[derive(Clone, Debug)]
pub struct Brick {
    pub col: u32,
    pub row: u32,
    /// Size in grid cells along columns and rows.
    pub w: u32,
    pub h: u32,
    pub colour: u16,
    pub part: &'static str,
    pub kind: PartKind,
    /// Flat layout: 0 is the visible layer; 1 is the backing layer below it.
    pub layer: u32,
    /// Upright layout: first depth slot (0 is the front) and depth in studs.
    pub z: u32,
    pub d: u32,
}

pub struct Mosaic {
    pub cols: u32,
    pub rows: u32,
    pub layout: Layout,
    pub kind: PartKind,
    /// Colour per cell, `None` when empty.
    pub cells: Vec<Option<u16>>,
    pub bricks: Vec<Brick>,
}

impl Mosaic {
    pub fn build(src: &SourceImage, s: &Settings) -> Mosaic {
        let cols = s.width.max(1);
        let cell_px = src.width as f32 / cols as f32;
        let rows = ((src.height as f32 / (cell_px * s.cell_aspect())).round() as u32).max(1);
        let mut cells = sample(src, cols, rows, s);
        let (mut cols, mut rows) = (cols, rows);
        if let Some(colour) = s.border {
            // Rows of an upright wall are shorter than a stud is wide, so use more of them.
            let across = s.border_width.max(1);
            let down = ((across as f32 / s.cell_aspect()).round() as u32).max(1);
            (cells, cols, rows) = add_border(&cells, cols, rows, colour, across, down);
        }
        if let Some(background) = s.background {
            for cell in &mut cells {
                cell.get_or_insert(background);
            }
        }
        // A flat mosaic's picture layer is its top, so smooth means tiles throughout.
        let kind = if s.layout == Layout::Flat && s.smooth_top && s.kind == PartKind::Plate {
            PartKind::Tile
        } else {
            s.kind
        };
        let bricks = match s.layout {
            Layout::Flat => {
                let mut bricks = merge_flat(&cells, cols, rows, kind, 0);
                if let Some(colour) = s.backing {
                    let backing = merge_backing(&cells, &bricks, cols, rows, colour);
                    let (top, backing) = repair(&cells, bricks, backing, cols, rows);
                    bricks = top;
                    bricks.extend(backing);
                }
                bricks
            }
            Layout::Upright => {
                let wall = merge_wall(&cells, cols, rows, s.kind, s.backing);
                let wall = repair_wall(wall, cols, rows, s.kind);
                if s.smooth_top { smooth_wall_tops(wall, cols, rows) } else { wall }
            }
        };
        Mosaic { cols, rows, layout: s.layout, kind, cells, bricks }
    }

    pub fn cell(&self, col: u32, row: u32) -> Option<u16> {
        self.cells[(row * self.cols + col) as usize]
    }

    /// Part counts as (part file, colour code, count), sorted.
    pub fn parts_list(&self) -> Vec<(&'static str, u16, usize)> {
        let mut map = std::collections::BTreeMap::new();
        for b in &self.bricks {
            *map.entry((b.part, b.colour)).or_insert(0usize) += 1;
        }
        map.into_iter().map(|((p, c), n)| (p, c, n)).collect()
    }

    /// Physical size in millimetres (width, height, depth).
    pub fn size_mm(&self) -> (f32, f32, f32) {
        let stud_mm = 8.0;
        let ldu_mm = 0.4;
        let layers = if self.bricks.iter().any(|b| b.layer == 1) {
            8.0
        } else {
            0.0
        };
        match self.layout {
            Layout::Flat => (
                self.cols as f32 * stud_mm,
                self.rows as f32 * stud_mm,
                (self.kind.height() + layers) * ldu_mm,
            ),
            Layout::Upright => (
                self.cols as f32 * stud_mm,
                self.rows as f32 * self.kind.height() * ldu_mm,
                self.bricks.iter().map(|b| b.z + b.d).max().unwrap_or(1) as f32 * stud_mm,
            ),
        }
    }

    /// Whether the build holds together: (groups of connected parts, separate islands in the
    /// picture, which parts are loose). Each island of the picture should be exactly one
    /// group; parts outside the biggest group on their island are loose.
    pub fn connectivity(&self) -> (usize, usize, Vec<bool>) {
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        let (mut groups, _) = part_groups(self.layout, &self.bricks, self.cols, self.rows);
        let group_count = (0..self.bricks.len())
            .filter(|&i| groups.find(i) == i)
            .count();

        let mut islands = UnionFind::new(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                if self.cells[i].is_none() {
                    continue;
                }
                if c + 1 < cols && self.cells[i + 1].is_some() {
                    islands.union(i, i + 1);
                }
                if r + 1 < rows && self.cells[i + cols].is_some() {
                    islands.union(i, i + cols);
                }
            }
        }
        let island_count = (0..cols * rows)
            .filter(|&i| self.cells[i].is_some() && islands.find(i) == i)
            .count();

        // Size of each group, and the biggest group on each island.
        let group_of: Vec<usize> = (0..self.bricks.len()).map(|i| groups.find(i)).collect();
        let island_of: Vec<usize> = self
            .bricks
            .iter()
            .map(|b| islands.find(b.row as usize * cols + b.col as usize))
            .collect();
        let mut size = std::collections::HashMap::new();
        for &g in &group_of {
            *size.entry(g).or_insert(0usize) += 1;
        }
        let mut main = std::collections::HashMap::new();
        for (i, &g) in group_of.iter().enumerate() {
            let best = main.entry(island_of[i]).or_insert(g);
            if size[&g] > size[best] {
                *best = g;
            }
        }
        let loose = (0..self.bricks.len())
            .map(|i| main[&island_of[i]] != group_of[i])
            .collect();
        (group_count, island_count, loose)
    }
}

/// Groups parts that hold onto each other. Also returns which part occupies each
/// (cell, slot), where flat slots are layers and upright slots are depths.
fn part_groups(layout: Layout, bricks: &[Brick], cols: u32, rows: u32) -> (UnionFind, Vec<usize>) {
    let (cols, rows) = (cols as usize, rows as usize);
    let mut groups = UnionFind::new(bricks.len());
    let mut occupant = vec![usize::MAX; cols * rows * 2];
    for (i, b) in bricks.iter().enumerate() {
        let slots = match layout {
            Layout::Flat => b.layer..b.layer + 1,
            Layout::Upright => b.z..b.z + b.d,
        };
        for r in b.row..b.row + b.h {
            for c in b.col..b.col + b.w {
                for z in slots.clone() {
                    occupant[(r as usize * cols + c as usize) * 2 + z as usize] = i;
                }
            }
        }
    }
    for cell in 0..cols * rows {
        let here = [occupant[cell * 2], occupant[cell * 2 + 1]];
        match layout {
            // Parts are joined where the picture layer sits on the backing.
            Layout::Flat => {
                if here[0] != usize::MAX && here[1] != usize::MAX {
                    groups.union(here[0], here[1]);
                }
            }
            // Parts are joined where one sits on another in the row below.
            Layout::Upright if cell + cols < cols * rows => {
                for z in 0..2 {
                    let below = occupant[(cell + cols) * 2 + z];
                    if here[z] != usize::MAX && below != usize::MAX {
                        groups.union(here[z], below);
                    }
                }
            }
            Layout::Upright => {}
        }
    }
    (groups, occupant)
}

struct UnionFind(Vec<usize>);

impl UnionFind {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.0[a] = b;
    }
}

/// Area-averages the source over each cell and picks the nearest LEGO colour.
fn sample(src: &SourceImage, cols: u32, rows: u32, s: &Settings) -> Vec<Option<u16>> {
    let matcher = Matcher::new(&s.enabled_colours);
    let sx = src.width as f32 / cols as f32;
    let sy = src.height as f32 / rows as f32;
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        let y0 = (row as f32 * sy) as u32;
        let y1 = (((row + 1) as f32 * sy).ceil() as u32).clamp(y0 + 1, src.height);
        for col in 0..cols {
            let x0 = (col as f32 * sx) as u32;
            let x1 = (((col + 1) as f32 * sx).ceil() as u32).clamp(x0 + 1, src.width);
            let mut sum = [0f64; 4];
            for y in y0..y1 {
                let line = (y * src.width) as usize * 4;
                for x in x0..x1 {
                    let p = &src.rgba[line + x as usize * 4..][..4];
                    let a = p[3] as f64 / 255.0;
                    sum[0] += p[0] as f64 * a;
                    sum[1] += p[1] as f64 * a;
                    sum[2] += p[2] as f64 * a;
                    sum[3] += a;
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as f64;
            let coverage = (sum[3] / n) as f32;
            let colour = if coverage >= s.alpha_threshold && sum[3] > 0.0 {
                let rgb = [sum[0], sum[1], sum[2]].map(|v| (v / sum[3]) as f32);
                matcher.nearest(rgb)
            } else {
                None
            };
            out.push(colour);
        }
    }
    out
}

/// Grows the grid by a border of `across` columns and `down` rows on each side, and fills
/// the empty cells near the picture (within a rounded outline) with `colour`.
fn add_border(
    cells: &[Option<u16>],
    cols: u32,
    rows: u32,
    colour: u16,
    across: u32,
    down: u32,
) -> (Vec<Option<u16>>, u32, u32) {
    let (new_cols, new_rows) = (cols + 2 * across, rows + 2 * down);
    let mut out = vec![None; (new_cols * new_rows) as usize];
    for r in 0..rows {
        for c in 0..cols {
            out[((r + down) * new_cols + c + across) as usize] = cells[(r * cols + c) as usize];
        }
    }
    // An ellipse a little over the border size: it includes diagonal neighbours, so the
    // outline never breaks into pieces that only touch at corners.
    let (rx, ry) = (across as f32 + 0.5, down as f32 + 0.5);
    let near_picture = |c: i64, r: i64| {
        (-(down as i64)..=down as i64).any(|dy| {
            (-(across as i64)..=across as i64).any(|dx| {
                let (x, y) = (c + dx, r + dy);
                (dx as f32 / rx).powi(2) + (dy as f32 / ry).powi(2) <= 1.0
                    && x >= 0
                    && y >= 0
                    && x < new_cols as i64
                    && y < new_rows as i64
                    && out[(y * new_cols as i64 + x) as usize].is_some()
            })
        })
    };
    let border: Vec<usize> = (0..new_rows as i64)
        .flat_map(|r| (0..new_cols as i64).map(move |c| (c, r)))
        .filter(|&(c, r)| out[(r * new_cols as i64 + c) as usize].is_none() && near_picture(c, r))
        .map(|(c, r)| (r * new_cols as i64 + c) as usize)
        .collect();
    for i in border {
        out[i] = Some(colour);
    }
    (out, new_cols, new_rows)
}

/// Greedy rectangle packing: at each free cell, place the largest part that fits.
fn merge_flat(
    cells: &[Option<u16>],
    cols: u32,
    rows: u32,
    kind: PartKind,
    layer: u32,
) -> Vec<Brick> {
    let mut used = vec![false; cells.len()];
    let idx = |c: u32, r: u32| (r * cols + c) as usize;
    let fits = |used: &[bool], c: u32, r: u32, w: u32, h: u32, colour: u16| {
        if c + w > cols || r + h > rows {
            return false;
        }
        (r..r + h)
            .all(|rr| (c..c + w).all(|cc| !used[idx(cc, rr)] && cells[idx(cc, rr)] == Some(colour)))
    };
    let mut bricks = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let Some(colour) = cells[idx(c, r)] else {
                continue;
            };
            if used[idx(c, r)] {
                continue;
            }
            let mut placed = (1, 1, kind.part_for(1, 1));
            for &(long, short, part) in kind.catalogue() {
                let orientations = [(long, short), (short, long)];
                if let Some(&(w, h)) = orientations
                    .iter()
                    .find(|&&(w, h)| fits(&used, c, r, w, h, colour))
                {
                    placed = (w, h, part);
                    break;
                }
            }
            let (w, h, part) = placed;
            for rr in r..r + h {
                for cc in c..c + w {
                    used[idx(cc, rr)] = true;
                }
            }
            bricks.push(Brick {
                col: c,
                row: r,
                w,
                h,
                colour,
                part,
                kind,
                layer,
                z: 0,
                d: 1,
            });
        }
    }
    bricks
}

/// Backing plates for a flat mosaic. Like [`merge_flat`], but at each cell it picks the
/// plate whose edges line up least with the picture layer's seams, so that every seam
/// above is bridged by a plate below.
fn merge_backing(
    cells: &[Option<u16>],
    top: &[Brick],
    cols: u32,
    rows: u32,
    colour: u16,
) -> Vec<Brick> {
    let idx = |c: u32, r: u32| (r * cols + c) as usize;
    let mut top_id = vec![usize::MAX; cells.len()];
    for (i, b) in top.iter().enumerate() {
        for r in b.row..b.row + b.h {
            for c in b.col..b.col + b.w {
                top_id[idx(c, r)] = i;
            }
        }
    }
    // A seam between two cells of the picture layer that belong to different parts.
    let seam = |a: usize, b: usize| {
        top_id[a] != usize::MAX && top_id[b] != usize::MAX && top_id[a] != top_id[b]
    };
    let mut used = vec![false; cells.len()];
    let mut bricks = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if cells[idx(c, r)].is_none() || used[idx(c, r)] {
                continue;
            }
            let fits = |w: u32, h: u32| {
                c + w <= cols
                    && r + h <= rows
                    && (r..r + h).all(|rr| {
                        (c..c + w).all(|cc| !used[idx(cc, rr)] && cells[idx(cc, rr)].is_some())
                    })
            };
            // Count the plate's right and bottom edge segments that sit directly under a seam.
            // (Its left and top edges are already fixed by the plates placed before it.)
            // If no backing edge lies under a seam, every seam is bridged and the whole
            // picture holds together, so that matters more than using big plates.
            let aligned = |w: u32, h: u32| {
                let right = (r..r + h)
                    .filter(|&rr| c + w < cols && seam(idx(c + w - 1, rr), idx(c + w, rr)));
                let bottom = (c..c + w)
                    .filter(|&cc| r + h < rows && seam(idx(cc, r + h - 1), idx(cc, r + h)));
                right.count() + bottom.count()
            };
            let (w, h, part) = PartKind::Plate
                .catalogue()
                .iter()
                .flat_map(|&(long, short, part)| [(long, short, part), (short, long, part)])
                .filter(|&(w, h, _)| fits(w, h))
                .min_by_key(|&(w, h, _)| (aligned(w, h), std::cmp::Reverse(w * h)))
                .expect("a 1x1 plate always fits");
            for rr in r..r + h {
                for cc in c..c + w {
                    used[idx(cc, rr)] = true;
                }
            }
            bricks.push(Brick {
                col: c,
                row: r,
                w,
                h,
                colour,
                part,
                kind: PartKind::Plate,
                layer: 1,
                z: 0,
                d: 1,
            });
        }
    }
    bricks
}

/// Finds neighbouring cells whose parts don't hold onto each other (both layers have a
/// seam there) and re-tiles one layer locally so a single part spans the two cells.
/// The backing is normally re-tiled; when the two cells share a colour, the picture layer
/// can be re-tiled instead, which fixes stepped diagonal edges the backing alone can't.
/// Repeats while that helps and returns the best result: (picture, backing).
fn repair(
    cells: &[Option<u16>],
    mut top: Vec<Brick>,
    mut backing: Vec<Brick>,
    cols: u32,
    rows: u32,
) -> (Vec<Brick>, Vec<Brick>) {
    const MAX_REPAIRS: usize = 400;
    let mut best = (usize::MAX, top.clone(), backing.clone());
    // A fixed-seed xorshift generator: random choices escape repairs that undo each other,
    // while the same picture still always gives the same build.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..MAX_REPAIRS {
        let all: Vec<Brick> = top.iter().chain(&backing).cloned().collect();
        let (mut groups, occupant) = part_groups(Layout::Flat, &all, cols, rows);
        let group_count = (0..all.len()).filter(|&i| groups.find(i) == i).count();
        if group_count < best.0 {
            best = (group_count, top.clone(), backing.clone());
        }
        let mut loose = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let a = (r * cols + c) as usize;
                for (ok, b) in [(c + 1 < cols, a + 1), (r + 1 < rows, a + cols as usize)] {
                    if ok
                        && occupant[a * 2] != usize::MAX
                        && occupant[b * 2] != usize::MAX
                        && groups.find(occupant[a * 2]) != groups.find(occupant[b * 2])
                    {
                        loose.push((a, b));
                    }
                }
            }
        }
        if loose.is_empty() {
            break;
        }
        let (a, b) = loose[random() as usize % loose.len()];
        let fix_top = cells[a] == cells[b] && random() % 2 == 1;
        let choice = random();
        if fix_top {
            let kind = top[occupant[a * 2]].kind;
            let rebuilt = bridge(
                cells,
                &top,
                occupant[a * 2],
                occupant[b * 2],
                a,
                b,
                cols,
                rows,
                choice,
                |region| merge_flat(region, cols, rows, kind, 0),
            );
            top = rebuilt;
        } else {
            let n_top = top.len();
            let rebuilt = bridge(
                cells,
                &backing,
                occupant[a * 2 + 1] - n_top,
                occupant[b * 2 + 1] - n_top,
                a,
                b,
                cols,
                rows,
                choice,
                |region| {
                    merge_backing(
                        region,
                        &top,
                        cols,
                        rows,
                        region.iter().flatten().next().copied().unwrap_or(0),
                    )
                },
            );
            backing = rebuilt;
        }
    }
    (best.1, best.2)
}

/// Replaces parts `pa` and `pb` of a layer with a part covering cells `a` and `b` (which
/// must match in colour), plus parts from `retile` for the leftover cells. `choice` picks
/// the covering part: usually the largest, sometimes a random one that fits.
#[allow(clippy::too_many_arguments)]
fn bridge(
    cells: &[Option<u16>],
    layer: &[Brick],
    pa: usize,
    pb: usize,
    a: usize,
    b: usize,
    cols: u32,
    rows: u32,
    choice: u64,
    retile: impl Fn(&[Option<u16>]) -> Vec<Brick>,
) -> Vec<Brick> {
    if pa == pb {
        return layer.to_vec();
    }
    let template = layer[pa].clone();
    let mut region = vec![None; (cols * rows) as usize];
    for p in [&layer[pa], &layer[pb]] {
        for r in p.row..p.row + p.h {
            for c in p.col..p.col + p.w {
                let i = (r * cols + c) as usize;
                // The backing is one colour; the picture keeps each cell's own colour.
                region[i] = if template.layer == 1 {
                    Some(template.colour)
                } else {
                    cells[i]
                };
            }
        }
    }
    let colour = region[a];
    let (ac, ar) = (a as u32 % cols, a as u32 / cols);
    let (bc, br) = (b as u32 % cols, b as u32 / cols);
    let inside = |c0: u32, r0: u32, w: u32, h: u32| {
        c0 + w <= cols
            && r0 + h <= rows
            && (r0..r0 + h).all(|r| (c0..c0 + w).all(|c| region[(r * cols + c) as usize] == colour))
    };
    let mut candidates: Vec<_> = template
        .kind
        .catalogue()
        .iter()
        .flat_map(|&(long, short, part)| [(long, short, part), (short, long, part)])
        .flat_map(|(w, h, part)| {
            let cs = (bc + 1).saturating_sub(w)..=ac;
            cs.flat_map(move |c0| {
                ((br + 1).saturating_sub(h)..=ar).map(move |r0| (c0, r0, w, h, part))
            })
        })
        .filter(|&(c0, r0, w, h, _)| c0 + w > bc && r0 + h > br && inside(c0, r0, w, h))
        .collect();
    // Largest first; a 1x2 part always fits, so this is never empty.
    candidates.sort_by_key(|&(_, _, w, h, _)| std::cmp::Reverse(w * h));
    let pick = if choice.is_multiple_of(3) {
        (choice / 3) as usize % candidates.len()
    } else {
        0
    };
    let (c0, r0, w, h, part) = candidates[pick];
    for r in r0..r0 + h {
        for c in c0..c0 + w {
            region[(r * cols + c) as usize] = None;
        }
    }
    let mut out: Vec<Brick> = layer
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != pa && i != pb)
        .map(|(_, b)| b.clone())
        .collect();
    out.extend(retile(&region));
    out.push(Brick {
        col: c0,
        row: r0,
        w,
        h,
        colour: colour.expect("cell is filled"),
        part,
        ..template
    });
    out
}

/// Upright counterpart of [`repair`]. Wall parts only hold onto the rows above and below,
/// so a loose join between two neighbouring cells is fixed by re-splitting that row, or
/// the row above or below, so that one part spans both columns.
fn repair_wall(mut bricks: Vec<Brick>, cols: u32, rows: u32, kind: PartKind) -> Vec<Brick> {
    const MAX_REPAIRS: usize = 400;
    let lengths = |d: u32| -> Vec<(u32, &'static str)> {
        let mut l: Vec<_> = kind
            .catalogue()
            .iter()
            .filter(|p| p.1 == d)
            .map(|p| (p.0, p.2))
            .collect();
        if d == 2 {
            l.push((1, kind.part_for(2, 1)));
        }
        l
    };
    let mut best = (usize::MAX, bricks.clone());
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..MAX_REPAIRS {
        let (mut groups, occupant) = part_groups(Layout::Upright, &bricks, cols, rows);
        let group_count = (0..bricks.len()).filter(|&i| groups.find(i) == i).count();
        if group_count < best.0 {
            best = (group_count, bricks.clone());
        }
        let at = |c: u32, r: u32, z: u32| occupant[((r * cols + c) * 2 + z) as usize];
        // Loose joins can only be sideways: a part always holds onto the one below it.
        let mut loose = Vec::new();
        for r in 0..rows {
            for c in 0..cols.saturating_sub(1) {
                let (a, b) = (at(c, r, 0), at(c + 1, r, 0));
                if a != usize::MAX && b != usize::MAX && groups.find(a) != groups.find(b) {
                    loose.push((c, r));
                }
            }
        }
        if loose.is_empty() {
            break;
        }
        let (c, r) = loose[random() as usize % loose.len()];
        // Rows and depth slots where one part could span columns c and c + 1.
        let options: Vec<(usize, usize)> =
            [Some(r), r.checked_sub(1), (r + 1 < rows).then_some(r + 1)]
                .into_iter()
                .flatten()
                .flat_map(|row| (0..2).map(move |z| (row, z)))
                .map(|(row, z)| (at(c, row, z), at(c + 1, row, z)))
                .filter(|&(p, q)| {
                    p != usize::MAX
                        && q != usize::MAX
                        && p != q
                        && bricks[p].colour == bricks[q].colour
                        && (bricks[p].z, bricks[p].d) == (bricks[q].z, bricks[q].d)
                })
                .collect();
        if options.is_empty() {
            continue; // e.g. a colour change in a one-row-thick stroke: no part can span it
        }
        let (p, q) = options[random() as usize % options.len()];
        let template = bricks[p].clone();
        let (start, end) = (
            bricks[p].col.min(bricks[q].col),
            (bricks[p].col + bricks[p].w).max(bricks[q].col + bricks[q].w),
        );
        let lengths = lengths(template.d);
        // A part spanning c and c + 1, inside start..end; usually the longest, sometimes another.
        let mut spans: Vec<(u32, u32, &'static str)> = lengths
            .iter()
            .flat_map(|&(len, part)| (start..=c).map(move |x| (x, len, part)))
            .filter(|&(x, len, _)| x + len >= c + 2 && x + len <= end)
            .collect();
        spans.sort_by_key(|s| std::cmp::Reverse(s.1));
        let choice = random();
        let (x, len, part) = spans[if choice % 3 == 0 {
            (choice / 3) as usize % spans.len()
        } else {
            0
        }];
        let mut pieces = split_run(start, x, &lengths, |_| false);
        pieces.push((x, len, part));
        pieces.extend(split_run(x + len, end, &lengths, |_| false));
        let (hi, lo) = (p.max(q), p.min(q));
        bricks.swap_remove(hi);
        bricks.swap_remove(lo);
        bricks.extend(pieces.into_iter().map(|(col, w, part)| Brick {
            col,
            w,
            part,
            ..template.clone()
        }));
    }
    best.1
}

/// Swaps each wall plate with nothing on top of it for a tile of the same size, where that
/// tile exists. Only fully uncovered plates qualify: a tile has no studs for a part to grip.
fn smooth_wall_tops(mut bricks: Vec<Brick>, cols: u32, rows: u32) -> Vec<Brick> {
    let (_, occupant) = part_groups(Layout::Upright, &bricks, cols, rows);
    for b in &mut bricks {
        if b.kind != PartKind::Plate {
            continue;
        }
        let covered = b.row > 0
            && (b.col..b.col + b.w)
                .any(|c| (b.z..b.z + b.d).any(|z| occupant[(((b.row - 1) * cols + c) * 2 + z) as usize] != usize::MAX));
        let (long, short) = (b.w.max(b.d), b.w.min(b.d));
        let tile = PartKind::Tile.catalogue().iter().find(|t| t.0 == long && t.1 == short);
        if let (false, Some(&(_, _, part))) = (covered, tile) {
            b.kind = PartKind::Tile;
            b.part = part;
        }
    }
    bricks
}

/// Splits columns `start..end` into parts with the given lengths (longest first), avoiding
/// part ends where `avoid` is true unless nothing else fits.
fn split_run(
    start: u32,
    end: u32,
    lengths: &[(u32, &'static str)],
    avoid: impl Fn(u32) -> bool,
) -> Vec<(u32, u32, &'static str)> {
    let mut out = Vec::new();
    let mut c = start;
    while c < end {
        let candidates: Vec<_> = lengths.iter().filter(|l| l.0 <= end - c).collect();
        let &&(len, part) = candidates
            .iter()
            .find(|l| c + l.0 == end || !avoid(c + l.0))
            .unwrap_or(&candidates[0]);
        out.push((c, len, part));
        c += len;
    }
    out
}

/// Runs of equal cells in a row, as (start, end, value).
fn runs<T: PartialEq + Copy>(row: &[Option<T>]) -> Vec<(u32, u32, T)> {
    let mut out = Vec::new();
    let mut c = 0;
    while c < row.len() {
        let Some(v) = row[c] else {
            c += 1;
            continue;
        };
        let end = (c..row.len())
            .find(|&cc| row[cc] != Some(v))
            .unwrap_or(row.len());
        out.push((c as u32, end as u32, v));
        c = end;
    }
    out
}

/// Upright walls, built from the bottom row up. Seams are kept off the seams of the row
/// below. A colour change is a seam in every row, though, so with a back layer the wall is
/// two studs deep and alternates:
/// - two-deep rows of picture-coloured parts;
/// - rows with picture parts in front and backing-coloured parts behind that span the
///   picture's seams, tying the two-deep rows above and below together.
fn merge_wall(
    cells: &[Option<u16>],
    cols: u32,
    rows: u32,
    kind: PartKind,
    backing: Option<u16>,
) -> Vec<Brick> {
    let lengths = |short: u32| -> Vec<(u32, &'static str)> {
        kind.catalogue()
            .iter()
            .filter(|p| p.1 == short)
            .map(|p| (p.0, p.2))
            .collect()
    };
    let single = lengths(1);
    let mut double = lengths(2);
    // A 1x2 part turned front-to-back fills a one-wide gap in a two-deep row.
    double.push((1, kind.part_for(2, 1)));

    let mut bricks = Vec::new();
    let mut seams_below = vec![false; cols as usize + 1];
    for (n, r) in (0..rows).rev().enumerate() {
        let row = &cells[(r * cols) as usize..((r + 1) * cols) as usize];
        let mut seams = vec![false; cols as usize + 1];
        let two_deep = backing.is_some() && n % 2 == 0;
        for (start, end, colour) in runs(row) {
            let parts = split_run(start, end, if two_deep { &double } else { &single }, |c| {
                seams_below[c as usize]
            });
            for (c, w, part) in parts {
                let d = if two_deep { 2 } else { 1 };
                bricks.push(Brick {
                    col: c,
                    row: r,
                    w,
                    h: 1,
                    colour,
                    part,
                    kind,
                    layer: 0,
                    z: 0,
                    d,
                });
                seams[c as usize] = true;
                seams[(c + w) as usize] = true;
            }
        }
        if let (Some(colour), false) = (backing, two_deep) {
            let filled: Vec<Option<()>> = row.iter().map(|c| c.map(|_| ())).collect();
            for (start, end, ()) in runs(&filled) {
                for (c, w, part) in split_run(start, end, &single, |c| seams[c as usize]) {
                    bricks.push(Brick {
                        col: c,
                        row: r,
                        w,
                        h: 1,
                        colour,
                        part,
                        kind,
                        layer: 0,
                        z: 1,
                        d: 1,
                    });
                }
            }
        }
        seams_below = seams;
    }
    bricks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test picture: a filled disc split into a red left half and a blue right half,
    /// with a horizontal yellow band, on a transparent background.
    pub(crate) fn test_image() -> SourceImage {
        let (width, height) = (200u32, 160u32);
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let (dx, dy) = (x as f32 - 100.0, y as f32 - 80.0);
                let inside = dx * dx / (95.0 * 95.0) + dy * dy / (75.0 * 75.0) <= 1.0;
                let rgb = if (70..90).contains(&y) {
                    [250, 200, 10]
                } else if x < 100 {
                    [180, 0, 0]
                } else {
                    [30, 90, 170]
                };
                rgba.extend(rgb);
                rgba.push(if inside { 255 } else { 0 });
            }
        }
        SourceImage { width, height, rgba }
    }

    pub(crate) fn wall_settings(kind: PartKind) -> Settings {
        Settings { layout: Layout::Upright, kind, width: 24, ..Settings::default() }
    }

    /// For each part, whether any part sits directly on top of it.
    fn covered(m: &Mosaic) -> Vec<bool> {
        let (_, occupant) = part_groups(m.layout, &m.bricks, m.cols, m.rows);
        m.bricks
            .iter()
            .map(|b| {
                b.row > 0
                    && (b.col..b.col + b.w).any(|c| {
                        (b.z..b.z + b.d)
                            .any(|z| occupant[(((b.row - 1) * m.cols + c) * 2 + z) as usize] != usize::MAX)
                    })
            })
            .collect()
    }

    #[test]
    fn smooth_flat_mosaic_uses_tiles_for_the_picture() {
        let s = Settings { smooth_top: true, ..Settings::default() };
        let m = Mosaic::build(&test_image(), &s);
        assert!(m.bricks.iter().filter(|b| b.layer == 0).all(|b| b.kind == PartKind::Tile));
        assert!(m.bricks.iter().filter(|b| b.layer == 1).all(|b| b.kind == PartKind::Plate));
        let (groups, islands, _) = m.connectivity();
        assert_eq!(groups, islands);
    }

    #[test]
    fn smooth_wall_tiles_only_uncovered_plates() {
        for backing in [None, Some(0)] {
            let s = Settings { smooth_top: true, backing, ..wall_settings(PartKind::Plate) };
            let m = Mosaic::build(&test_image(), &s);
            let covered = covered(&m);
            let mut tiles = 0;
            for (b, &covered) in m.bricks.iter().zip(&covered) {
                let tile_exists = PartKind::Tile
                    .catalogue()
                    .iter()
                    .any(|t| t.0 == b.w.max(b.d) && t.1 == b.w.min(b.d));
                match b.kind {
                    PartKind::Tile => {
                        assert!(!covered, "tile at {},{} has a part on top", b.col, b.row);
                        tiles += 1;
                    }
                    PartKind::Plate => assert!(covered || !tile_exists, "plate at {},{} could be a tile", b.col, b.row),
                    PartKind::Brick => unreachable!(),
                }
            }
            assert!(tiles > 0);
        }
    }

    #[test]
    fn smooth_top_leaves_brick_walls_alone() {
        let s = Settings { smooth_top: true, ..wall_settings(PartKind::Brick) };
        let m = Mosaic::build(&test_image(), &s);
        assert!(m.bricks.iter().all(|b| b.kind == PartKind::Brick));
    }
}
