//! A little garden and the pet's room: seeds that grow a step each day they are watered, flowers
//! that go on the shelf when picked, and a room that fills up as the pet levels up. The rules are
//! pure; `draw` paints the page of the hub.

use crate::prelude::*;

pub(crate) const POTS: usize = 3;
/// Seed, sprout, leaves, bud, in flower.
pub(crate) const BLOOM: u8 = 4;
/// Experience for a flower picked.
pub(crate) const XP_PICK: u32 = 15;

pub(crate) struct Seed {
    pub name: &'static str,
    pub thirst: u32, // days it goes without water before it wilts
    pub petal: u32,
    pub heart: u32,
}

pub(crate) static SEEDS: [Seed; 5] = [
    Seed { name: "DAISY", thirst: 2, petal: CREAM, heart: rgb(0xff, 0xd0, 0x3a) },
    Seed { name: "SUNFLOWER", thirst: 2, petal: rgb(0xff, 0xd0, 0x3a), heart: rgb(0x7a, 0x4b, 0x2a) },
    Seed { name: "TULIP", thirst: 2, petal: rgb(0xe8, 0x48, 0x5a), heart: rgb(0xb0, 0x2a, 0x40) },
    Seed { name: "CACTUS", thirst: 5, petal: rgb(0xff, 0x7f, 0xb8), heart: rgb(0xff, 0xd0, 0x3a) },
    Seed { name: "TOMATO", thirst: 2, petal: rgb(0xe8, 0x40, 0x30), heart: rgb(0x4c, 0x94, 0x4e) },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Pot {
    pub seed: Option<u8>,
    pub stage: u8,
    pub watered: u32, // the last day it had water (days since 1970)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Garden {
    pub pots: [Pot; POTS],
    pub picked: [u32; 5], // flowers on the shelf, of each seed
    pub choice: u8,       // the seed that is planted next
}

/// What watering did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Watered {
    pub grew: u32,
    pub bloomed: Option<u8>, // a seed that came into flower
    pub revived: u32,
    pub had: u32, // already had water today
}

/// How a pot is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
    Empty,
    Growing,
    Thirsty,
    Wilted,
    Flower,
}

impl Garden {
    pub const fn new() -> Garden {
        Garden { pots: [Pot { seed: None, stage: 0, watered: 0 }; POTS], picked: [0; 5], choice: 0 }
    }

    pub fn state(&self, i: usize, day: u32) -> State {
        let p = self.pots[i];
        let Some(seed) = p.seed else { return State::Empty };
        if p.stage >= BLOOM {
            State::Flower // it waits to be picked, and no longer needs anything
        } else if day.saturating_sub(p.watered) > SEEDS[seed as usize].thirst {
            State::Wilted
        } else if p.watered < day {
            State::Thirsty
        } else {
            State::Growing
        }
    }

    /// Put the chosen seed in an empty pot. Planting waters it too.
    pub fn plant(&mut self, i: usize, day: u32) -> bool {
        if self.pots[i].seed.is_some() {
            return false;
        }
        self.pots[i] = Pot { seed: Some(self.choice), stage: 0, watered: day };
        true
    }

    /// Water every pot. A pot grows a step on each day it gets water; a wilted one only recovers.
    pub fn water(&mut self, day: u32) -> Watered {
        let mut w = Watered::default();
        for i in 0..POTS {
            match self.state(i, day) {
                State::Empty | State::Flower => {}
                State::Growing => w.had += 1,
                State::Wilted => {
                    self.pots[i].watered = day;
                    w.revived += 1;
                }
                State::Thirsty => {
                    let p = &mut self.pots[i];
                    p.watered = day;
                    p.stage += 1;
                    w.grew += 1;
                    if p.stage == BLOOM {
                        w.bloomed = p.seed;
                    }
                }
            }
        }
        w
    }

