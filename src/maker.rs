//! The character maker: draw a pet, choose what it can do, and have it. What is made is a pack like
//! any other, "my-characters", so it can be given away as a file.

use crate::prelude::*;
use crate::tools::{self, close_rect, common, create, fit_edit, kk, painted, toggle, work_on, CLOSE};
use std::fmt::Write;

pub(crate) static WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static EDIT: AtomicIsize = AtomicIsize::new(0);
static HOVER: AtomicI32 = AtomicI32::new(-1);
static DESK: Mutex<Option<Desk>> = Mutex::new(None);

pub(crate) const ID: &str = "my-characters";
const HEAD: (usize, usize) = (16, 13);
const BODY: (usize, usize) = (12, 5);
const MOST: usize = 64;

/// The colours to paint with; E is an eye.
const PALETTE: &[u8] = b"KEiGwWSsILPpZRreCOoDdMHhlYyzAagjJbuUckqvFfTtxNnmVB";
const COLS: usize = 17;

const GIFTS: [(&str, &str, Gift); 7] = [
    ("NONE", "none", Gift::None),
    ("FLIES", "fly", Gift::Fly),
    ("LEAPS", "leap", Gift::Leap),
    ("SPINS THREADS", "web", Gift::Web),
    ("SWINGS ON A CABLE", "cable", Gift::Cable),
    ("LONG ARMS", "limb", Gift::Limb),
    ("IS SUDDENLY THERE", "warp", Gift::Warp),
];
const BACKS: [&str; 5] = ["NOTHING", "A CAPE", "WINGS", "SWORDS", "FOUR MORE ARMS"];
const TAILS: [(&str, i32); 3] = [("NONE", 0), ("SHORT", 3), ("LONG", 6)];
const VOICES: [(&str, &str, usize); 5] = [("ALOOF", "aloof", 0), ("EAGER", "eager", 1), ("GENTLE", "gentle", 2), ("FORMAL", "formal", 3), ("MACHINE", "machine", VOICE_MACHINE)];
const LIKES: [(&str, [&str; 3]); 5] = [("SWEETS", ["candy", "cake", "donut"]), ("MEALS", ["pizza", "meat", "ramen"]), ("FRUIT", ["apple", "carrot", "water"]), ("FISH AND MILK", ["fish", "milk", "cookie"]), ("FUEL", ["fuel", "battery", "oil"])];

// ---- rules (pure, so they can be tested) ----

type Grid = Vec<Vec<u8>>;

fn blank((w, h): (usize, usize)) -> Grid {
    vec![vec![b'.'; w]; h]
}

/// A character being made.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Draft {
    pub id: String, // empty until it is kept
    pub name: String,
    head: Grid,
    body: Grid,
    arms: [u8; 2],
    legs: [u8; 2],
    glow: u8,
    back: usize,
    back_c: u8,
    tail: usize,
    tail_c: u8,
    gift: usize,
    tricks: [usize; 3], // numbers in ACTS
    voice: usize,
    likes: usize,
}

/// The letter whose colour is closest.
fn letter(c: u8) -> u8 {
    if c == b'.' || PALETTE.contains(&c) {
        return c;
    }
    let Some(v) = pet_color(c) else { return b'K' };
    let far = |a: u32, b: u32| [0, 8, 16].iter().map(|s| ((a >> s & 0xff) as i32 - (b >> s & 0xff) as i32).pow(2)).sum::<i32>();
    PALETTE.iter().copied().filter(|p| !b"KE".contains(p)).min_by_key(|p| far(pet_color(*p).unwrap_or(0), v)).unwrap_or(b'K')
}

fn act(name: &str) -> usize {
    ACTS.iter().position(|a| a.0 == name).unwrap_or(0)
}

impl Draft {
    pub fn new() -> Draft {
        let mut d = Draft {
            id: String::new(),
            name: String::new(),
            head: blank(HEAD),
            body: blank(BODY),
            arms: [b'O', b'W'],
            legs: [b'O', b'W'],
            glow: b'Y',
            back: 0,
            back_c: b'R',
            tail: 0,
            tail_c: b'O',
            gift: 0,
            tricks: [act("hop"), act("spin"), act("wave")],
            voice: 1,
            likes: 0,
        };
        // something to start from: a round head with a face, and a body
        for (y, row) in [".....KKKKKK.....", "...KKOOOOOOKK...", "..KOOOOOOOOOOK..", ".KOOOOOOOOOOOOK.", ".KOOOOOOOOOOOOK.", ".KOOEEOOOOEEOOK.", ".KOOEEOOOOEEOOK.", ".KOZOOOKKOOOZOK.", "..KOOOOOOOOOOK..", "...KKKKKKKKKK..."].iter().enumerate() {
            d.head[y + 3] = row.bytes().collect();
        }
        for (y, row) in ["..KKKKKKKK..", ".KOOWWWWOOK.", ".KOOWWWWOOK.", ".KOOOOOOOOK.", "..KKKKKKKK.."].iter().enumerate() {
            d.body[y] = row.bytes().collect();
        }
        d
    }

