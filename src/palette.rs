//! Solid LDraw colours (codes and RGB values from LDConfig.ldr) and nearest-colour matching.

#[derive(Clone, Copy, Debug)]
pub struct LegoColour {
    pub code: u16,
    pub name: &'static str,
    pub rgb: [u8; 3],
}

const fn c(code: u16, name: &'static str, hex: u32) -> LegoColour {
    LegoColour {
        code,
        name,
        rgb: [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8],
    }
}

pub const COLOURS: &[LegoColour] = &[
    c(0, "Black", 0x1B2A34),
    c(1, "Blue", 0x1E5AA8),
    c(2, "Green", 0x00852B),
    c(3, "Dark Turquoise", 0x069D9F),
    c(4, "Red", 0xB40000),
    c(5, "Dark Pink", 0xD3359D),
    c(6, "Brown", 0x543324),
    c(7, "Light Grey", 0x8A928D),
    c(8, "Dark Grey", 0x545955),
    c(9, "Light Blue", 0x97CBD9),
    c(10, "Bright Green", 0x58AB41),
    c(11, "Light Turquoise", 0x00AAA4),
    c(12, "Salmon", 0xF06D61),
    c(13, "Pink", 0xF6A9BB),
    c(14, "Yellow", 0xFAC80A),
    c(15, "White", 0xF4F4F4),
    c(17, "Light Green", 0xADD9A8),
    c(18, "Light Yellow", 0xFFD67F),
    c(19, "Tan", 0xD7BA8C),
    c(20, "Light Violet", 0xAFBED6),
    c(22, "Purple", 0x671F81),
    c(23, "Dark Blue Violet", 0x0E3E9A),
    c(25, "Orange", 0xD67923),
    c(26, "Magenta", 0x901F76),
    c(27, "Lime", 0xA5CA18),
    c(28, "Dark Tan", 0x897D62),
    c(29, "Bright Pink", 0xFF9ECD),
    c(30, "Medium Lavender", 0xA06EB9),
    c(31, "Lavender", 0xCDA4DE),
    c(68, "Very Light Orange", 0xFDC383),
    c(69, "Bright Reddish Lilac", 0x8A12A8),
    c(70, "Reddish Brown", 0x5F3109),
    c(71, "Light Bluish Grey", 0x969696),
    c(72, "Dark Bluish Grey", 0x646464),
    c(73, "Medium Blue", 0x7396C8),
    c(74, "Medium Green", 0x7FC475),
    c(77, "Light Pink", 0xFECCCF),
    c(78, "Light Nougat", 0xFFC995),
    c(84, "Medium Nougat", 0xAA7D55),
    c(85, "Medium Lilac", 0x441A91),
    c(86, "Medium Brown", 0x7B5D41),
    c(89, "Blue Violet", 0x1C58A7),
    c(92, "Nougat", 0xBB805A),
    c(110, "Bright Bluish Violet", 0x4354A3),
    c(112, "Medium Bluish Violet", 0x6874AC),
    c(115, "Medium Yellowish Green", 0xB7D425),
    c(118, "Aqua", 0xB3D7D1),
    c(120, "Light Lime", 0xD9E4A7),
    c(125, "Light Orange", 0xF9A777),
    c(151, "Very Light Bluish Grey", 0xC8C8C8),
    c(191, "Bright Light Orange", 0xFCAC00),
    c(212, "Bright Light Blue", 0x9DC3F7),
    c(226, "Bright Light Yellow", 0xFFEC6C),
    c(272, "Dark Blue", 0x19325A),
    c(288, "Dark Green", 0x00451A),
    c(308, "Dark Brown", 0x352100),
    c(320, "Dark Red", 0x720012),
    c(321, "Dark Azure", 0x469BC3),
    c(322, "Medium Azure", 0x68C3E2),
    c(323, "Light Aqua", 0xD3F2EA),
    c(326, "Yellowish Green", 0xE2F99A),
    c(330, "Olive Green", 0x77774E),
    c(353, "Coral", 0xFF6D77),
    c(366, "Earth Orange", 0xD86D2C),
    c(373, "Sand Purple", 0x75657D),
    c(378, "Sand Green", 0x708E7C),
    c(379, "Sand Blue", 0x70819A),
    c(450, "Fabuland Brown", 0xD27744),
    c(462, "Medium Orange", 0xF58624),
    c(484, "Dark Orange", 0x91501C),
    c(503, "Very Light Grey", 0xBCB4A5),
];

pub fn by_code(code: u16) -> Option<&'static LegoColour> {
    COLOURS.iter().find(|c| c.code == code)
}