    /// Pick a flower for the shelf; the pot is free again.
    pub fn pick(&mut self, i: usize) -> Option<u8> {
        let p = self.pots[i];
        let seed = p.seed.filter(|_| p.stage >= BLOOM)?;
        self.picked[seed as usize] += 1;
        self.pots[i] = Pot::default();
        Some(seed)
    }

    /// Pots that would like water today.
    pub fn thirsty(&self, day: u32) -> usize {
        (0..POTS).filter(|i| matches!(self.state(*i, day), State::Thirsty | State::Wilted)).count()
    }

    pub fn next_seed(&mut self) {
        self.choice = (self.choice + 1) % SEEDS.len() as u8;
    }

    pub fn to_text(&self) -> String {
        let pots: Vec<String> = self.pots.iter().map(|p| p.seed.map_or("-".to_string(), |s| format!("{s},{},{}", p.stage, p.watered))).collect();
        let picked: Vec<String> = self.picked.iter().map(|n| n.to_string()).collect();
        format!("{};{};{}", self.choice, pots.join("|"), picked.join(","))
    }

    pub fn from_text(t: &str) -> Garden {
        let mut g = Garden::new();
        let mut parts = t.split(';');
        g.choice = parts.next().and_then(|c| c.parse::<u8>().ok()).filter(|c| (*c as usize) < SEEDS.len()).unwrap_or(0);
        for (pot, text) in g.pots.iter_mut().zip(parts.next().unwrap_or("").split('|')) {
            let v: Vec<u32> = text.split(',').filter_map(|x| x.parse().ok()).collect();
            if let [seed, stage, watered] = v[..] {
                if (seed as usize) < SEEDS.len() {
                    *pot = Pot { seed: Some(seed as u8), stage: (stage as u8).min(BLOOM), watered };
                }
            }
        }
        for (n, text) in g.picked.iter_mut().zip(parts.next().unwrap_or("").split(',')) {
            *n = text.parse().unwrap_or(0);
        }
        g
    }
}

/// Things for the room, and the level that brings each.
pub(crate) const ROOM: [(&str, u32); 4] = [("A RUG", 2), ("A LAMP", 4), ("BOOKS", 6), ("A PICTURE", 9)];

pub(crate) fn has(level: u32, i: usize) -> bool {
    level >= ROOM[i].1
}

/// What the room gets next, if anything.
pub(crate) fn next_thing(level: u32) -> Option<(&'static str, u32)> {
    ROOM.iter().find(|r| level < r.1).copied()
}

// ---- pictures ----

pub(crate) const ICON_PLANT: [&[u8]; 7] = [b".XX.XX.", b"XXX.XXX", b".XX.XX.", b"...X...", b"XXXXXXX", b".XXXXX.", b"..XXX.."];

const POT: [&[u8]; 6] = [b"KKKKKKKKK", b"KdddddddK", b"KCCCCCCCK", b".KCCCCCK.", b".KCCCCCK.", b"..KKKKK.."];
const SEED: [&[u8]; 1] = [b"..d.d.."];
const SPROUT: [&[u8]; 2] = [b"..g.g..", b"...g..."];
const LEAVES: [&[u8]; 4] = [b"gg...gg", b".gg.gg.", b"...g...", b"...g..."];
const BUD: [&[u8]; 6] = [b"...X...", b"..XXX..", b"...g...", b"gg.g.gg", b".ggggg.", b"...g..."];
const FLOWER: [&[u8]; 8] = [b"..X.X..", b".XXXXX.", b"XXXYXXX", b".XXXXX.", b"..XgX..", b"gg.g.gg", b".ggggg.", b"...g..."];
const CACTUS: [&[u8]; 8] = [b"...X...", b"..XYX..", b".jjjjj.", b".jjjjj.", b"jjjjjjj", b"j.jjj.j", b"..jjj..", b"..jjj.."];
const TOMATO_PLANT: [&[u8]; 8] = [b"...g...", b".XX.gg.", b"XXXX.g.", b"XXXXggg", b".XXg.XX", b"gg.gXXX", b".ggggXX", b"...g..."];
const LAMP: [&[u8]; 12] = [b".KKKKK.", b"KYYYYYK", b"KYYYYYK", b"KKKKKKK", b"...K...", b"...K...", b"...K...", b"...K...", b"...K...", b"...K...", b"..KKK..", b".KKKKK."];

