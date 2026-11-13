//! Pixel art and the palette: pet sprites, icons, colours.

// ---- palette ----
pub(crate) const KEY: u32 = rgb(255, 0, 255); // transparent colour
pub(crate) const PINK: u32 = rgb(0xff, 0x8f, 0xa3);
pub(crate) const CREAM: u32 = rgb(0xff, 0xf1, 0xd6);
pub(crate) const GREEN: u32 = rgb(0x7e, 0xe0, 0xa0);
pub(crate) const UP_BLUE: u32 = rgb(0x7e, 0xc8, 0xff);

/// Pet outline and eyes: fixed, so every pet looks the same in every theme.
pub(crate) const PET_INK: u32 = rgb(0x2b, 0x21, 0x40);

/// The colours that make up the look of the windows. Meaning colours (green = good, red = today,
/// pink = AI) stay fixed across themes; these are the surfaces and the accent.
pub(crate) struct Theme {
    pub name: &'static str,
    pub ink: u32,         // window and card background
    pub accent: u32,      // borders, titles
    pub panel: u32,       // inner panels
    pub panel_light: u32, // hover and separators
    pub panel_edge: u32,  // panel borders
    pub dim: u32,         // quiet labels
    pub hint: u32,        // secondary text
}

pub(crate) static THEMES: [Theme; 5] = [
    Theme { name: "Night", ink: rgb(0x2b, 0x21, 0x40), accent: rgb(0xf2, 0xa6, 0x5a), panel: rgb(0x1a, 0x14, 0x28), panel_light: rgb(0x3d, 0x31, 0x60), panel_edge: rgb(0x5a, 0x4a, 0x85), dim: rgb(0x8a, 0x7f, 0xb0), hint: rgb(0xa8, 0x9c, 0xc8) },
    Theme { name: "Ocean", ink: rgb(0x14, 0x28, 0x3a), accent: rgb(0x5a, 0xd1, 0xf2), panel: rgb(0x0e, 0x1a, 0x28), panel_light: rgb(0x22, 0x45, 0x6b), panel_edge: rgb(0x3a, 0x6a, 0x9a), dim: rgb(0x7a, 0x9f, 0xc0), hint: rgb(0x9c, 0xc0, 0xdc) },
    Theme { name: "Forest", ink: rgb(0x1c, 0x2f, 0x22), accent: rgb(0x9a, 0xd6, 0x5a), panel: rgb(0x12, 0x20, 0x1a), panel_light: rgb(0x2b, 0x4d, 0x36), panel_edge: rgb(0x46, 0x8a, 0x5a), dim: rgb(0x7f, 0xb0, 0x8f), hint: rgb(0xa0, 0xcd, 0xb0) },
    Theme { name: "Candy", ink: rgb(0x3a, 0x1f, 0x3f), accent: rgb(0xff, 0x7a, 0xc8), panel: rgb(0x28, 0x14, 0x30), panel_light: rgb(0x5a, 0x2f, 0x66), panel_edge: rgb(0x8a, 0x4a, 0x9f), dim: rgb(0xb0, 0x7f, 0xb8), hint: rgb(0xcc, 0x9c, 0xd0) },
    Theme { name: "Mono", ink: rgb(0x24, 0x26, 0x2b), accent: rgb(0xd8, 0xd8, 0xd8), panel: rgb(0x17, 0x18, 0x1c), panel_light: rgb(0x3a, 0x3d, 0x45), panel_edge: rgb(0x5c, 0x60, 0x6b), dim: rgb(0x8a, 0x8e, 0x99), hint: rgb(0xae, 0xb2, 0xbd) },
];

static THEME_NOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(crate) fn theme() -> &'static Theme {
    &THEMES[THEME_NOW.load(std::sync::atomic::Ordering::Relaxed).min(THEMES.len() - 1)]
}

pub(crate) fn set_theme(i: usize) {
    THEME_NOW.store(i.min(THEMES.len() - 1), std::sync::atomic::Ordering::Relaxed);
}

/// Window opacity choices (percent) for the idle pet.
pub(crate) const OPACITIES: [u32; 4] = [100, 85, 70, 50];

pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

pub(crate) struct Pet {
    pub name: &'static str,
    pub face: u8, // fill colour char used when the eyes blink shut
    pub grid: &'static [&'static [u8]],
}