    /// A character that is there, if it fits the maker.
    pub fn of(p: &Pet) -> Option<Draft> {
        let arms = p.arms.as_ref()?;
        let put = |g: &[&[u8]], (w, h): (usize, usize)| -> Option<Grid> {
            let gw = g.first().map_or(0, |r| r.len());
            if gw > w || g.len() > h || (w - gw) % 2 != 0 {
                return None;
            }
            let mut out = blank((w, h));
            let (dx, dy) = ((w - gw) / 2, h - g.len());
            for (y, row) in g.iter().enumerate() {
                for (x, c) in row.iter().enumerate() {
                    out[y + dy][x + dx] = letter(*c);
                }
            }
            Some(out)
        };
        if p.build != Build::Legs || p.torso.is_empty() {
            return None;
        }
        let (back, back_c) = match p.gear.back {
            Back::Cape(c) => (1, c),
            Back::Wings(c, _) => (2, c),
            Back::Swords => (3, b'R'),
            Back::Limbs(c, _) => (4, c),
            _ => (0, b'R'),
        };
        let mut tricks = [act("hop"), act("spin"), act("wave")];
        for (t, c) in tricks.iter_mut().zip(p.tricks) {
            *t = ACTS.iter().position(|a| a.1 == *c).unwrap_or(*t);
        }
        let likes = LIKES.iter().position(|(_, l)| l.iter().filter(|f| food_named(f).is_some_and(|f| p.foods.contains(&f))).count() >= 2).unwrap_or(0);
        Some(Draft {
            id: String::new(),
            name: p.name.to_string(),
            head: put(p.head, HEAD)?,
            body: put(p.torso, BODY)?,
            arms: [letter(arms.c), letter(arms.hand)],
            legs: [letter(p.legs.c), letter(p.legs.foot)],
            glow: letter(p.gear.glow),
            back,
            back_c: letter(back_c),
            tail: p.tail.as_ref().map_or(0, |t| if t.len > 4 { 2 } else { 1 }),
            tail_c: p.tail.as_ref().map_or(letter(p.legs.c), |t| letter(t.c)),
            gift: GIFTS.iter().position(|g| g.2 == p.gear.gift).unwrap_or(0),
            tricks,
            voice: VOICES.iter().position(|v| v.2 == p.voice).unwrap_or(1),
            likes,
        })
    }

    /// Without the empty rows above and below.
    fn trimmed(g: &Grid) -> Grid {
        let full = |r: &Vec<u8>| r.iter().any(|c| *c != b'.');
        let (a, b) = (g.iter().position(full), g.iter().rposition(full));
        match (a, b) {
            (Some(a), Some(b)) => g[a..=b].to_vec(),
            _ => Vec::new(),
        }
    }

    /// The colour behind the eyes: what is most often beside them.
    fn face(&self) -> u8 {
        let mut count = [0u32; 256];
        for (y, row) in self.head.iter().enumerate() {
            for (x, c) in row.iter().enumerate() {
                if *c != b'E' {
                    continue;
                }
                for (i, j) in [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                    let n = self.head.get((y as i32 + j) as usize).and_then(|r| r.get((x as i32 + i) as usize));
                    if let Some(n) = n.filter(|n| !b".KE".contains(n)) {
                        count[*n as usize] += 1;
                    }
                }
            }
        }
        (0..=255u8).max_by_key(|c| (count[*c as usize], *c == b'W')).filter(|c| count[*c as usize] > 0).unwrap_or(b'W')
    }

    /// What is still missing.
    pub fn missing(&self) -> Option<&'static str> {
        let n = self.name.trim();
        if n.is_empty() {
            Some("Give it a name first.")
        } else if n.chars().count() > 12 || n.chars().any(char::is_control) {
            Some("The name is too long: 12 letters at most.")
        } else if Draft::trimmed(&self.head).len() < 4 {
            Some("It needs a head to speak of.")
        } else if Draft::trimmed(&self.body).is_empty() {
            Some("It needs a body.")
        } else {
            None
        }
    }

    /// As a pack has it.
    fn text(&self, tab: &str) -> String {
        let mut o = String::new();
        let ch = |c: u8| c as char;
        let _ = writeln!(o, "[pet {}]\nname = {}\ntab = {tab}\nface = {}\neye = K\nhead =", self.id, self.name.trim(), ch(self.face()));
        let head = Draft::trimmed(&self.head);
        for r in &head {
            let _ = writeln!(o, "| {}", String::from_utf8_lossy(r));
        }
        o.push_str("torso =\n");
        for r in Draft::trimmed(&self.body) {
            let _ = writeln!(o, "| {}", String::from_utf8_lossy(&r));
        }
        let _ = writeln!(o, "legs = 2.5 2 {} {}\narms = 3 {} {}", ch(self.legs[0]), ch(self.legs[1]), ch(self.arms[0]), ch(self.arms[1]));
        if TAILS[self.tail].1 > 0 {
            let _ = writeln!(o, "tail = {} {} {}", TAILS[self.tail].1, ch(self.tail_c), ch(self.tail_c));
        }
        let c = ch(self.back_c);
        match self.back {
            1 => drop(writeln!(o, "back = cape {c}")),
            2 => drop(writeln!(o, "back = wings {c} {c}")),
            3 => drop(writeln!(o, "back = swords")),
            4 => drop(writeln!(o, "back = limbs {c}")),
            _ => {}
        }
        // the mouth is under the eyes
        let eyes = head.iter().rposition(|r| r.contains(&b'E')).map_or(head.len() * 6 / 10, |y| y + 1).min(head.len() - 1);
        let mut tricks: Vec<&str> = Vec::new();
        for t in self.tricks {
            if !tricks.contains(&ACTS[t].0) {
                tricks.push(ACTS[t].0);
            }
        }
        let _ = writeln!(o, "glow = {}\ngift = {}\nmouth = 0 {eyes}\nvoice = {}\nfoods = {}\ntricks = {}\n", ch(self.glow), GIFTS[self.gift].1, VOICES[self.voice].1, LIKES[self.likes].1.join(" "), tricks.join(" "));
        o
    }

    /// Look at it as a pet, for as long as `f` takes.
    fn as_pet<R>(&self, f: impl FnOnce(&Pet) -> R) -> R {
        let mut head = Draft::trimmed(&self.head);
        if head.is_empty() {
            head = vec![vec![b'.'; HEAD.0]];
        }
        let mut body = Draft::trimmed(&self.body);
        if body.is_empty() {
            body = vec![vec![b'.'; BODY.0]];
        }
        let rows: Vec<&[u8]> = head.iter().map(|r| &r[..]).collect();
        let brows: Vec<&[u8]> = body.iter().map(|r| &r[..]).collect();
        // a pet says its drawings live for ever; this one does not outlive `f`, and nobody keeps it
        let forever = |g: &[&[u8]]| -> &'static [&'static [u8]] { unsafe { std::mem::transmute(g) } };
        let pet = Pet {
            name: "",
            eye: b'K',
            face: self.face(),
            head: forever(&rows),
            torso: forever(&brows),
            build: Build::Legs,
            legs: Legs { hip: 2.5, len: 2, c: self.legs[0], foot: self.legs[1] },
            arms: Some(Arms { len: 3, c: self.arms[0], hand: self.arms[1], fat: 1.6 }),
            tail: (TAILS[self.tail].1 > 0).then(|| Tail { len: TAILS[self.tail].1, c: self.tail_c, tip: self.tail_c }),
            gear: Gear {
                back: match self.back {
                    1 => Back::Cape(self.back_c),
                    2 => Back::Wings(self.back_c, self.back_c),
                    3 => Back::Swords,
                    4 => Back::Limbs(self.back_c, false),
                    _ => Back::None,
                },
                tool: None,
                glow: self.glow,
                gift: GIFTS[self.gift].2,
                aloft: Aloft::Jets,
                door: false,
                disc: &[],
                lights: &[],
                board: [b'V', b'i', 0],
            },
            mouth: (0.0, 0),
            voice: VOICES[self.voice].2,
            says: &[],
            foods: [0, 1, 2],
            tricks: &[],
        };
        f(&pet)
    }
}