fn plant_art(seed: u8, stage: u8) -> &'static [&'static [u8]] {
    match (seed, stage) {
        (_, 0) => &SEED,
        (_, 1) => &SPROUT,
        (_, 2) => &LEAVES,
        (3, BLOOM..) => &CACTUS,
        (4, BLOOM..) => &TOMATO_PLANT,
        (_, 3) => &BUD,
        _ => &FLOWER,
    }
}

/// The colours of a plant; a wilted one is brown and grey.
fn plant_colour(seed: u8, wilted: bool) -> impl Fn(u8) -> Option<u32> {
    let s = &SEEDS[seed as usize];
    let (petal, heart) = if wilted { (rgb(0xa8, 0x9c, 0x94), rgb(0x8a, 0x78, 0x66)) } else { (s.petal, s.heart) };
    move |c| match c {
        b'X' => Some(petal),
        b'Y' => Some(heart),
        b'g' | b'j' if wilted => Some(rgb(0x9a, 0x7a, 0x4a)),
        b'g' => Some(rgb(0x5c, 0xb8, 0x50)),
        b'j' => Some(rgb(0x4c, 0x94, 0x4e)),
        c => pet_color(c),
    }
}

/// What the hub needs to draw the page.
#[derive(Clone)]
pub(crate) struct View {
    pub garden: Garden,
    pub day: u32,
    pub level: u32,
    pub look: seasons::Look,
}

pub(crate) const ROOM_H: i32 = 128;
pub(crate) const POT_H: i32 = 96;
pub(crate) const ROW_H: i32 = 26;

/// Height of the page below its title.
pub(crate) fn height(u: &dyn Fn(i32) -> i32) -> i32 {
    u(ROOM_H) + u(6) + u(POT_H) + u(6) + u(ROW_H) + u(36)
}

fn sky(d: seasons::Daytime) -> u32 {
    match d {
        seasons::Daytime::Morning => rgb(0xa8, 0xd8, 0xff),
        seasons::Daytime::Day => rgb(0x6c, 0xb8, 0xff),
        seasons::Daytime::Evening => rgb(0xff, 0x9a, 0x5a),
        seasons::Daytime::Night => rgb(0x1c, 0x22, 0x4a),
    }
}

fn ground(s: seasons::Season) -> u32 {
    match s {
        seasons::Season::Winter => rgb(0xf4, 0xf8, 0xff),
        seasons::Season::Autumn => rgb(0xd8, 0x8a, 0x3a),
        seasons::Season::Spring => rgb(0x7c, 0xd0, 0x6a),
        seasons::Season::Summer => rgb(0x5c, 0xb8, 0x50),
    }
}