/// sRGB (0..255) to CIE L*a*b* (D65).
pub fn to_lab(rgb: [f32; 3]) -> [f32; 3] {
    fn lin(v: f32) -> f32 {
        let v = v / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    fn f(t: f32) -> f32 {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    }
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Matches colours against an enabled subset of [`COLOURS`].
pub struct Matcher {
    entries: Vec<(u16, [f32; 3])>,
}

impl Matcher {
    pub fn new(enabled: &[u16]) -> Self {
        let entries = COLOURS
            .iter()
            .filter(|c| enabled.contains(&c.code))
            .map(|c| (c.code, to_lab(c.rgb.map(f32::from))))
            .collect();
        Self { entries }
    }

    /// The nearest colour to a L*a*b* value, with that colour's own L*a*b* value.
    pub fn nearest_lab(&self, lab: [f32; 3]) -> Option<(u16, [f32; 3])> {
        self.entries
            .iter()
            .min_by(|a, b| dist2(a.1, lab).total_cmp(&dist2(b.1, lab)))
            .copied()
    }

    /// The nearest colour to `target` among those within `slack` (ΔE) of the best match for
    /// `original`. Dithering uses this so it only mixes colours close to the original.
    pub fn nearest_lab_near(
        &self,
        target: [f32; 3],
        original: [f32; 3],
        slack: f32,
    ) -> Option<(u16, [f32; 3])> {
        let best = self.nearest_lab(original)?;
        let limit = dist2(best.1, original).sqrt() + slack;
        self.entries
            .iter()
            .filter(|e| dist2(e.1, original).sqrt() <= limit)
            .min_by(|a, b| dist2(a.1, target).total_cmp(&dist2(b.1, target)))
            .copied()
    }
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Colour codes enabled by default: the commonly available solid colours.
pub fn default_enabled() -> Vec<u16> {
    vec![
        0, 1, 2, 4, 5, 10, 14, 15, 19, 25, 26, 27, 28, 70, 71, 72, 73, 78, 84, 191, 212, 226, 272,
        288, 308, 320, 321, 322, 326, 378, 379, 484, 462, 353,
    ]
}

/// Lightness, chroma and hue angle (degrees) in CIE LCh.
pub fn lch(rgb: [u8; 3]) -> [f32; 3] {
    let [l, a, b] = to_lab(rgb.map(f32::from));
    [l, a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0)]
}

/// Colour families, in display order.
pub const FAMILIES: &[&str] = &[
    "White, greys and black",
    "Reds",
    "Oranges",
    "Yellows",
    "Greens",
    "Turquoises",
    "Blues",
    "Purples",
    "Pinks",
    "Browns",
];

/// Index into [`FAMILIES`], judged from the colour itself because LEGO names are a poor guide.
pub fn family(c: &LegoColour) -> usize {
    let [l, chroma, hue] = lch(c.rgb);
    if chroma < 10.0 {
        return 0;
    }
    match hue {
        h if h >= 330.0 || h < 20.0 => 8,
        h if h < 48.0 => 1,
        // Dull or dark oranges and yellows are browns and tans.
        h if h < 95.0 && (l < 45.0 || chroma < 32.0) => 9,
        h if h < 80.0 => 2,
        h if h < 105.0 => 3,
        h if h < 170.0 => 4,
        h if h < 215.0 => 5,
        h if h < 300.0 => 6,
        _ => 7,
    }
}

pub fn is_neutral(c: &LegoColour) -> bool {
    family(c) == 0
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// By family (reds, oranges, …), light to dark within each.
    Family,
    /// Around the colour wheel in 15° steps, light to dark within each. Neutrals first.
    Hue,
}

/// All colours in the given order.
pub fn sorted(order: SortOrder) -> Vec<&'static LegoColour> {
    let mut colours: Vec<_> = COLOURS.iter().collect();
    let key = |c: &LegoColour| {
        let [l, _, hue] = lch(c.rgb);
        let group = match order {
            SortOrder::Family => family(c),
            // Start the wheel at red (about 20° in LCh), so pinks come last.
            SortOrder::Hue if is_neutral(c) => 0,
            SortOrder::Hue => 1 + ((hue - 20.0).rem_euclid(360.0) / 15.0) as usize,
        };
        (group, -(l * 100.0) as i32)
    };
    colours.sort_by_key(|c| key(c));
    colours
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(order: SortOrder) -> Vec<&'static str> {
        sorted(order).iter().map(|c| c.name).collect()
    }

    fn pos(list: &[&str], name: &str) -> usize {
        list.iter().position(|n| *n == name).unwrap()
    }

    #[test]
    fn family_order_groups_families_light_to_dark() {
        let list = names(SortOrder::Family);
        // Each family is one contiguous block, light to dark within it.
        let colours = sorted(SortOrder::Family);
        for pair in colours.windows(2) {
            let (a, b) = (family(pair[0]), family(pair[1]));
            assert!(a <= b, "{} before {}", pair[0].name, pair[1].name);
            if a == b {
                assert!(
                    lch(pair[0].rgb)[0] >= lch(pair[1].rgb)[0],
                    "{} before {}",
                    pair[0].name,
                    pair[1].name
                );
            }
        }
        assert!(pos(&list, "Bright Light Blue") < pos(&list, "Blue"));
        assert!(pos(&list, "Blue") < pos(&list, "Dark Blue"));
        assert!(pos(&list, "Light Green") < pos(&list, "Green"));
        assert!(pos(&list, "Green") < pos(&list, "Dark Green"));
        assert_eq!(list[0], "White");
        assert_eq!(family(by_code(0).unwrap()), 0, "black is a neutral");
    }

    #[test]
    fn hue_order_starts_with_neutrals_then_walks_the_wheel() {
        let colours = sorted(SortOrder::Hue);
        let neutrals = colours.iter().take_while(|c| is_neutral(c)).count();
        assert_eq!(neutrals, COLOURS.iter().filter(|c| is_neutral(c)).count());
        let hues: Vec<f32> = colours[neutrals..]
            .iter()
            .map(|c| (lch(c.rgb)[2] - 20.0).rem_euclid(360.0))
            .collect();
        for pair in hues.windows(2) {
            assert!((pair[0] / 15.0).floor() <= (pair[1] / 15.0).floor());
        }
    }
}