/// A pack of all of them.
pub(crate) fn pack_text(all: &[Draft]) -> String {
    let mut o = format!("petpack 1\nid = {ID}\nname = My characters\nabout = Made in the character maker.\n\n");
    for (i, d) in all.iter().enumerate() {
        let tab = if i < pack::PER_TAB { "MINE".to_string() } else { format!("MINE {}", i / pack::PER_TAB + 1) };
        o += &d.text(&tab);
    }
    o
}

/// A name as an id that nobody else has.
fn id_for(name: &str, taken: &[String]) -> String {
    let mut base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().trim_matches('-').chars().take(16).collect();
    if base.is_empty() {
        base = "pet".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Put it among the others: over the one it was, or as a new one.
pub(crate) fn keep(all: &mut Vec<Draft>, d: &mut Draft) -> Result<(), &'static str> {
    if let Some(m) = d.missing() {
        return Err(m);
    }
    let same = |o: &Draft| o.id == d.id && !d.id.is_empty();
    if all.iter().any(|o| !same(o) && o.name.trim().eq_ignore_ascii_case(d.name.trim())) {
        return Err("You have one with that name already.");
    }
    match all.iter().position(same) {
        Some(i) => all[i] = d.clone(),
        None if all.len() >= MOST => return Err("You have made so many! Remove one first."),
        None => {
            d.id = id_for(&d.name, &all.iter().map(|o| o.id.clone()).collect::<Vec<_>>());
            all.push(d.clone());
        }
    }
    Ok(())
}

fn fill_at(g: &mut Grid, x: usize, y: usize, c: u8) {
    let was = g[y][x];
    if was == c {
        return;
    }
    let mut todo = vec![(x, y)];
    while let Some((x, y)) = todo.pop() {
        if g[y][x] != was {
            continue;
        }
        g[y][x] = c;
        if x > 0 {
            todo.push((x - 1, y));
        }
        if y > 0 {
            todo.push((x, y - 1));
        }
        if x + 1 < g[0].len() {
            todo.push((x + 1, y));
        }
        if y + 1 < g.len() {
            todo.push((x, y + 1));
        }
    }
}

/// A dark line around everything that has none.
fn outline(g: &mut Grid) {
    let was = g.clone();
    let solid = |x: i32, y: i32| was.get(y as usize).and_then(|r| r.get(x as usize)).is_some_and(|c| !b".K".contains(c));
    for (y, row) in g.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            let (x, y) = (x as i32, y as i32);
            if *c == b'.' && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(i, j)| solid(x + i, y + j)) {
                *c = b'K';
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Pen,
    Fill,
    Rub,
}

/// Everything on the table.
struct Desk {
    d: Draft,
    all: Vec<Draft>,
    back: Vec<Draft>, // to undo
    on_body: bool,
    mirror: bool,
    tool: Tool,
    colour: u8,
    painting: Option<u8>,
    from: usize, // which character it was copied from
    mine: usize,
    clip: usize, // what the preview does
    t: f32,
    note: String,
    good: bool,
}

impl Desk {
    fn new(all: Vec<Draft>) -> Desk {
        Desk { d: Draft::new(), all, back: Vec::new(), on_body: false, mirror: true, tool: Tool::Pen, colour: b'K', painting: None, from: usize::MAX, mine: usize::MAX, clip: 0, t: 0.0, note: "DRAW, NAME IT, AND KEEP IT.".into(), good: true }
    }

    fn grid(&mut self) -> &mut Grid {
        if self.on_body { &mut self.d.body } else { &mut self.d.head }
    }

    fn size(&self) -> (usize, usize) {
        if self.on_body { BODY } else { HEAD }
    }

    fn remember(&mut self) {
        if self.back.last() != Some(&self.d) {
            self.back.push(self.d.clone());
            if self.back.len() > 60 {
                self.back.remove(0);
            }
        }
    }

    fn undo(&mut self) {
        if let Some(d) = self.back.pop() {
            self.d = Draft { name: std::mem::take(&mut self.d.name), ..d };
        }
    }

    fn paint(&mut self, x: usize, y: usize, c: u8) {
        let (w, _) = self.size();
        let (mirror, tool) = (self.mirror, self.tool);
        let g = self.grid();
        for x in if mirror { vec![x, w - 1 - x] } else { vec![x] } {
            match tool {
                Tool::Fill => fill_at(g, x, y, c),
                _ => g[y][x] = c,
            }
        }
    }
}

// ---- the window ----

const W: i32 = 604;
const H: i32 = 478;
const CELL: i32 = 18;
const CX: i32 = 14;
const CY: i32 = 66;
const RX: i32 = 322; // the right half

// what can be clicked
const HEAD_TAB: i32 = 1;
const BODY_TAB: i32 = 2;
const MIRROR: i32 = 3;
const PEN: i32 = 10;
const FILL: i32 = 11;
const RUB: i32 = 12;
const UNDO: i32 = 13;
const OUTLINE: i32 = 14;
const CLEAR: i32 = 15;
const ROWS: i32 = 20; // the lines on the right
const SPOTS: i32 = 40; // their colours: two a line
const FROM_L: i32 = 70;
const FROM_R: i32 = 71;
const MINE_L: i32 = 72;
const MINE_R: i32 = 73;
const NEW: i32 = 74;
const DELETE: i32 = 75;
const KEEP: i32 = 76;
const PREVIEW: i32 = 77;
const SWATCH: i32 = 200;
const CELLS: i32 = 1000;

const LINES: [&str; 11] = ["ARMS, HANDS", "LEGS, FEET", "GLOW", "BACK", "TAIL", "POWER", "TRICK 1", "TRICK 2", "TRICK 3", "VOICE", "LIKES"];

fn line_rect(i: i32) -> RECT {
    rc(kk(RX), kk(72 + i * 24), kk(W - RX - 14), kk(22))
}

/// The little fields of colour on a line.
fn spot_rect(i: i32, n: i32) -> RECT {
    let r = line_rect(i);
    rc(r.right - kk(24 * (2 - n)), r.top + kk(2), kk(20), kk(18))
}

/// Which of the two fields a line has.
fn spots(i: i32) -> &'static [i32] {
    match i {
        0 | 1 => &[0, 1],
        2..=4 => &[1],
        _ => &[],
    }
}