/// The room with the pet in it, the pots, and the buttons.
pub(crate) unsafe fn draw(dc: HDC, art: &mut Surface, mrects: &mut Vec<(RECT, MAct)>, pet: &Pet, v: &View, ix: i32, top: i32, iw: i32, scale: f32) {
    let u = |n: i32| (n as f32 * scale).round() as i32;
    let p = u(2).max(1); // one pixel of the pictures
    let gap = u(6);

    // ---- the room ----
    let (rh, floor_h) = (u(ROOM_H), u(26));
    let wall = rig::mix(theme().panel, CREAM, 0.12);
    fill(dc, ix, top, iw, rh - floor_h, wall);
    fill(dc, ix, top + rh - floor_h, iw, floor_h, rgb(0x9a, 0x6a, 0x44));
    for k in 1..6 {
        fill(dc, ix + k * iw / 6, top + rh - floor_h, 1.max(u(1)), floor_h, rgb(0x86, 0x5a, 0x38));
    }
    // the window shows the time of day and the season
    let (wx, wy, ww, wh) = (ix + u(12), top + u(12), u(78), u(56));
    fill(dc, wx - u(3), wy - u(3), ww + u(6), wh + u(6), CREAM);
    fill(dc, wx, wy, ww, wh, sky(v.look.daytime));
    let night = v.look.daytime == seasons::Daytime::Night;
    if night {
        fill(dc, wx + ww - u(20), wy + u(8), u(10), u(10), CREAM);
        fill(dc, wx + ww - u(16), wy + u(6), u(8), u(8), sky(v.look.daytime));
        for (sx, sy) in [(10, 8), (26, 20), (40, 6), (16, 30)] {
            fill(dc, wx + u(sx), wy + u(sy), p, p, CREAM);
        }
    } else {
        fill(dc, wx + ww - u(22), wy + u(8), u(12), u(12), rgb(0xff, 0xe0, 0x66));
    }
    fill(dc, wx, wy + wh - u(12), ww, u(12), ground(v.look.season));
    let dots = match v.look.season {
        seasons::Season::Winter => Some(CREAM),
        seasons::Season::Autumn => Some(rgb(0xe8, 0x8a, 0x2e)),
        seasons::Season::Spring => Some(PINK),
        seasons::Season::Summer => night.then_some(rgb(0xe6, 0xff, 0x7a)),
    };
    if let Some(c) = dots {
        for (sx, sy) in [(8, 14), (22, 34), (34, 12), (48, 28), (60, 18), (14, 26)] {
            fill(dc, wx + u(sx), wy + u(sy), p, p, c);
        }
    }
    fill(dc, wx + ww / 2 - u(1), wy, u(3), wh, CREAM);
    fill(dc, wx, wy + wh / 2 - u(1), ww, u(3), CREAM);

    // the shelf, with what was picked
    let (sx, sy, sw) = (ix + iw - u(128), top + u(40), u(116));
    fill(dc, sx, sy, sw, u(4), rgb(0x86, 0x5a, 0x38));
    let mut at = sx + u(4);
    if has(v.level, 2) {
        for (k, c) in [TOMATO_RED, UP_BLUE, GREEN, ALERT_ORANGE].into_iter().enumerate() {
            fill(dc, at, sy - u(22) + (k as i32 % 2) * u(3), u(6), u(22) - (k as i32 % 2) * u(3), c);
            at += u(8);
        }
        at += u(4);
    }
    for (seed, n) in v.garden.picked.iter().enumerate().filter(|(_, n)| **n > 0) {
        if at + u(14) > sx + sw {
            break;
        }
        let s = &SEEDS[seed];
        fill(dc, at + u(2), sy - u(10), u(10), u(10), rgb(0xa9, 0xdc, 0xf5));
        fill(dc, at + u(6), sy - u(16), u(2), u(6), rgb(0x5c, 0xb8, 0x50));
        fill(dc, at + u(3), sy - u(22), u(8), u(6), s.petal);
        fill(dc, at + u(5), sy - u(20), u(4), u(2), s.heart);
        if *n > 1 {
            SetTextColor(dc, theme().hint);
            let mut r = rc(at, sy + u(5), u(14), u(12));
            text_at(dc, &n.to_string(), &mut r, DT_SINGLELINE | DT_CENTER);
        }
        at += u(16);
    }
    if has(v.level, 3) {
        // a picture on the wall
        let (px0, py0) = (ix + iw - u(76), top + u(64));
        fill(dc, px0 - u(2), py0 - u(2), u(34), u(24), rgb(0xb8, 0x86, 0x2e));
        fill(dc, px0, py0, u(30), u(20), rgb(0xa8, 0xd8, 0xff));
        fill(dc, px0, py0 + u(12), u(30), u(8), rgb(0x5c, 0xb8, 0x50));
        fill(dc, px0 + u(18), py0 + u(3), u(6), u(6), rgb(0xff, 0xe0, 0x66));
    }
    let feet_y = top + rh - u(8);
    if has(v.level, 0) {
        fill(dc, ix + iw / 2 - u(56), feet_y - u(4), u(112), u(8), TOMATO);
        fill(dc, ix + iw / 2 - u(50), feet_y - u(2), u(100), u(4), rgb(0xff, 0xe0, 0x66));
    }
    if has(v.level, 1) {
        let lp = u(3);
        sprite(dc, &LAMP, ix + iw - u(30), feet_y - LAMP.len() as i32 * lp, lp, &|c| if c == b'Y' && night { Some(rgb(0xff, 0xf0, 0x9a)) } else { pet_color(c) });
    }
    GdiFlush();
    let feet = ((ix + iw / 2) as f32, feet_y as f32);
    let r = Rig { scarf: seasons::scarf(Some(v.look)), ..Rig::REST };
    blit(art, &compose(pet, &r), pet, &r, feet, 2.4 * scale, 1);

    // ---- the pots ----
    let ptop = top + rh + gap;
    let tw = (iw - 2 * gap) / 3;
    let th = u(POT_H);
    for i in 0..POTS {
        let x = ix + i as i32 * (tw + gap);
        let state = v.garden.state(i, v.day);
        let edge = match state {
            State::Flower => GREEN,
            State::Thirsty => UP_BLUE,
            State::Wilted => ALERT_ORANGE,
            _ => theme().panel_edge,
        };
        fill(dc, x, ptop, tw, th, edge);
        fill(dc, x + u(2), ptop + u(2), tw - u(4), th - u(4), theme().panel);
        let pp = u(4);
        let pot_w = POT[0].len() as i32 * pp;
        let (pot_x, pot_y) = (x + (tw - pot_w) / 2, ptop + th - u(20) - POT.len() as i32 * pp);
        sprite(dc, &POT, pot_x, pot_y, pp, &pet_color);
        let pot = v.garden.pots[i];
        if let Some(seed) = pot.seed {
            let g = plant_art(seed, pot.stage);
            let gw = g[0].len() as i32 * pp;
            sprite(dc, g, x + (tw - gw) / 2, pot_y - g.len() as i32 * pp, pp, &plant_colour(seed, state == State::Wilted));
        }
        let (label, colour) = match state {
            State::Empty => (format!("+ {}", SEEDS[v.garden.choice as usize].name), GREEN),
            State::Flower => ("PICK ME".to_string(), GREEN),
            State::Thirsty => ("THIRSTY".to_string(), UP_BLUE),
            State::Wilted => ("WILTED".to_string(), ALERT_ORANGE),
            State::Growing => (format!("{} {}/{}", SEEDS[pot.seed.unwrap_or(0) as usize].name, pot.stage, BLOOM), theme().dim),
        };
        SetTextColor(dc, colour);
        let mut r = rc(x + u(2), ptop + th - u(19), tw - u(4), u(16));
        text_at(dc, &label, &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_END_ELLIPSIS);
        mrects.push((rc(x, ptop, tw, th), MAct::GardenPot(i)));
    }

    // ---- water, and which seed is next ----
    let by = ptop + th + gap;
    let half = (iw - gap) / 2;
    let thirsty = v.garden.thirsty(v.day);
    let water = rc(ix, by, half, u(ROW_H));
    pixel_btn(dc, water, if thirsty > 0 { "WATER" } else { "ALL WATERED" }, if thirsty > 0 { BTN_GREEN } else { theme().panel_light });
    mrects.push((water, MAct::GardenWater));
    let seed = rc(ix + half + gap, by, iw - half - gap, u(ROW_H));
    pixel_btn(dc, seed, &format!("SEED: {}", SEEDS[v.garden.choice as usize].name), theme().panel_light);
    mrects.push((seed, MAct::GardenSeed));
    SetTextColor(dc, theme().hint);
    let hint = match next_thing(v.level) {
        Some((what, at)) => format!("Water once a day; four days to flower. Level {at} brings {} for the room.", what.to_lowercase()),
        None => "Water once a day; four days to flower. Picked ones go on the shelf.".to_string(),
    };
    let mut r = rc(ix + u(4), by + u(ROW_H) + u(3), iw - u(8), u(34));
    text_at(dc, &hint, &mut r, DT_WORDBREAK);
}