pub(crate) static PETS: [Pet; 5] = [
    Pet {
        name: "Cat",
        face: b'O',
        grid: &[
            b"..KK........KK..",
            b".KOOK......KOOK.",
            b".KOPOKKKKKKOPOK.",
            b".KOOOOOOOOOOOOK.",
            b"KOOOoOOooOOoOOOK",
            b"KOOOOOOOOOOOOOOK",
            b"KOOEEOOOOOOEEOOK",
            b"KOOEEOOOOOOEEOOK",
            b"KOZOOOOPPOOOOZOK",
            b"KOOWWOOKKOOWWOOK",
            b".KOOWWWWWWWWOOK.",
            b"..KOOOOOOOOOOK..",
            b"..KWWKOOOOKWWK..",
            b"...KKKKKKKKKK...",
        ],
    },
    Pet {
        name: "Dog",
        face: b'D',
        grid: &[
            b"...KKKKKKKKKK...",
            b"..KDDDDDDDDDDK..",
            b".KdKDDDDDDDDKdK.",
            b"KddKDDDDDDDDKddK",
            b"KddKDEEDDEEDKddK",
            b"KddKDEEDDEEDKddK",
            b"KddKDDDDDDDDKddK",
            b".KdKDWWWWWWDKdK.",
            b"..KKDWKKKKWDKK..",
            b"...KDWWPPWWDK...",
            b"...KDDWPPWDDK...",
            b"...KWWKKKKWWK...",
            b"....KKK..KKK....",
        ],
    },
    Pet {
        name: "Mochi",
        face: b'W',
        grid: &[
            b"....KKKK....",
            b"..KKWWWWKK..",
            b".KWWWWWWWWK.",
            b".KWWEWWEWWK.",
            b"KWWWEWWEWWWK",
            b"KWZZWWWWZZWK",
            b"KWWWWPPWWWWK",
            b".KWWWWWWWWK.",
            b".KKWWWWWWKK.",
            b"..KKKKKKKK..",
            b"..KPK..KPK..",
        ],
    },
    Pet {
        name: "Agent",
        face: b'S',
        grid: &[
            b"....KKKKKKKK....",
            b"...KHHHHHHHHK...",
            b"..KHHhhHHHHHHK..",
            b"..KHHHHHHHHHHK..",
            b"..KHSSSSSSSSHK..",
            b"..KSSSSSSSSSSK..",
            b"..KBGBBKKBGBBK..",
            b"..KSBBBSSBBBSK..",
            b"..KSSSSsSSSSSK..",
            b"..KSSSSSSKKSSK..",
            b"...KsSSSSSSsK...",
            b"....KKWWWWKK....",
            b"..KTTWWRRWWTTK..",
            b".KTtTWWrrWWTtTK.",
            b"KTTtTWWWWWWTPTTK",
        ],
    },
    Pet {
        name: "Super Car",
        face: b'G',
        grid: &[
            b"................",
            b"......KKKKK.....",
            b"..KK.KGGGGGKK...",
            b"..KRKGGEGGEGGKK.",
            b"KKKRRKGEGGEGKRRK",
            b"KRRRRRRRRRRRRRRK",
            b"KrWWWWWWWWWRRRYK",
            b".KKRRRRRRRRRRKK.",
            b"..KNNK....KNNK..",
            b"..KNVNK..KNVNK..",
            b"...KKK....KKK...",
        ],
    },
];

#[cfg(test)]
mod theme_tests {
    use super::*;

    fn lum(c: u32) -> f64 {
        let ch = |v: u32| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c & 0xff) + 0.7152 * ch((c >> 8) & 0xff) + 0.0722 * ch((c >> 16) & 0xff)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_is_readable() {
        let mut names = Vec::new();
        for t in THEMES.iter() {
            assert!(!names.contains(&t.name), "duplicate theme {}", t.name);
            names.push(t.name);
            for c in [t.ink, t.accent, t.panel, t.panel_light, t.panel_edge, t.dim, t.hint] {
                assert_ne!(c, KEY, "{} uses the transparent colour", t.name);
            }
            assert!(contrast(CREAM, t.ink) >= 7.0, "{}: text on window", t.name);
            assert!(contrast(CREAM, t.panel) >= 7.0, "{}: text on panel", t.name);
            assert!(contrast(t.accent, t.ink) >= 4.5, "{}: accent on window", t.name);
            assert!(contrast(t.ink, t.accent) >= 4.5, "{}: title text on accent", t.name);
            assert!(contrast(t.hint, t.panel) >= 4.5, "{}: hint text", t.name);
            assert!(contrast(t.dim, t.panel) >= 3.0, "{}: dim labels", t.name);
            assert!(contrast(t.panel_light, t.panel) >= 1.3, "{}: hover must be visible", t.name);
        }
    }
}

#[cfg(test)]
mod pet_tests {
    use super::*;

    #[test]
    fn every_pet_is_a_clean_rectangle_that_fits_and_only_uses_known_colours() {
        for p in PETS.iter() {
            let w = p.grid[0].len();
            assert!(w <= 16 && p.grid.len() <= 15, "{} is {}x{}", p.name, w, p.grid.len());
            for (j, row) in p.grid.iter().enumerate() {
                assert_eq!(row.len(), w, "{} row {j} has a different width", p.name);
                for &ch in row.iter() {
                    assert!(ch == b'.' || pet_color(ch).is_some(), "{} uses unknown colour {}", p.name, ch as char);
                }
            }
            assert!(pet_color(p.face).is_some(), "{} face colour", p.name);
        }
    }
}