fn name_rect() -> RECT {
    rc(kk(RX + 60), kk(44), kk(W - RX - 82), kk(20))
}

fn button_rect(b: i32) -> RECT {
    match b {
        HEAD_TAB => rc(kk(CX), kk(38), kk(64), kk(22)),
        BODY_TAB => rc(kk(CX + 70), kk(38), kk(64), kk(22)),
        MIRROR => rc(kk(CX + 198), kk(38), kk(90), kk(22)),
        PEN..=CLEAR => rc(kk(CX + (b - PEN) * 48), kk(CY + 13 * CELL + 8), kk(46), kk(24)),
        FROM_L => rc(kk(RX + 108), kk(362), kk(24), kk(22)),
        FROM_R => rc(kk(W - 38), kk(362), kk(24), kk(22)),
        MINE_L => rc(kk(RX + 108), kk(390), kk(24), kk(22)),
        MINE_R => rc(kk(W - 38), kk(390), kk(24), kk(22)),
        NEW => rc(kk(RX + 108), kk(420), kk(48), kk(26)),
        DELETE => rc(kk(RX + 160), kk(420), kk(48), kk(26)),
        KEEP => rc(kk(RX + 212), kk(420), kk(W - RX - 226), kk(26)),
        PREVIEW => rc(kk(RX), kk(344), kk(102), kk(102)),
        _ => rc(0, 0, 0, 0),
    }
}

const BUTTONS: [i32; 17] = [HEAD_TAB, BODY_TAB, MIRROR, PEN, FILL, RUB, UNDO, OUTLINE, CLEAR, FROM_L, FROM_R, MINE_L, MINE_R, NEW, DELETE, KEEP, PREVIEW];

fn swatch_rect(i: usize) -> RECT {
    rc(kk(CX + (i % COLS) as i32 * 17), kk(CY + 13 * CELL + 42 + (i / COLS) as i32 * 17), kk(16), kk(16))
}

/// Top left of the drawing.
fn corner((w, h): (usize, usize)) -> (i32, i32) {
    (CX + (HEAD.0 - w) as i32 * CELL / 2, CY + (HEAD.1 - h) as i32 * CELL / 2)
}

fn hit(x: i32, y: i32, size: (usize, usize)) -> i32 {
    if contains(&close_rect(W), x, y) {
        return CLOSE;
    }
    if let Some(b) = BUTTONS.into_iter().find(|b| contains(&button_rect(*b), x, y)) {
        return b;
    }
    for i in 0..LINES.len() as i32 {
        if let Some(n) = spots(i).iter().find(|n| contains(&spot_rect(i, **n), x, y)) {
            return SPOTS + i * 2 + n;
        }
        if contains(&line_rect(i), x, y) {
            return ROWS + i;
        }
    }
    if let Some(i) = (0..PALETTE.len()).find(|i| contains(&swatch_rect(*i), x, y)) {
        return SWATCH + i as i32;
    }
    let (cx, cy) = corner(size);
    let (gx, gy) = ((x - kk(cx)).div_euclid(kk(CELL).max(1)), (y - kk(cy)).div_euclid(kk(CELL).max(1)));
    if (0..size.0 as i32).contains(&gx) && (0..size.1 as i32).contains(&gy) {
        return CELLS + gy * 100 + gx;
    }
    -1
}

/// The characters that can be copied.
fn models() -> Vec<usize> {
    (0..).take_while(|i| pack::is_there(*i)).filter(|i| pack::pack_of(*i) == 0 && Draft::of(&PETS[*i]).is_some()).collect()
}

fn load() -> Vec<Draft> {
    pack::of_pack(ID)
        .into_iter()
        .filter_map(|(i, id)| {
            let mut d = Draft::of(&PETS[i])?;
            d.id = id;
            Some(d)
        })
        .collect()
}

pub(crate) unsafe fn show() {
    if hw(&WND).is_null() {
        work_on(null_mut());
        let (w, e) = create("PixelMakerWnd", "Pixel Pet - Character maker", W, H, name_rect(), 0x80 /* ES_AUTOHSCROLL */, 14);
        WND.store(w as isize, Relaxed);
        EDIT.store(e as isize, Relaxed);
        SendMessageW(e, 0x00C5 /* EM_SETLIMITTEXT */, 12, 0);
        SetTimer(w, 1, 90, None);
    }
    if DESK.lock().unwrap().is_none() {
        *DESK.lock().unwrap() = Some(Desk::new(load()));
    }
    toggle(hw(&WND), hw(&EDIT));
}

unsafe fn open_draft(d: Draft) {
    let name = d.name.clone();
    if let Some(k) = DESK.lock().unwrap().as_mut() {
        k.remember();
        k.d = d;
    }
    SetWindowTextW(hw(&EDIT), wide(&name).as_ptr());
}

/// Write them all down, and have them.
unsafe fn store(all: &[Draft]) -> Result<Option<usize>, String> {
    if all.is_empty() {
        if let Some(p) = pack::number(ID) {
            pack::remove(p);
        }
        settle(hw(&PET_WND));
        return Ok(None);
    }
    let a = pack::import_bytes(pack_text(all).into_bytes())?;
    settle(hw(&PET_WND));
    Ok(Some(a.first))
}