const TOMATO_RED: u32 = rgb(0xd6, 0x40, 0x4e);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_flowers_after_four_days_of_water() {
        let mut g = Garden::new();
        assert!(g.plant(0, 100));
        assert!(!g.plant(0, 100)); // taken
        assert_eq!(g.state(0, 100), State::Growing); // planting waters it
        assert_eq!(g.water(100), Watered { had: 1, ..Watered::default() });
        for day in 101..104 {
            assert_eq!(g.state(0, day), State::Thirsty);
            assert_eq!(g.water(day).grew, 1);
        }
        assert_eq!(g.pots[0].stage, 3);
        let w = g.water(104);
        assert_eq!((w.grew, w.bloomed), (1, Some(0)));
        assert_eq!(g.state(0, 104), State::Flower);
        assert_eq!(g.state(0, 200), State::Flower); // a flower waits, it does not wilt
        assert_eq!(g.water(105), Watered::default());
    }

    #[test]
    fn forgotten_plants_wilt_and_water_brings_them_back_without_growing() {
        let mut g = Garden::new();
        g.plant(0, 10);
        assert_eq!(g.state(0, 12), State::Thirsty);
        assert_eq!(g.state(0, 13), State::Wilted);
        let w = g.water(13);
        assert_eq!((w.revived, w.grew), (1, 0));
        assert_eq!(g.pots[0].stage, 0);
        assert_eq!(g.state(0, 13), State::Growing);
        // a cactus can go longer
        g.choice = 3;
        g.plant(1, 10);
        assert_eq!(g.state(1, 15), State::Thirsty);
        assert_eq!(g.state(1, 16), State::Wilted);
        assert_eq!(g.thirsty(16), 2);
    }

    #[test]
    fn a_picked_flower_goes_on_the_shelf_and_frees_the_pot() {
        let mut g = Garden::new();
        g.choice = 1;
        g.plant(2, 0);
        assert_eq!(g.pick(2), None); // not yet
        for d in 1..=4 {
            g.water(d);
        }
        assert_eq!(g.pick(2), Some(1));
        assert_eq!(g.picked, [0, 1, 0, 0, 0]);
        assert_eq!(g.state(2, 5), State::Empty);
        assert_eq!(g.pick(2), None);
    }

    #[test]
    fn the_garden_is_kept_as_text() {
        let mut g = Garden::new();
        g.choice = 4;
        g.plant(1, 2000);
        g.water(2001);
        g.picked = [3, 0, 1, 0, 7];
        let t = g.to_text();
        assert_eq!(Garden::from_text(&t), g);
        assert_eq!(Garden::from_text(""), Garden::new());
        assert_eq!(Garden::from_text("9;9,9,9|x|;a,b"), Garden::new()); // nonsense is ignored
    }

    #[test]
    fn every_picture_is_clean() {
        let all: [&[&[u8]]; 10] = [&POT, &SEED, &SPROUT, &LEAVES, &BUD, &FLOWER, &CACTUS, &TOMATO_PLANT, &LAMP, &ICON_PLANT];
        for g in all {
            for row in g {
                assert_eq!(row.len(), g[0].len());
                assert!(row.iter().all(|c| matches!(c, b'.' | b'X' | b'Y' | b'g' | b'j') || pet_color(*c).is_some()));
            }
        }
        for seed in 0..SEEDS.len() as u8 {
            for stage in 0..=BLOOM {
                assert!(!plant_art(seed, stage).is_empty());
            }
        }
    }

    #[test]
    fn the_room_fills_up_level_by_level() {
        assert_eq!(next_thing(1), Some(("A RUG", 2)));
        assert!(has(2, 0) && !has(2, 1));
        assert_eq!(next_thing(9), None);
        assert!((0..ROOM.len()).all(|i| has(9, i)));
    }
}