pub(crate) fn pet_color(c: u8) -> Option<u32> {
    Some(match c {
        b'K' | b'E' => PET_INK,
        b'O' => rgb(0xf2, 0xa6, 0x5a),
        b'P' => PINK,
        b'W' => CREAM,
        b'D' => rgb(0xbf, 0x82, 0x4f),
        b'd' => rgb(0x7a, 0x4b, 0x2a),
        b'C' => rgb(0xd9, 0x77, 0x57),
        b'L' => rgb(0xf0, 0xa5, 0x8a),
        b'S' => rgb(0xf5, 0xc9, 0xa0),
        b'H' => rgb(0x4a, 0x34, 0x26),
        b'T' => rgb(0x3a, 0x3f, 0x5c),
        b'R' => rgb(0xd6, 0x40, 0x4e),
        b'r' => rgb(0x9c, 0x2a, 0x36),
        b'h' => rgb(0x6b, 0x4a, 0x36),
        b's' => rgb(0xd9, 0xa5, 0x77),
        b'B' => rgb(0x0d, 0x0b, 0x14),
        b'G' => rgb(0xdd, 0xe6, 0xff),
        b't' => rgb(0x50, 0x57, 0x80),
        b'o' => rgb(0xc9, 0x7b, 0x2f), // darker orange (tabby stripes)
        b'Y' => rgb(0xff, 0xe0, 0x66), // headlight
        b'N' => rgb(0x55, 0x58, 0x6e), // tyre
        b'V' => rgb(0xb0, 0xb6, 0xc8), // silver
        b'Z' => rgb(0xff, 0x9d, 0xa8), // blush
        _ => return None,
    })
}

pub(crate) const ICON_TASKS: [&[u8]; 6] = [b"......X", b".....XX", b"X...XX.", b"XX.XX..", b".XXX...", b"..X...."];
pub(crate) const ICON_FOCUS: [&[u8]; 7] = [b"..XXX..", b".X...X.", b"X..X..X", b"X..XX.X", b"X.....X", b".X...X.", b"..XXX.."];
pub(crate) const ICON_RECAP: [&[u8]; 7] = [b".....X.", b"..X..X.", b"..X.XX.", b"X.X.XX.", b"XXXXXX.", b"XXXXXX.", b"XXXXXXX"];
pub(crate) const ICON_PETS: [&[u8]; 6] = [b"X.....X", b"XX...XX", b"XXXXXXX", b"X.XXX.X", b"XXXXXXX", b".XXXXX."];
pub(crate) const ICON_OPTS: [&[u8]; 6] = [b"XXXXXXX", b"..X....", b"XXXXXXX", b"....X..", b"XXXXXXX", b"..X...."];
pub(crate) const ICON_NAP: [&[u8]; 7] = [b"XXXXXXX", b".....X.", b"....X..", b"...X...", b"..X....", b".X.....", b"XXXXXXX"];
pub(crate) const ICON_QUIT: [&[u8]; 6] = [b"...X...", b"X..X..X", b"X..X..X", b"X.....X", b".X...X.", b"..XXX.."];
pub(crate) const HEART: [&[u8]; 5] = [b".X.X.", b"XXXXX", b"XXXXX", b".XXX.", b"..X.."];
pub(crate) const SPARK: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
pub(crate) const DROP: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
pub(crate) const ARROW: [&[u8]; 7] = [b"...X...", b"...X...", b"...X...", b"XXXXXXX", b".XXXXX.", b"..XXX..", b"...X..."];

pub(crate) const ALERT_ORANGE: u32 = rgb(0xff, 0xb4, 0x54);
pub(crate) const BTN_GREEN: u32 = rgb(0x2e, 0x8b, 0x57);
pub(crate) const BTN_GRAY: u32 = rgb(0xb8, 0xb3, 0xc9);
pub(crate) const TOMATO: u32 = rgb(0xff, 0x6b, 0x6b);

pub(crate) const RED: u32 = rgb(0xff, 0x78, 0x78);
pub(crate) const BTN_RED: u32 = rgb(0xa8, 0x32, 0x3e);

pub(crate) const CHECK: [&[u8]; 5] = [b".....X", b"X...XX", b"XX.XX.", b".XXX..", b"..X..."];
pub(crate) const CROSS: [&[u8]; 7] = [b"X.....X", b".X...X.", b"..X.X..", b"...X...", b"..X.X..", b".X...X.", b"X.....X"];