unsafe fn press(h: i32, right: bool) {
    let turn = |n: &mut usize, len: usize| *n = if right { (*n + len - 1) % len } else { (*n + 1) % len };
    // the ones that need no lock held
    match h {
        FROM_L | FROM_R => {
            let m = models();
            let from = {
                let mut k = DESK.lock().unwrap();
                let Some(k) = k.as_mut() else { return };
                k.from = match k.from {
                    usize::MAX => 0,
                    n if h == FROM_L => (n + m.len() - 1) % m.len(),
                    n => (n + 1) % m.len(),
                };
                k.from
            };
            if let Some(mut d) = m.get(from).and_then(|i| Draft::of(&PETS[*i])) {
                let keep_name = get_text(hw(&EDIT));
                let copied = d.name.clone();
                d.name = keep_name;
                open_draft(d);
                if let Some(k) = DESK.lock().unwrap().as_mut() {
                    (k.note, k.good) = (format!("COPIED FROM {}. MAKE IT YOUR OWN!", copied.to_uppercase()), true);
                }
            }
            return;
        }
        MINE_L | MINE_R => {
            let d = {
                let mut k = DESK.lock().unwrap();
                let Some(k) = k.as_mut().filter(|k| !k.all.is_empty()) else { return };
                let n = k.all.len();
                k.mine = match k.mine {
                    usize::MAX => 0,
                    i if h == MINE_L => (i + n - 1) % n,
                    i => (i + 1) % n,
                };
                k.all[k.mine.min(n - 1)].clone()
            };
            open_draft(d);
            return;
        }
        NEW => return open_draft(Draft::new()),
        KEEP => {
            let name = get_text(hw(&EDIT)).trim().to_string();
            let kept = {
                let mut k = DESK.lock().unwrap();
                let Some(k) = k.as_mut() else { return };
                k.d.name = name;
                let mut all = k.all.clone();
                let mut d = k.d.clone();
                match keep(&mut all, &mut d) {
                    Ok(()) => Some((all, d)),
                    Err(e) => {
                        (k.note, k.good) = (e.to_uppercase(), false);
                        None
                    }
                }
            };
            let Some((all, d)) = kept else { return };
            let done = store(&all);
            let at = all.iter().position(|o| o.id == d.id);
            let mut k = DESK.lock().unwrap();
            let Some(k) = k.as_mut() else { return };
            match done {
                Ok(first) => {
                    (k.note, k.good) = (format!("{} IS IN LOOKS, CHARACTER, MY CHARACTERS.", d.name.to_uppercase()), true);
                    k.mine = at.unwrap_or(usize::MAX);
                    k.d = d;
                    k.all = all;
                    drop_lock_and_wear(first.zip(at).map(|(f, a)| f + a));
                }
                Err(e) => (k.note, k.good) = (e.to_uppercase(), false),
            }
            return;
        }
        DELETE => {
            let all = {
                let mut k = DESK.lock().unwrap();
                let Some(k) = k.as_mut() else { return };
                let mut all = k.all.clone();
                all.retain(|o| o.id != k.d.id);
                if all.len() == k.all.len() {
                    (k.note, k.good) = ("THAT ONE WAS NOT KEPT YET.".into(), false);
                    return;
                }
                all
            };
            let ok = store(&all);
            {
                let mut k = DESK.lock().unwrap();
                let Some(k) = k.as_mut() else { return };
                match ok {
                    Ok(_) => {
                        (k.note, k.good) = (format!("{} IS GONE.", k.d.name.to_uppercase()), true);
                        k.all = all;
                        k.mine = usize::MAX;
                        k.d.id.clear(); // what is drawn may be kept again
                    }
                    Err(e) => (k.note, k.good) = (e.to_uppercase(), false),
                }
            }
            return;
        }
        _ => {}
    }
    let mut k = DESK.lock().unwrap();
    let Some(k) = k.as_mut() else { return };
    match h {
        HEAD_TAB => k.on_body = false,
        BODY_TAB => k.on_body = true,
        MIRROR => k.mirror = !k.mirror,
        PEN => k.tool = Tool::Pen,
        FILL => k.tool = Tool::Fill,
        RUB => k.tool = Tool::Rub,
        UNDO => k.undo(),
        OUTLINE => {
            k.remember();
            outline(k.grid());
        }
        CLEAR => {
            k.remember();
            *k.grid() = blank(k.size());
        }
        PREVIEW => {
            k.clip = (k.clip + 1) % 5;
            k.t = 0.0;
        }
        SWATCH..=299 => {
            if let Some(c) = PALETTE.get((h - SWATCH) as usize) {
                k.colour = *c;
                if k.tool == Tool::Rub {
                    k.tool = Tool::Pen;
                }
            }
        }
        SPOTS..=69 => {
            // an eye is not a colour to wear
            let c = if k.colour == b'E' { b'K' } else { k.colour };
            k.remember();
            let (i, n) = ((h - SPOTS) / 2, ((h - SPOTS) % 2) as usize);
            match i {
                0 => k.d.arms[n] = c,
                1 => k.d.legs[n] = c,
                2 => k.d.glow = c,
                3 => k.d.back_c = c,
                _ => k.d.tail_c = c,
            }
        }
        ROWS..=39 => {
            k.remember();
            match h - ROWS {
                3 => turn(&mut k.d.back, BACKS.len()),
                4 => turn(&mut k.d.tail, TAILS.len()),
                5 => turn(&mut k.d.gift, GIFTS.len()),
                i @ 6..=8 => {
                    turn(&mut k.d.tricks[(i - 6) as usize], ACTS.len());
                    // and show it
                    k.clip = (i - 6) as usize + 2;
                    k.t = 0.0;
                }
                9 => turn(&mut k.d.voice, VOICES.len()),
                10 => turn(&mut k.d.likes, LIKES.len()),
                _ => {}
            }
        }
        CELLS.. => {
            let (x, y) = (((h - CELLS) % 100) as usize, ((h - CELLS) / 100) as usize);
            k.remember();
            let c = if right || k.tool == Tool::Rub { b'.' } else { k.colour };
            k.painting = Some(c);
            k.paint(x, y, c);
        }
        _ => {}
    }
}

