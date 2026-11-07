//! Pixel art and the palette: pet sprites, icons, colours.

// ---- palette ----
pub(crate) const KEY: u32 = rgb(255, 0, 255); // transparent colour
pub(crate) const INK: u32 = rgb(0x2b, 0x21, 0x40);
pub(crate) const ORANGE: u32 = rgb(0xf2, 0xa6, 0x5a);
pub(crate) const PINK: u32 = rgb(0xff, 0x8f, 0xa3);
pub(crate) const CREAM: u32 = rgb(0xff, 0xf1, 0xd6);
pub(crate) const GREEN: u32 = rgb(0x7e, 0xe0, 0xa0);
pub(crate) const UP_BLUE: u32 = rgb(0x7e, 0xc8, 0xff);

pub(crate) const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

pub(crate) struct Pet {
    pub name: &'static str,
    pub face: u8, // fill colour char used when the eyes blink shut
    pub grid: &'static [&'static [u8]],
}

pub(crate) static PETS: [Pet; 4] = [
    Pet {
        name: "Cat",
        face: b'O',
        grid: &[
            b".KK......KK.",
            b".KOK....KOK.",
            b".KOOKKKKOOK.",
            b"KOOOOOOOOOOK",
            b"KOOEOOOOEOOK",
            b"KOOOOPPOOOOK",
            b"KOWWOOOOWWOK",
            b".KOOOOOOOOK.",
            b".KOOOOOOOOK.",
            b".KWWKOOKWWK.",
            b"..KKK..KKK..",
        ],
    },
    Pet {
        name: "Dog",
        face: b'D',
        grid: &[
            b"..KKKKKKKK..",
            b".KDDDDDDDDK.",
            b"KdKDDDDDDKdK",
            b"KddDEDDEDddK",
            b"KddDDDDDDddK",
            b"KdKDWWWWDKdK",
            b".KKDWKKWDKK.",
            b"..KDWWWWDK..",
            b"..KDDPPDDK..",
            b"...KKKKKKK..",
            b"............",
        ],
    },
    Pet {
        name: "Claude",
        face: b'C',
        grid: &[
            b"............",
            b"..KKKKKKKK..",
            b".KCCCCCCCCK.",
            b".KCCECCECCK.",
            b"KKCCECCECCKK",
            b"KCCCCCCCCCCK",
            b"KKCCCCCCCCKK",
            b".KCCCCCCCCK.",
            b".KCLCCCCLCK.",
            b".KKCKKKKCKK.",
            b"..KK....KK..",
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
];

pub(crate) fn pet_color(c: u8) -> Option<u32> {
    Some(match c {
        b'K' | b'E' => INK,
        b'O' => ORANGE,
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
        _ => return None,
    })
}

pub(crate) const ICON_TASKS: [&[u8]; 6] = [b"......X", b".....XX", b"X...XX.", b"XX.XX..", b".XXX...", b"..X...."];
pub(crate) const ICON_FOCUS: [&[u8]; 7] = [b"..XXX..", b".X...X.", b"X..X..X", b"X..XX.X", b"X.....X", b".X...X.", b"..XXX.."];
pub(crate) const ICON_RECAP: [&[u8]; 7] = [b".....X.", b"..X..X.", b"..X.XX.", b"X.X.XX.", b"XXXXXX.", b"XXXXXX.", b"XXXXXXX"];
pub(crate) const ICON_PETS: [&[u8]; 6] = [b"X.....X", b"XX...XX", b"XXXXXXX", b"X.XXX.X", b"XXXXXXX", b".XXXXX."];
pub(crate) const ICON_OPTS: [&[u8]; 6] = [b"XXXXXXX", b"..X....", b"XXXXXXX", b"....X..", b"XXXXXXX", b"..X...."];
pub(crate) const ICON_QUIT: [&[u8]; 6] = [b"...X...", b"X..X..X", b"X..X..X", b"X.....X", b".X...X.", b"..XXX.."];
pub(crate) const HEART: [&[u8]; 5] = [b".X.X.", b"XXXXX", b"XXXXX", b".XXX.", b"..X.."];
pub(crate) const SPARK: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
pub(crate) const DROP: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
pub(crate) const ARROW: [&[u8]; 7] = [b"...X...", b"...X...", b"...X...", b"XXXXXXX", b".XXXXX.", b"..XXX..", b"...X..."];

pub(crate) const ALERT_ORANGE: u32 = rgb(0xff, 0xb4, 0x54);
pub(crate) const BTN_GREEN: u32 = rgb(0x2e, 0x8b, 0x57);
pub(crate) const BTN_GRAY: u32 = rgb(0xb8, 0xb3, 0xc9);
pub(crate) const TOMATO: u32 = rgb(0xff, 0x6b, 0x6b);

pub(crate) const PANEL: u32 = rgb(0x1a, 0x14, 0x28);
pub(crate) const PANEL_LIGHT: u32 = rgb(0x3d, 0x31, 0x60);
pub(crate) const PANEL_EDGE: u32 = rgb(0x5a, 0x4a, 0x85);
pub(crate) const DIM: u32 = rgb(0x8a, 0x7f, 0xb0);
pub(crate) const HINT: u32 = rgb(0xa8, 0x9c, 0xc8);
pub(crate) const RED: u32 = rgb(0xff, 0x78, 0x78);
pub(crate) const BTN_RED: u32 = rgb(0xa8, 0x32, 0x3e);

pub(crate) const CHECK: [&[u8]; 5] = [b".....X", b"X...XX", b"XX.XX.", b".XXX..", b"..X..."];
pub(crate) const CROSS: [&[u8]; 7] = [b"X.....X", b".X...X.", b"..X.X..", b"...X...", b"..X.X..", b".X...X.", b"X.....X"];