/// Wear what has just been made. Done by a message, as the desk is locked while it is asked for.
unsafe fn drop_lock_and_wear(pet: Option<usize>) {
    if let Some(i) = pet {
        PostMessageW(hw(&WND), WM_WEAR, i, 0);
    }
}

const WM_WEAR: u32 = WM_APP + 20;

/// A picture of the draft, drawn into a window that is not made of dots.
unsafe fn preview(dc: HDC, k: &Desk, r: &RECT) {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let back = theme().panel;
    // blit writes the dots the way a picture has them: blue first
    let swap = |c: u32| (c & 0xff) << 16 | (c & 0xff00) | (c >> 16 & 0xff);
    let mut px = vec![swap(back); (w * h) as usize];
    k.d.as_pet(|pet| {
        let clip = match k.clip {
            0 => Clip::Stand,
            1 => Clip::Walk,
            n => ACTS[k.d.tricks[n - 2]].1,
        };
        let len = clip.secs().unwrap_or(100.0);
        let rig = sample(pet, clip, k.t % (len + 0.6).max(0.1), &Push::NONE);
        let scale = h as f32 / 25.0;
        blit(&mut Surface { px: &mut px, w, h }, &compose(pet, &rig), pet, &rig, (w as f32 / 2.0, h as f32 * 0.93), scale, 1);
    });
    fill(dc, r.left, r.top, w, h, back);
    for y in 0..h {
        let row = &px[(y * w) as usize..((y + 1) * w) as usize];
        let mut x = 0;
        while x < w {
            let c = row[x as usize];
            let mut e = x + 1;
            while e < w && row[e as usize] == c {
                e += 1;
            }
            let c = swap(c & 0xff_ffff);
            if c != back {
                fill(dc, r.left + x, r.top + y, e - x, 1, c);
            }
            x = e;
        }
    }
}

unsafe fn draw(hwnd: HWND) {
    let hover = HOVER.load(Relaxed);
    let desk = DESK.lock().unwrap();
    let Some(k) = desk.as_ref() else {
        let mut ps: PAINTSTRUCT = zeroed();
        BeginPaint(hwnd, &mut ps);
        EndPaint(hwnd, &ps);
        return;
    };
    let mine = k.all.len();
    painted(hwnd, W, H, "CHARACTER MAKER", hover, |dc, big, small| {
        SelectObject(dc, small);
        let on = |b: bool| if b { BTN_GREEN } else { theme().panel_light };
        for (b, label, bg) in [
            (HEAD_TAB, "HEAD", on(!k.on_body)),
            (BODY_TAB, "BODY", on(k.on_body)),
            (MIRROR, "MIRROR", on(k.mirror)),
            (PEN, "PEN", on(k.tool == Tool::Pen)),
            (FILL, "FILL", on(k.tool == Tool::Fill)),
            (RUB, "RUB", on(k.tool == Tool::Rub)),
            (UNDO, "UNDO", theme().panel_light),
            (OUTLINE, "LINE", theme().panel_light),
            (CLEAR, "CLEAR", BTN_RED),
            (FROM_L, "<", theme().panel_light),
            (FROM_R, ">", theme().panel_light),
            (MINE_L, "<", theme().panel_light),
            (MINE_R, ">", theme().panel_light),
            (NEW, "NEW", theme().panel_light),
            (DELETE, "DEL", BTN_RED),
            (KEEP, "KEEP", BTN_GREEN),
        ] {
            pixel_button(dc, &button_rect(b), label, bg, hover == b);
        }
        // the drawing
        let size = k.size();
        let (cx, cy) = corner(size);
        let grid = if k.on_body { &k.d.body } else { &k.d.head };
        let c = kk(CELL);
        fill(dc, kk(CX) - kk(2), kk(CY) - kk(2), kk(16 * CELL) + kk(4), kk(13 * CELL) + kk(4), theme().panel_edge);
        fill(dc, kk(CX), kk(CY), kk(16 * CELL), kk(13 * CELL), theme().panel);
        for (y, row) in grid.iter().enumerate() {
            for (x, v) in row.iter().enumerate() {
                let (px, py) = (kk(cx) + x as i32 * c, kk(cy) + y as i32 * c);
                let shade = if (x + y) % 2 == 0 { rgb(0x8a, 0x8c, 0x9c) } else { rgb(0x9b, 0x9d, 0xad) };
                match *v {
                    b'.' => fill(dc, px, py, c, c, shade),
                    b'E' => {
                        fill(dc, px, py, c, c, PET_INK);
                        // one shine for the whole eye
                        let first = (x == 0 || grid[y][x - 1] != b'E') && (y == 0 || grid[y - 1][x] != b'E');
                        if first {
                            fill(dc, px + c / 4, py + c / 4, c / 2, c / 2, CREAM);
                        }
                    }
                    v => fill(dc, px, py, c, c, pet_color(v).unwrap_or(CREAM)),
                }
            }
        }
        // where the middle is
        fill(dc, kk(cx) + size.0 as i32 * c / 2, kk(cy), 1, size.1 as i32 * c, theme().accent);
        if hover >= CELLS {
            let (x, y) = ((hover - CELLS) % 100, (hover - CELLS) / 100);
            let (px, py) = (kk(cx) + x * c, kk(cy) + y * c);
            for (x, y, w, h) in [(px, py, c, 1), (px, py + c - 1, c, 1), (px, py, 1, c), (px + c - 1, py, 1, c)] {
                fill(dc, x, y, w, h, CREAM);
            }
        }
        for (i, v) in PALETTE.iter().enumerate() {
            let r = swatch_rect(i);
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if *v == k.colour {
                fill(dc, r.left - 1, r.top - 1, w + 2, h + 2, CREAM);
            }
            fill(dc, r.left, r.top, w, h, pet_color(*v).unwrap_or(CREAM));
            if *v == b'E' {
                fill(dc, r.left + w / 5, r.top + h / 5, w / 3, h / 3, CREAM);
            }
        }
        // the lines on the right
        SelectObject(dc, big);
        SetTextColor(dc, theme().hint);
        let mut r = rc(kk(RX), kk(42), kk(60), kk(24));
        text_at(dc, "NAME", &mut r, DT_SINGLELINE | DT_VCENTER);
        tools::framed(dc, &rc(kk(RX + 54), kk(40), kk(W - RX - 68), kk(28)), GREEN);
        SelectObject(dc, small);
        for (i, label) in LINES.iter().enumerate() {
            let i = i as i32;
            let r = line_rect(i);
            let lit = hover == ROWS + i;
            fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, if lit && i > 2 { theme().panel_light } else { theme().panel });
            SetTextColor(dc, CREAM);
            let mut lr = RECT { left: r.left + kk(6), ..r };
            text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
            let value = match i {
                3 => BACKS[k.d.back].to_string(),
                4 => TAILS[k.d.tail].0.to_string(),
                5 => GIFTS[k.d.gift].0.to_string(),
                6..=8 => ACTS[k.d.tricks[(i - 6) as usize]].0.to_uppercase(),
                9 => VOICES[k.d.voice].0.to_string(),
                10 => LIKES[k.d.likes].0.to_string(),
                _ => String::new(),
            };
            let cut = if spots(i).is_empty() { kk(6) } else { kk(30) };
            SetTextColor(dc, UP_BLUE);
            let mut vr = RECT { right: r.right - cut, ..r };
            text_at(dc, &value, &mut vr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            for n in spots(i) {
                let s = spot_rect(i, *n);
                let c = match (i, n) {
                    (0, n) => k.d.arms[*n as usize],
                    (1, n) => k.d.legs[*n as usize],
                    (2, _) => k.d.glow,
                    (3, _) => k.d.back_c,
                    _ => k.d.tail_c,
                };
                let edge = if hover == SPOTS + i * 2 + n { CREAM } else { theme().panel_edge };
                fill(dc, s.left, s.top, s.right - s.left, s.bottom - s.top, edge);
                let e = kk(2);
                fill(dc, s.left + e, s.top + e, s.right - s.left - 2 * e, s.bottom - s.top - 2 * e, pet_color(c).unwrap_or(CREAM));
            }
        }
        let pr = button_rect(PREVIEW);
        preview(dc, k, &pr);
        SetTextColor(dc, theme().hint);
        let mut r = rc(kk(RX + 134), kk(362), kk(W - RX - 174), kk(22));
        text_at(dc, "START FROM", &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
        let mut r = rc(kk(RX + 134), kk(390), kk(W - RX - 174), kk(22));
        text_at(dc, &format!("MY {mine}"), &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
        let mut r = rc(kk(RX + 108), kk(340), kk(W - RX - 122), kk(20));
        text_at(dc, ["STANDING", "WALKING", "TRICK 1", "TRICK 2", "TRICK 3"][k.clip], &mut r, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, if k.good { theme().hint } else { TOMATO });
        let mut r = rc(kk(CX), kk(H - 26), kk(W - 28), kk(20));
        text_at(dc, &k.note, &mut r, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
    });
}

pub(crate) unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    work_on(hwnd);
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    let size = || DESK.lock().unwrap().as_ref().map_or(HEAD, |k| k.size());
    match msg {
        WM_PAINT => draw(hwnd),
        WM_DPICHANGED => {
            resize_for_dpi(hwnd, lp);
            fit_edit(hw(&EDIT), name_rect(), 14);
        }
        WM_TIMER => {
            if IsWindowVisible(hwnd) != 0 {
                if let Some(k) = DESK.lock().unwrap().as_mut() {
                    k.t += 0.09;
                }
                InvalidateRect(hwnd, &button_rect(PREVIEW), 0);
            }
        }
        WM_MOUSEMOVE => {
            let h = hit(mx, my, size());
            {
                let mut k = DESK.lock().unwrap();
                if let Some((k, c)) = k.as_mut().and_then(|k| k.painting.map(|c| (k, c))) {
                    if h >= CELLS && k.tool != Tool::Fill {
                        k.paint(((h - CELLS) % 100) as usize, ((h - CELLS) / 100) as usize, c);
                    }
                }
            }
            if HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            let h = hit(mx, my, size());
            if h == CLOSE {
                ShowWindow(hwnd, SW_HIDE);
                return 0;
            }
            if h >= CELLS {
                SetCapture(hwnd);
            }
            press(h, msg == WM_RBUTTONDOWN);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_LBUTTONUP | WM_RBUTTONUP => {
            ReleaseCapture();
            if let Some(k) = DESK.lock().unwrap().as_mut() {
                k.painting = None;
            }
        }
        WM_WEAR => {
            reanchor(hw(&PET_WND), |s| {
                s.body.stop();
                s.body.meal = None;
                s.pet = pack::still(wp);
            });
        }
        _ => return common(hwnd, msg, wp, lp, W).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp)),
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(n: &str) -> Draft {
        Draft { name: n.into(), ..Draft::new() }
    }

    #[test]
    fn what_is_made_is_a_pack_that_can_be_read() {
        let mut all = Vec::new();
        for (i, n) in ["Bob", "Zed the 2nd", "\u{20ac}uro"].iter().enumerate() {
            let mut d = named(n);
            (d.back, d.tail, d.gift, d.voice, d.likes) = (i + 1, i % 3, i + 1, i, i);
            keep(&mut all, &mut d).unwrap();
        }
        let p = pack::parse(&pack_text(&all)).unwrap_or_else(|e| panic!("{e}\n{}", pack_text(&all)));
        assert_eq!((p.id.as_str(), p.len()), (ID, 3));
        // every choice there is
        for i in 0..ACTS.len() {
            let mut d = named("X");
            d.tricks = [i, (i + 1) % ACTS.len(), i];
            (d.back, d.tail, d.gift, d.voice, d.likes) = (i % BACKS.len(), i % TAILS.len(), i % GIFTS.len(), i % VOICES.len(), i % LIKES.len());
            let c = PALETTE[i % PALETTE.len()];
            (d.arms, d.legs, d.glow, d.back_c, d.tail_c) = ([c; 2], [c; 2], c, c, c);
            let mut all = Vec::new();
            keep(&mut all, &mut d).unwrap();
            pack::parse(&pack_text(&all)).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    #[test]
    fn as_many_as_there_may_be_fit_into_one_pack() {
        let mut all = Vec::new();
        for i in 0..MOST {
            keep(&mut all, &mut named(&format!("Pet {i}"))).unwrap();
        }
        assert!(keep(&mut all, &mut named("One more")).is_err());
        assert_eq!(pack::parse(&pack_text(&all)).map(|p| p.len()).unwrap_or_else(|e| panic!("{e}")), MOST);
    }

    #[test]
    fn the_biggest_drawing_fits_the_window() {
        let mut d = named("Big");
        d.head = vec![vec![b'O'; HEAD.0]; HEAD.1];
        d.body = vec![vec![b'O'; BODY.0]; BODY.1];
        d.id = "big".into();
        pack::parse(&pack_text(&[d])).unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn changing_one_keeps_what_the_settings_call_it() {
        let mut all = Vec::new();
        let mut a = named("Ann");
        keep(&mut all, &mut a).unwrap();
        a.name = "Anna".into();
        keep(&mut all, &mut a).unwrap();
        assert_eq!((all.len(), all[0].id.as_str(), all[0].name.as_str()), (1, "ann", "Anna"));
        assert!(keep(&mut all, &mut named("ANNA")).is_err(), "two of the same name");
        keep(&mut all, &mut named("Ann")).unwrap();
        assert_eq!(all[1].id, "ann-2");
        assert_eq!(id_for("!!!", &[]), "pet");
        assert!(id_for(&"x".repeat(99), &[]).len() <= 24);
    }

    #[test]
    fn what_cannot_be_kept_is_said() {
        assert!(Draft::new().missing().is_some());
        assert!(named("Thirteen chars").missing().is_some());
        let mut d = named("Bob");
        assert_eq!(d.missing(), None);
        d.head = blank(HEAD);
        assert!(d.missing().is_some());
        let mut d = named("Bob");
        d.body = blank(BODY);
        assert!(d.missing().is_some());
    }

    #[test]
    fn a_copy_looks_like_what_it_was_copied_from() {
        let mut n = 0;
        for p in PETS.iter() {
            let Some(d) = Draft::of(p) else { continue };
            n += 1;
            let mut d2 = d.clone();
            d2.id = "x".into();
            pack::parse(&pack_text(&[d2])).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            // the same drawing, where the colours are the pet's own
            let mine = p.head.concat().iter().all(|c| *c < 128);
            if mine {
                assert_eq!(Draft::trimmed(&d.head).iter().map(|r| r.iter().filter(|c| **c != b'.').count()).sum::<usize>(), p.head.concat().iter().filter(|c| **c != b'.').count(), "{}", p.name);
            }
            d.as_pet(|q| {
                let cv = compose(q, &Rig::REST);
                let mut px = vec![0u32; 120 * 120];
                blit(&mut Surface { px: &mut px, w: 120, h: 120 }, &cv, q, &Rig::REST, (60.0, 110.0), 3.0, 1);
                assert!(px.iter().any(|c| *c != 0));
            });
        }
        assert!(n >= 20, "only {n} can be copied");
    }

    #[test]
    fn painting_mirrors_fills_and_can_be_undone() {
        let mut k = Desk::new(Vec::new());
        k.d.head = blank(HEAD);
        k.remember();
        k.paint(1, 2, b'R');
        assert_eq!((k.d.head[2][1], k.d.head[2][14]), (b'R', b'R'));
        k.mirror = false;
        k.remember();
        k.paint(3, 3, b'b');
        assert_eq!(k.d.head[3][12], b'.');
        k.tool = Tool::Fill;
        k.remember();
        k.paint(0, 0, b'g');
        assert_eq!((k.d.head[12][15], k.d.head[2][1]), (b'g', b'R'));
        k.undo();
        assert_eq!(k.d.head[12][15], b'.');
        k.undo();
        k.undo();
        assert_eq!(k.d.head, blank(HEAD));
        k.undo(); // nothing left
        k.on_body = true;
        k.tool = Tool::Pen;
        k.paint(0, 0, b'K');
        assert_eq!(k.d.body[0][0], b'K');
    }

    #[test]
    fn the_line_goes_around_what_is_painted() {
        let mut g = blank((6, 5));
        g[2][2] = b'R';
        outline(&mut g);
        let k = g.iter().flatten().filter(|c| **c == b'K').count();
        assert_eq!((k, g[2][2], g[1][2], g[2][1]), (4, b'R', b'K', b'K'));
        let before = g.clone();
        outline(&mut g);
        assert_eq!(g, before, "once is enough");
    }

    #[test]
    fn the_face_is_what_is_around_the_eyes() {
        let mut d = named("x");
        assert_eq!(d.face(), b'O');
        for c in d.head.iter_mut().flatten().filter(|c| **c == b'O') {
            *c = b'g';
        }
        assert_eq!(d.face(), b'g');
        d.head = blank(HEAD);
        d.head[0][0] = b'E'; // in the very corner
        assert_eq!(d.face(), b'W');
    }

    #[test]
    fn nothing_in_the_window_overlaps() {
        let mut all: Vec<RECT> = BUTTONS.iter().map(|b| button_rect(*b)).collect();
        all.extend((0..LINES.len() as i32).map(line_rect));
        all.extend((0..PALETTE.len()).map(swatch_rect));
        all.extend([close_rect(W), name_rect(), rc(kk(CX), kk(CY), kk(16 * CELL), kk(13 * CELL))]);
        for (i, a) in all.iter().enumerate() {
            assert!(a.left >= 0 && a.top >= 0 && a.right <= kk(W) && a.bottom <= kk(H - 26), "{i}: {} {} {} {}", a.left, a.top, a.right, a.bottom);
            for (j, b) in all[..i].iter().enumerate() {
                assert!(a.left >= b.right || b.left >= a.right || a.top >= b.bottom || b.top >= a.bottom, "{i} overlaps {j}");
            }
        }
        // every cell of both drawings can be hit, and nothing else answers for it
        for size in [HEAD, BODY] {
            let (cx, cy) = corner(size);
            for y in 0..size.1 as i32 {
                for x in 0..size.0 as i32 {
                    assert_eq!(hit(kk(cx) + x * kk(CELL) + 1, kk(cy) + y * kk(CELL) + 1, size), CELLS + y * 100 + x);
                }
            }
        }
    }
}
