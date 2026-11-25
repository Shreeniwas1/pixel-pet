//! Packs: files with more characters in them. A `.petpack` is plain text: drawings, colours, tools,
//! what a character says, and its own moves as points in time. It is only ever read, never run, and
//! everything in it is checked before it is used. `docs/PACKS.md` says how to write one.
//!
//! The characters that come with the pet are its first pack (`assets/heroes.petpack`).

use crate::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::{LazyLock, RwLock};

/// So many packs can be in use at a time, and so much can be in one.
pub(crate) const MOST: usize = 8;
pub(crate) const MOST_TABS: usize = 8;
pub(crate) const PER_TAB: usize = 16;
const MOST_PETS: usize = MOST_TABS * PER_TAB;
const MOST_MOVES: usize = 200;
const MOST_POINTS: usize = 2000;
const BIGGEST: u64 = 4 << 20;
const MAX_W: i32 = 22;
const MAX_H: i32 = 20;

/// What the files are called.
pub(crate) const ENDING: &str = "petpack";
/// Sent to the pet's own window when a file has been chosen.
pub(crate) const WM_PACK: u32 = WM_APP + 5;

type Why = String;

// ---- colours of their own ----

const EXTRA: usize = 128;
const NO_COLOUR: u32 = u32::MAX;
static COLOURS: [AtomicU32; EXTRA] = [const { AtomicU32::new(NO_COLOUR) }; EXTRA];

/// A colour that is not one of the pet's letters.
pub(crate) fn colour(c: u8) -> Option<u32> {
    let v = COLOURS[c.checked_sub(128)? as usize].load(Relaxed);
    (v != NO_COLOUR).then_some(v)
}

/// The byte that stands for a colour, the same one for the same colour.
fn byte_of(rgb: u32) -> Option<u8> {
    static TURN: Mutex<()> = Mutex::new(());
    let _turn = TURN.lock().unwrap();
    for (i, c) in COLOURS.iter().enumerate() {
        let v = c.load(Relaxed);
        if v == NO_COLOUR || v == rgb {
            c.store(rgb, Relaxed);
            return Some(128 + i as u8);
        }
    }
    None
}

// ---- moves ----

/// What flies off while a move goes on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Puff {
    Trail,
    Shards,
    Smoke,
    Spark,
    Ash,
    Petal,
    Heart,
    Zed,
}

/// A blow, at one moment of a move.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Hit {
    Ground,
    Front,
    Dust,
    Click,
}

const PUFFS: [(&str, Puff); 8] = [
    ("trail", Puff::Trail),
    ("shards", Puff::Shards),
    ("smoke", Puff::Smoke),
    ("spark", Puff::Spark),
    ("ash", Puff::Ash),
    ("petal", Puff::Petal),
    ("heart", Puff::Heart),
    ("zed", Puff::Zed),
];
const HITS: [(&str, Hit); 4] = [("ground", Hit::Ground), ("front", Hit::Front), ("dust", Hit::Dust), ("click", Hit::Click)];
const EYES: [(&str, Eyes); 7] = [
    ("open", Eyes::Open),
    ("half", Eyes::Half),
    ("closed", Eyes::Closed),
    ("happy", Eyes::Happy),
    ("wide", Eyes::Wide),
    ("angry", Eyes::Angry),
    ("dizzy", Eyes::Dizzy),
];
const YES_NO: [(&str, bool); 2] = [("yes", true), ("no", false)];

type Set = fn(&mut Rig, f32);
/// What a move can change, between which values, and how.
const PROPS: [(&str, f32, f32, Set); 25] = [
    ("dx", -6.0, 6.0, |r, v| r.dx = (r.dx + v).clamp(-6.0, 6.0)),
    ("dy", -4.0, 12.0, |r, v| r.dy = (r.dy + v).clamp(-4.0, 13.0)),
    ("sx", 0.4, 1.6, |r, v| r.sx *= v),
    ("sy", 0.43, 1.6, |r, v| r.sy *= v),
    ("shear", -1.0, 1.0, |r, v| r.shear = v),
    ("rot", -1080.0, 1080.0, |r, v| r.rot = v),
    ("pivot", 0.0, 1.0, |r, v| r.pivot = v),
    ("leg1", -3.0, 3.0, |r, v| r.legs[0].0 = v),
    ("lift1", 0.0, 3.0, |r, v| r.legs[0].1 = v),
    ("leg2", -3.0, 3.0, |r, v| r.legs[1].0 = v),
    ("lift2", 0.0, 3.0, |r, v| r.legs[1].1 = v),
    ("crouch", 0.0, 1.0, |r, v| r.crouch = v),
    ("arm1", -60.0, 200.0, |r, v| r.arms[0] = v),
    ("arm2", -60.0, 200.0, |r, v| r.arms[1] = v),
    ("headx", -3.0, 3.0, |r, v| r.head.0 = v),
    ("heady", -3.0, 3.0, |r, v| r.head.1 = v),
    ("look", -1.0, 1.0, |r, v| r.look = v),
    ("turn", 0.0, 1.0, |r, v| r.turn = v),
    ("mouth", 0.0, 1.0, |r, v| r.mouth = v),
    ("alpha", 0.0, 1.0, |r, v| r.alpha = v),
    ("aura", 0.0, 1.0, |r, v| r.aura = v),
    ("reach", 0.3, 3.0, |r, v| r.reach = v),
    ("wing", 0.0, 1.0, |r, v| r.wing = v),
    ("clones", 0.0, 1.0, |r, v| r.clones = v),
    ("tail", -1.0, 1.0, |r, v| r.tail = v),
];

type MakeFx = fn(f32, f32) -> Fx;
/// The effects, how many numbers each takes and how big the first may be.
const FX: [(&str, usize, f32, MakeFx); 34] = [
    ("none", 0, 1.0, |_, _| Fx::None),
    ("ray", 1, 1.2, |k, _| Fx::Ray(k)),
    ("eyeray", 1, 1.2, |k, _| Fx::EyeRay(k)),
    ("rays", 1, 1.2, |k, _| Fx::Rays(k)),
    ("ring", 1, 1.0, |k, _| Fx::Ring(k)),
    ("portal", 1, 1.0, |k, _| Fx::Portal(k)),
    ("bolt", 1, 1.0, |k, _| Fx::Bolt(k)),
    ("orbs", 2, 1.0, Fx::Orbs),
    ("sigil", 2, 1.0, Fx::Sigil),
    ("thrown", 1, 1.0, |k, _| Fx::Thrown(k)),
    ("arrow", 2, 1.0, Fx::Arrow),
    ("thread", 0, 1.0, |_, _| Fx::Thread),
    ("hang", 0, 1.0, |_, _| Fx::Hang),
    ("slash", 1, 1.0, |k, _| Fx::Slash(k)),
    ("cut", 1, 1.0, |k, _| Fx::Cut(k)),
    ("claws", 0, 1.0, |_, _| Fx::Claws),
    ("fist", 1, 1.0, |k, _| Fx::Fist(k)),
    ("hoops", 2, 1.0, Fx::Hoops),
    ("spikes", 1, 1.0, |k, _| Fx::Spikes(k)),
    ("moon", 1, 1.0, |k, _| Fx::Moon(k)),
    ("flash", 1, 1.0, |k, _| Fx::Flash(k)),
    ("jets", 1, 1.0, |k, _| Fx::Jets(k)),
    ("lights", 1, 1.0, |k, _| Fx::Lights(k)),
    ("heart", 1, 1.0, |k, _| Fx::Heart(k)),
    ("radar", 2, 1.0, Fx::Radar),
    ("flowers", 1, 1.0, |k, _| Fx::Flowers(k)),
    ("dome", 1, 1.0, |k, _| Fx::Dome(k)),
    ("fire", 2, 1.0, Fx::Fire),
    ("board", 1, 1.0, |k, _| Fx::Board(k)),
    ("limbs", 2, 1.0, Fx::Limbs),
    ("bomb", 1, 2.0, |k, _| Fx::Bomb(k)),
    ("cards", 1, 1.0, |k, _| Fx::Cards(k)),
    ("firewings", 2, 1.0, Fx::FireWings),
    ("scrap", 2, 1.0, Fx::Scrap),
];

/// A move from a pack.
#[derive(Debug)]
pub(crate) struct Move {
    pub secs: f32,
    pub speed: Option<(f32, f32, f32)>, // cells per second, from when, to when
    pub puff: Option<(Puff, f32, f32, f32)>, // what, how often, from when, to when
    pub hit: Option<(Hit, f32)>,
    pub warp: Option<f32>, // when it is suddenly somewhere else
    tracks: Vec<(usize, Vec<(f32, f32)>)>,
    eyes: Vec<(f32, Eyes)>,
    gone: Vec<(f32, bool)>,
    infront: Vec<(f32, bool)>,
    fx: Vec<(f32, usize, f32, f32)>,
}

static MOVES: RwLock<Vec<&'static Move>> = RwLock::new(Vec::new());

/// The move behind `Clip::Own(n)`.
pub(crate) fn own(n: u16) -> &'static Move {
    MOVES.read().unwrap()[n as usize]
}

/// The last point at or before a moment.
fn step<T: Copy>(p: &[(f32, T)], t: f32) -> Option<T> {
    p[..p.partition_point(|q| q.0 <= t)].last().or(p.first()).map(|q| q.1)
}

fn between(p: &[(f32, f32)], t: f32) -> f32 {
    let i = p.partition_point(|q| q.0 <= t);
    match (i.checked_sub(1).map(|j| p[j]), p.get(i)) {
        (Some(a), Some(b)) => a.1 + (b.1 - a.1) * ((t - a.0) / (b.0 - a.0).max(1e-6)),
        (Some(a), None) => a.1,
        (None, Some(b)) => b.1,
        _ => 0.0,
    }
}

impl Move {
    pub fn travel(&self, t: f32) -> f32 {
        self.speed.map_or(0.0, |(v, a, b)| if (a..b).contains(&t) { v } else { 0.0 })
    }

    /// Put the body into this move's pose.
    pub fn pose(&self, r: &mut Rig, t: f32) {
        for (p, pts) in &self.tracks {
            PROPS[*p].3(r, between(pts, t));
        }
        (r.sx, r.sy) = (r.sx.clamp(0.4, 1.7), r.sy.clamp(0.43, 1.7));
        r.eyes = step(&self.eyes, t).unwrap_or(r.eyes);
        r.gone = step(&self.gone, t).unwrap_or(r.gone);
        r.arms_front = step(&self.infront, t).unwrap_or(r.arms_front);
        let i = self.fx.partition_point(|q| q.0 <= t);
        if let Some(a) = i.checked_sub(1).map(|j| self.fx[j]).or(self.fx.first().copied()) {
            let (mut k, mut v) = (a.2, a.3);
            if let Some(b) = self.fx.get(i).filter(|b| b.1 == a.1 && b.0 > a.0 && t >= a.0) {
                let f = (t - a.0) / (b.0 - a.0);
                (k, v) = (a.2 + (b.2 - a.2) * f, a.3 + (b.3 - a.3) * f);
            }
            r.fx = FX[a.1].3(k, v);
        }
    }
}

// ---- reading a file ----

struct Entry {
    line: usize,
    key: String,
    value: String,
    rows: Vec<String>,
    used: std::cell::Cell<bool>,
}

struct Section {
    line: usize,
    kind: String,
    name: String,
    entries: Vec<Entry>,
}

fn at(line: usize, what: impl std::fmt::Display) -> Why {
    format!("Line {line}: {what}")
}

impl Section {
    fn get(&self, key: &str) -> Option<&Entry> {
        let e = self.entries.iter().find(|e| e.key == key)?;
        e.used.set(true);
        Some(e)
    }

    fn need(&self, key: &str) -> Result<&Entry, Why> {
        self.get(key).ok_or_else(|| at(self.line, format!("[{} {}] has no \"{key}\"", self.kind, self.name)))
    }

    /// Everything has been understood, and nothing was said twice.
    fn done(&self) -> Result<(), Why> {
        match self.entries.iter().find(|e| !e.used.get()) {
            Some(e) => Err(at(e.line, format!("\"{}\" means nothing here, or is there twice", e.key))),
            None => Ok(()),
        }
    }
}

impl Entry {
    fn err(&self, what: impl std::fmt::Display) -> Why {
        at(self.line, format!("{}: {what}", self.key))
    }

    fn words(&self) -> Vec<&str> {
        self.value.split_whitespace().collect()
    }

    fn number(&self, w: &str, lo: f32, hi: f32) -> Result<f32, Why> {
        match w.parse::<f32>() {
            Ok(v) if v.is_finite() && (lo..=hi).contains(&v) => Ok(v),
            _ => Err(self.err(format!("\"{w}\" should be a number from {lo} to {hi}"))),
        }
    }

    fn whole(&self, w: &str, lo: i32, hi: i32) -> Result<i32, Why> {
        match w.parse::<i32>() {
            Ok(v) if (lo..=hi).contains(&v) => Ok(v),
            _ => Err(self.err(format!("\"{w}\" should be a whole number from {lo} to {hi}"))),
        }
    }

    fn one_of<T: Copy>(&self, w: &str, list: &[(&str, T)]) -> Result<T, Why> {
        match list.iter().find(|(n, _)| *n == w) {
            Some((_, v)) => Ok(*v),
            None => Err(self.err(format!("\"{w}\" should be one of: {}", list.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))),
        }
    }

    fn count(&self, n: std::ops::RangeInclusive<usize>) -> Result<Vec<&str>, Why> {
        let w = self.words();
        if n.contains(&w.len()) { Ok(w) } else { Err(self.err(format!("it has {} parts instead of {}", w.len(), n.start()))) }
    }
}

/// Words that will be shown: short, and nothing strange in them.
fn plain(e: &Entry, most: usize) -> Result<String, Why> {
    let t = e.value.trim();
    if t.is_empty() || t.chars().count() > most || t.chars().any(char::is_control) {
        return Err(e.err(format!("it should be 1 to {most} letters")));
    }
    Ok(t.to_string())
}

/// Names that end up in file names and in the settings.
fn is_id(s: &str) -> bool {
    (1..=24).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.starts_with('-')
}

fn sections(text: &str) -> Result<Vec<Section>, Why> {
    let mut lines = text.trim_start_matches('\u{feff}').lines().enumerate().map(|(i, l)| (i + 1, l.trim_end()));
    let first = lines.next().map_or("", |l| l.1);
    match first.strip_prefix("petpack ").map(str::trim) {
        Some("1") => {}
        Some(v) if v.parse::<u32>().is_ok() => return Err("This pack needs a newer Pixel Pet.".into()),
        _ => return Err("This is not a pack for Pixel Pet.".into()),
    }
    let new = |line, kind: &str, name: &str| Section { line, kind: kind.into(), name: name.into(), entries: Vec::new() };
    let mut out = vec![new(1, "pack", "")];
    for (n, l) in lines {
        if l.trim().is_empty() || l.starts_with('#') {
            continue;
        }
        let s = out.last_mut().expect("there is always one");
        if let Some(row) = l.strip_prefix('|') {
            let Some(e) = s.entries.last_mut() else { return Err(at(n, "a drawing without a name")) };
            if e.rows.len() > 64 {
                return Err(at(n, "this drawing is too high"));
            }
            e.rows.push(row.strip_prefix(' ').unwrap_or(row).to_string());
        } else if let Some(h) = l.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            let (kind, name) = h.split_once(' ').unwrap_or((h, ""));
            if !["tool", "move", "pet"].contains(&kind) || !is_id(name.trim()) {
                return Err(at(n, format!("[{h}] should be a tool, a move or a pet, and a name of small letters, digits and -")));
            }
            out.push(new(n, kind, name.trim()));
            if out.len() > 600 {
                return Err(at(n, "there is too much in this pack"));
            }
        } else if let Some((k, v)) = l.split_once('=') {
            s.entries.push(Entry { line: n, key: k.trim().to_string(), value: v.trim().to_string(), rows: Vec::new(), used: std::cell::Cell::new(false) });
        } else {
            return Err(at(n, format!("\"{}\" is not understood", l.chars().take(40).collect::<String>())));
        }
    }
    Ok(out)
}

/// Letters to colours, those of the pack before those of the pet.
struct Paint(HashMap<char, u8>);

impl Paint {
    fn byte(&self, c: char) -> Option<u8> {
        self.0.get(&c).copied().or_else(|| u8::try_from(c).ok().filter(|b| b.is_ascii() && pet_color(*b).is_some()))
    }

    fn of(&self, e: &Entry, w: &str) -> Result<u8, Why> {
        let mut c = w.chars();
        match (c.next().and_then(|c| self.byte(c)), c.next()) {
            (Some(b), None) => Ok(b),
            _ => Err(e.err(format!("\"{w}\" is not a colour"))),
        }
    }

    fn grid(&self, e: &Entry, w: i32, h: i32) -> Result<Vec<Vec<u8>>, Why> {
        let mut out: Vec<Vec<u8>> = Vec::new();
        for (i, row) in e.rows.iter().enumerate() {
            let mut line = Vec::new();
            for c in row.chars() {
                line.push(if c == '.' { b'.' } else { self.byte(c).ok_or_else(|| at(e.line + 1 + i, format!("'{c}' is not a colour")))? });
            }
            if line.len() > w as usize || out.first().is_some_and(|f| f.len() != line.len()) {
                return Err(at(e.line + 1 + i, format!("every row should be as long as the first, and at most {w}")));
            }
            out.push(line);
        }
        if out.is_empty() || out[0].is_empty() || out.len() > h as usize {
            return Err(e.err(format!("it should have 1 to {h} rows")));
        }
        if out.iter().flatten().all(|c| *c == b'.') {
            return Err(e.err("there is nothing to see"));
        }
        Ok(out)
    }
}

fn read_move(s: &Section) -> Result<Move, Why> {
    let e = s.need("secs")?;
    let secs = e.number(&e.value, 0.3, 8.0)?;
    /// "0.5:value 1:value", in order of time.
    fn points<T>(e: &Entry, secs: f32, mut read: impl FnMut(&str) -> Result<T, Why>) -> Result<Vec<(f32, T)>, Why> {
        let mut out: Vec<(f32, T)> = Vec::new();
        for w in e.words() {
            let Some((t, v)) = w.split_once(':') else { return Err(e.err(format!("\"{w}\" should be a time and a value, like 0.5:10"))) };
            let t = e.number(t, 0.0, secs + 0.01)?;
            if out.last().is_some_and(|l| l.0 > t) || out.len() >= MOST_POINTS {
                return Err(e.err(format!("\"{w}\" comes too late, or there are too many")));
            }
            out.push((t, read(v)?));
        }
        if out.is_empty() { Err(e.err("there is nothing")) } else { Ok(out) }
    }
    let speed = match s.get("speed") {
        Some(e) => {
            let w = e.count(3..=3)?;
            Some((e.number(w[0], 0.0, 150.0)?, e.number(w[1], 0.0, secs)?, e.number(w[2], 0.0, secs)?))
        }
        None => None,
    };
    let puff = match s.get("puff") {
        Some(e) => {
            let w = e.count(2..=4)?;
            let (a, b) = if w.len() == 4 { (e.number(w[2], 0.0, secs)?, e.number(w[3], 0.0, secs)?) } else { (0.0, secs) };
            Some((e.one_of(w[0], &PUFFS)?, e.number(w[1], 0.03, 8.0)?, a, b))
        }
        None => None,
    };
    let hit = match s.get("hit") {
        Some(e) => {
            let w = e.count(2..=2)?;
            Some((e.one_of(w[0], &HITS)?, e.number(w[1], 0.0, secs)?))
        }
        None => None,
    };
    let warp = match s.get("warp") {
        Some(e) => Some(e.number(&e.value, 0.0, secs)?),
        None => None,
    };
    let mut tracks = Vec::new();
    for (i, (name, lo, hi, _)) in PROPS.iter().enumerate() {
        if let Some(e) = s.get(name) {
            tracks.push((i, points(e, secs, |v| e.number(v, *lo, *hi))?));
        }
    }
    let eyes = s.get("eyes").map_or(Ok(Vec::new()), |e| points(e, secs, |v| e.one_of(v, &EYES)))?;
    let gone = s.get("gone").map_or(Ok(Vec::new()), |e| points(e, secs, |v| e.one_of(v, &YES_NO)))?;
    let infront = s.get("infront").map_or(Ok(Vec::new()), |e| points(e, secs, |v| e.one_of(v, &YES_NO)))?;
    let fx = match s.get("fx") {
        Some(e) => points(e, secs, |v| {
            let mut w = v.split(',');
            let name = w.next().unwrap_or("");
            let Some(i) = FX.iter().position(|f| f.0 == name) else {
                return Err(e.err(format!("\"{name}\" should be one of: {}", FX.iter().map(|f| f.0).collect::<Vec<_>>().join(", "))));
            };
            let v: Vec<&str> = w.collect();
            if v.len() != FX[i].1 {
                return Err(e.err(format!("{name} takes {} numbers", FX[i].1)));
            }
            let k = v.first().map_or(Ok(0.0), |v| e.number(v, 0.0, FX[i].2))?;
            let t = v.get(1).map_or(Ok(0.0), |v| e.number(v, 0.0, 100.0))?;
            Ok((i, k, t))
        })?
        .into_iter()
        .map(|(t, (i, k, v))| (t, i, k, v))
        .collect(),
        None => Vec::new(),
    };
    s.done()?;
    Ok(Move { secs, speed, puff, hit, warp, tracks, eyes, gone, infront, fx })
}

enum Trick {
    Known(Clip),
    Own(usize),
}

struct NewPet {
    id: String,
    tab: String,
    was: Option<usize>,
    name: String,
    face: u8,
    eye: u8,
    head: Vec<Vec<u8>>,
    torso: Vec<Vec<u8>>,
    build: Build,
    legs: Legs,
    arms: Option<Arms>,
    tail: Option<Tail>,
    back: Back,
    tool: Option<usize>,
    glow: u8,
    gift: Gift,
    aloft: Aloft,
    door: bool,
    disc: Vec<u8>,
    lights: Vec<u8>,
    board: [u8; 3],
    mouth: (f32, i32),
    voice: usize,
    says: Vec<(Option<Ev>, String)>,
    foods: [usize; 3],
    tricks: Vec<Trick>,
}

/// A pack that has been read and found to be in order.
pub(crate) struct Parsed {
    pub id: String,
    pub name: String,
    tools: Vec<(Vec<Vec<u8>>, (i32, i32))>,
    moves: Vec<Move>,
    pets: Vec<NewPet>,
}

impl Parsed {
    pub fn len(&self) -> usize {
        self.pets.len()
    }
}

const VOICES: [(&str, usize); 5] = [("aloof", 0), ("eager", 1), ("gentle", 2), ("formal", 3), ("machine", VOICE_MACHINE)];
const BUILDS: [(&str, Build); 3] = [("legs", Build::Legs), ("wheels", Build::Wheels), ("hover", Build::Hover)];
const GIFTS: [(&str, Gift); 7] = [
    ("none", Gift::None),
    ("fly", Gift::Fly),
    ("leap", Gift::Leap),
    ("web", Gift::Web),
    ("cable", Gift::Cable),
    ("limb", Gift::Limb),
    ("warp", Gift::Warp),
];
const ALOFT: [(&str, Aloft); 6] = [
    ("jets", Aloft::Jets),
    ("board", Aloft::Board),
    ("fire", Aloft::Fire),
    ("firewings", Aloft::FireWings),
    ("orbs", Aloft::Orbs),
    ("ring", Aloft::Ring),
];
const EVENTS: [(&str, Ev); 12] = [
    ("wake", Ev::Wake),
    ("morning", Ev::Morning),
    ("focus", Ev::Focus),
    ("task", Ev::Task),
    ("pet", Ev::Pet),
    ("late", Ev::Late),
    ("tired", Ev::Tired),
    ("sad", Ev::Sad),
    ("back", Ev::Back),
    ("hungry", Ev::Hungry),
    ("fed", Ev::Fed),
    ("hit", Ev::Hit),
];

fn read_pet(s: &Section, paint: &Paint, tools: &[&Section], moves: &[&Section]) -> Result<NewPet, Why> {
    let colour = |key: &str, or: u8| s.get(key).map_or(Ok(or), |e| paint.of(e, &e.value));
    let letters = |key: &str, most: usize| -> Result<Vec<u8>, Why> {
        let Some(e) = s.get(key) else { return Ok(Vec::new()) };
        let v: Option<Vec<u8>> = e.value.chars().filter(|c| !c.is_whitespace()).map(|c| paint.byte(c)).collect();
        v.filter(|v| (1..=most).contains(&v.len())).ok_or_else(|| e.err(format!("it should be 1 to {most} colours")))
    };
    let name = plain(s.need("name")?, 12)?;
    let tab = plain(s.need("tab")?, 9)?.to_uppercase();
    let was = match s.get("was") {
        Some(e) => Some(e.whole(&e.value, 16, 999)? as usize),
        None => None,
    };
    let head = paint.grid(s.need("head")?, MAX_W, MAX_H)?;
    let torso = s.get("torso").map_or(Ok(Vec::new()), |e| paint.grid(e, MAX_W, MAX_H))?;
    let build = s.get("build").map_or(Ok(Build::Legs), |e| e.one_of(&e.value, &BUILDS))?;
    let e = s.need("legs")?;
    let w = e.count(4..=4)?;
    let legs = Legs { hip: e.number(w[0], 0.0, 10.0)?, len: e.whole(w[1], 1, 8)?, c: paint.of(e, w[2])?, foot: paint.of(e, w[3])? };
    let arms = match s.get("arms") {
        Some(e) => {
            let w = e.count(3..=4)?;
            Some(Arms { len: e.whole(w[0], 1, 6)?, c: paint.of(e, w[1])?, hand: paint.of(e, w[2])?, fat: w.get(3).map_or(Ok(1.6), |w| e.number(w, 0.6, 4.0))? })
        }
        None => None,
    };
    let tail = match s.get("tail") {
        Some(e) => {
            let w = e.count(3..=3)?;
            Some(Tail { len: e.whole(w[0], 1, 7)?, c: paint.of(e, w[1])?, tip: paint.of(e, w[2])? })
        }
        None => None,
    };
    let mut disc = Vec::new();
    let back = match s.get("back") {
        Some(e) => {
            let w = e.words();
            let n = |i: usize| w.get(i).map_or_else(|| Err(e.err("a colour is missing")), |w| paint.of(e, w));
            let (b, parts) = match w.first().copied().unwrap_or("") {
                "none" => (Back::None, 1),
                "cape" => (Back::Cape(n(1)?), 2),
                "wings" => (Back::Wings(n(1)?, n(2)?), 3),
                "pack" => (Back::Pack(n(1)?), 2),
                "swords" => (Back::Swords, 1),
                "limbs" if w.get(2) == Some(&"claws") => (Back::Limbs(n(1)?, true), 3),
                "limbs" => (Back::Limbs(n(1)?, false), 2),
                "disc" => {
                    // the rings from the outside in, and the dot in the middle
                    let rings = w.get(1).copied().unwrap_or("");
                    disc = rings.chars().map(|c| paint.byte(c)).collect::<Option<Vec<u8>>>().filter(|d| (2..=5).contains(&d.len())).ok_or_else(|| e.err("a disc has 2 to 5 rings"))?;
                    (Back::Disc, 2 + w.get(2).map_or(Ok(0), |w| paint.of(e, w).map(|c| { disc.push(c); 1 }))?)
                }
                b => return Err(e.err(format!("\"{b}\" should be one of: none, cape, wings, pack, swords, limbs, disc"))),
            };
            if w.len() != parts {
                return Err(e.err("there is too much"));
            }
            if b == Back::Disc && w.len() == 2 {
                disc.push(*disc.last().expect("it has rings"));
            }
            b
        }
        None => Back::None,
    };
    let tool = match s.get("tool") {
        Some(e) => Some(tools.iter().position(|t| t.name == e.value).ok_or_else(|| e.err(format!("there is no [tool {}]", e.value)))?),
        None => None,
    };
    let e = s.need("mouth")?;
    let w = e.count(2..=2)?;
    let mouth = (e.number(w[0], -10.0, 10.0)?, e.whole(w[1], 0, head.len() as i32 - 1)?);
    let e = s.need("foods")?;
    let mut foods = [0; 3];
    for (f, w) in foods.iter_mut().zip(e.count(3..=3)?) {
        *f = food_named(w).ok_or_else(|| e.err(format!("there is no \"{w}\" to eat")))?;
    }
    let e = s.need("tricks")?;
    let mut tricks = Vec::new();
    for w in e.count(1..=8)? {
        tricks.push(match (moves.iter().position(|m| m.name == w), Clip::named(w)) {
            (Some(i), _) => Trick::Own(i),
            (None, Some(c)) => Trick::Known(c),
            _ => return Err(e.err(format!("there is no move \"{w}\""))),
        });
    }
    let mut says = Vec::new();
    for e in s.entries.iter().filter(|e| e.key == "says" || e.key.starts_with("says ")) {
        e.used.set(true);
        let ev = match e.key.strip_prefix("says ") {
            Some(w) => Some(e.one_of(w.trim(), &EVENTS)?),
            None => None,
        };
        says.push((ev, plain(e, 80)?));
        if says.len() > 40 {
            return Err(e.err("it says too much"));
        }
    }
    let board = letters("board", 3)?;
    let p = NewPet {
        id: s.name.clone(),
        tab,
        was,
        name,
        face: colour("face", b'S')?,
        eye: colour("eye", b'K')?,
        build,
        legs,
        arms,
        tail,
        back,
        tool,
        glow: colour("glow", b'Y')?,
        gift: s.get("gift").map_or(Ok(Gift::None), |e| e.one_of(&e.value, &GIFTS))?,
        aloft: s.get("aloft").map_or(Ok(Aloft::Jets), |e| e.one_of(&e.value, &ALOFT))?,
        door: s.get("door").map_or(Ok(false), |e| e.one_of(&e.value, &YES_NO))?,
        disc,
        lights: letters("lights", 6)?,
        board: [board.first().copied().unwrap_or(b'V'), board.get(1).copied().unwrap_or(b'i'), board.get(2).copied().unwrap_or(0)],
        mouth,
        voice: s.get("voice").map_or(Ok(3), |e| e.one_of(&e.value, &VOICES))?,
        says,
        foods,
        tricks,
        head,
        torso,
    };
    s.done()?;
    // it has to fit the window
    let h = p.head.len() as i32 + (p.torso.len() as i32 - 1).max(0);
    let lift = match build {
        Build::Legs => p.legs.len + 1,
        Build::Wheels => p.legs.len / 2,
        Build::Hover => p.legs.len,
    };
    if h + lift > MAX_H {
        return Err(at(s.line, format!("{} is {} cells high, {MAX_H} is the most", p.name, h + lift)));
    }
    Ok(p)
}

/// Read a pack. Says what is wrong with it, and where.
pub(crate) fn parse(text: &str) -> Result<Parsed, Why> {
    let all = sections(text)?;
    let top = &all[0];
    let e = top.need("id")?;
    if !is_id(&e.value) {
        return Err(e.err("it should be small letters, digits and -, at most 24"));
    }
    let id = e.value.clone();
    let name = plain(top.need("name")?, 24)?;
    if let Some(e) = top.get("about") {
        plain(e, 300)?;
    }
    if let Some(e) = top.get("by") {
        plain(e, 60)?;
    }
    let mut paint = Paint(HashMap::new());
    for e in top.entries.iter().filter(|e| e.key.starts_with("colour ")) {
        e.used.set(true);
        let mut c = e.key["colour ".len()..].trim().chars();
        let (Some(letter), None) = (c.next(), c.next()) else { return Err(e.err("a colour has one letter")) };
        // the outline, the eyes and nothing keep their meaning
        if !letter.is_ascii_graphic() || ".KE".contains(letter) {
            return Err(e.err(format!("'{letter}' cannot be a colour")));
        }
        let hex = e.value.strip_prefix('#').filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()).ok_or_else(|| e.err("it should be like #ff8800"))?;
        let c = rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        // this very colour is what cannot be seen
        let c = if c == KEY { c ^ 1 } else { c };
        paint.0.insert(letter, byte_of(c).ok_or_else(|| e.err("there are too many colours in all packs together"))?);
    }
    top.done()?;
    let of = |kind: &str| all.iter().filter(|s| s.kind == kind).collect::<Vec<_>>();
    let (tools, moves, pets) = (of("tool"), of("move"), of("pet"));
    for list in [&tools, &moves, &pets] {
        for (i, s) in list.iter().enumerate() {
            if list[..i].iter().any(|o| o.name == s.name) {
                return Err(at(s.line, format!("there are two called [{} {}]", s.kind, s.name)));
            }
        }
    }
    if pets.is_empty() || pets.len() > MOST_PETS || moves.len() > MOST_MOVES {
        return Err(format!("A pack has 1 to {MOST_PETS} characters and at most {MOST_MOVES} moves."));
    }
    let mut out = Parsed { id, name, tools: Vec::new(), moves: Vec::new(), pets: Vec::new() };
    for s in &tools {
        let art = paint.grid(s.need("art")?, 16, 16)?;
        let e = s.need("grip")?;
        let w = e.count(2..=2)?;
        let grip = (e.whole(w[0], 0, art[0].len() as i32 - 1)?, e.whole(w[1], 0, art.len() as i32 - 1)?);
        s.done()?;
        out.tools.push((art, grip));
    }
    for s in &moves {
        out.moves.push(read_move(s)?);
    }
    for s in &pets {
        let p = read_pet(s, &paint, &tools, &moves)?;
        if out.pets.iter().any(|q| q.name.eq_ignore_ascii_case(&p.name)) {
            return Err(at(s.line, format!("there are two called {}", p.name)));
        }
        if out.pets.iter().filter(|q| q.tab == p.tab).count() >= PER_TAB {
            return Err(at(s.line, format!("a tab has room for {PER_TAB}")));
        }
        out.pets.push(p);
    }
    let tabs: std::collections::HashSet<&String> = out.pets.iter().map(|p| &p.tab).collect();
    if tabs.len() > MOST_TABS {
        return Err(format!("A pack has at most {MOST_TABS} tabs."));
    }
    Ok(out)
}

// ---- everybody there is ----

struct Item {
    pet: &'static Pet,
    key: String, // how the settings call it
    tab: &'static str,
    pack: usize,
    was: Option<usize>,
    there: bool,
}

struct Info {
    id: String,
    name: String,
    file: Option<PathBuf>,
    seen: Option<(u64, std::time::SystemTime)>,
    there: bool,
}

/// Only ever grows while the pet runs, so a number always means the same character.
struct All {
    items: Vec<Item>,
    packs: Vec<Info>,
}

fn keep<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

fn keep_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn keep_grid(g: Vec<Vec<u8>>) -> &'static [&'static [u8]] {
    keep(g.into_iter().map(keep).collect())
}

impl All {
    /// Take in a pack. One with the same id makes room for it.
    fn add(&mut self, p: Parsed, file: Option<PathBuf>) -> usize {
        let pack = if file.is_none() {
            0
        } else {
            for i in 0..self.packs.len() {
                if self.packs[i].id == p.id && self.packs[i].there {
                    self.drop_pack(i);
                }
            }
            let seen = file.as_deref().and_then(stamp);
            self.packs.push(Info { id: p.id.clone(), name: p.name.clone(), file, seen, there: true });
            self.packs.len() - 1
        };
        let first = self.items.len();
        let base = {
            let mut m = MOVES.write().unwrap();
            let base = m.len();
            m.extend(p.moves.into_iter().map(|v| &*Box::leak(Box::new(v))));
            base
        };
        let tools: Vec<&'static Tool> = p.tools.into_iter().map(|(g, grip)| &*Box::leak(Box::new(Tool { grid: keep_grid(g), grip }))).collect();
        for n in p.pets {
            let tricks = n.tricks.iter().map(|t| match t {
                Trick::Known(c) => *c,
                Trick::Own(i) => Clip::Own((base + i) as u16),
            });
            let pet: &'static Pet = Box::leak(Box::new(Pet {
                name: keep_str(n.name),
                eye: n.eye,
                face: n.face,
                head: keep_grid(n.head),
                torso: keep_grid(n.torso),
                build: n.build,
                legs: n.legs,
                arms: n.arms,
                tail: n.tail,
                gear: Gear {
                    back: n.back,
                    tool: n.tool.map(|i| tools[i]),
                    glow: n.glow,
                    gift: n.gift,
                    aloft: n.aloft,
                    door: n.door,
                    disc: keep(n.disc),
                    lights: keep(n.lights),
                    board: n.board,
                },
                mouth: n.mouth,
                voice: n.voice,
                says: keep(n.says.into_iter().map(|(e, t)| (e, keep_str(t))).collect()),
                foods: n.foods,
                tricks: keep(tricks.collect()),
            }));
            self.items.push(Item { pet, key: format!("{}/{}", p.id, n.id), tab: keep_str(n.tab), pack, was: n.was, there: true });
        }
        first
    }

    fn drop_pack(&mut self, i: usize) {
        self.packs[i].there = false;
        self.items.iter_mut().filter(|t| t.pack == i).for_each(|t| t.there = false);
    }
}

const BUILT_IN: &str = include_str!("../assets/heroes.petpack");

static ALL: LazyLock<RwLock<All>> = LazyLock::new(|| {
    let mut all = All { items: Vec::new(), packs: vec![Info { id: "pixel-pet".into(), name: "Pixel Pet".into(), file: None, seen: None, there: true }] };
    for (i, pet) in OURS.iter().enumerate() {
        let tab = OUR_TABS.iter().find(|(_, g)| g.contains(&i)).map_or("", |t| t.0);
        all.items.push(Item { pet, key: i.to_string(), tab, pack: 0, was: None, there: true });
    }
    match parse(BUILT_IN) {
        Ok(p) => {
            all.add(p, None);
        }
        Err(e) => debug_assert!(false, "heroes.petpack: {e}"),
    }
    if !cfg!(test) {
        scan(&mut all);
    }
    RwLock::new(all)
});

/// Every character, by number. The first sixteen never change.
pub(crate) struct Pets;
pub(crate) static PETS: Pets = Pets;

#[cfg(test)]
impl Pets {
    pub fn len(&self) -> usize {
        ALL.read().unwrap().items.len()
    }

    pub fn iter(&self) -> std::vec::IntoIter<&'static Pet> {
        ALL.read().unwrap().items.iter().map(|t| t.pet).collect::<Vec<_>>().into_iter()
    }
}

impl std::ops::Index<usize> for Pets {
    type Output = Pet;
    fn index(&self, i: usize) -> &Pet {
        let all = ALL.read().unwrap();
        all.items.get(i).unwrap_or(&all.items[0]).pet
    }
}

/// One tab of the picker.
pub(crate) struct Tab {
    pub name: &'static str,
    pub pack: usize,
    pub pets: Vec<usize>,
}

/// The picker's tabs, pack by pack.
pub(crate) fn tabs() -> Vec<Tab> {
    let all = ALL.read().unwrap();
    let mut out: Vec<Tab> = Vec::new();
    for (i, t) in all.items.iter().enumerate().filter(|(_, t)| t.there) {
        match out.iter_mut().find(|o| o.pack == t.pack && o.name == t.tab) {
            Some(o) => o.pets.push(i),
            None => out.push(Tab { name: t.tab, pack: t.pack, pets: vec![i] }),
        }
    }
    out.sort_by_key(|t| t.pack);
    out
}

/// The tab a character is on.
pub(crate) fn tab_of(pet: usize) -> usize {
    tabs().iter().position(|t| t.pets.contains(&pet)).unwrap_or(0)
}

/// What a pack is called.
pub(crate) fn name_of(pack: usize) -> String {
    ALL.read().unwrap().packs.get(pack).map_or(String::new(), |p| p.name.clone())
}

/// How the settings call a character.
pub(crate) fn key_of(pet: usize) -> String {
    ALL.read().unwrap().items.get(pet).map_or("0".into(), |t| t.key.clone())
}

/// The character the settings mean. Older versions wrote a number for all of them.
pub(crate) fn find(key: &str) -> Option<usize> {
    let all = ALL.read().unwrap();
    let there = |f: &dyn Fn(&Item) -> bool| all.items.iter().position(|t| t.there && f(t));
    match key.parse::<usize>() {
        Ok(n) if n < OURS.len() => Some(n),
        Ok(n) => there(&|t| t.was == Some(n)),
        Err(_) => there(&|t| t.key == key),
    }
}

/// The character still there, after packs came and went: itself, its successor, or the cat.
pub(crate) fn still(pet: usize) -> usize {
    let key = {
        let all = ALL.read().unwrap();
        match all.items.get(pet) {
            Some(t) if t.there => return pet,
            Some(t) => t.key.clone(),
            None => return 0,
        }
    };
    find(&key).unwrap_or(0)
}

// ---- the folder ----

/// Where imported packs are kept.
pub(crate) fn folder() -> Option<PathBuf> {
    data_path("pixel-pet-packs")
}

fn stamp(p: &Path) -> Option<(u64, std::time::SystemTime)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?))
}

fn read(p: &Path) -> Result<String, Why> {
    let len = std::fs::metadata(p).map_err(|_| "I cannot open that file.")?.len();
    if len > BIGGEST {
        return Err("That file is too big for a pack.".into());
    }
    String::from_utf8(std::fs::read(p).map_err(|_| "I cannot open that file.")?).map_err(|_| "This is not a pack for Pixel Pet.".into())
}

/// Look what is in the folder: take in what is new, let go of what has gone.
fn scan(all: &mut All) {
    let Some(dir) = folder() else { return };
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| Some(e.ok()?.path())).collect()).unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ENDING)) && p.is_file());
    files.sort();
    for i in 1..all.packs.len() {
        let p = &all.packs[i];
        if p.there && p.file.as_ref().is_some_and(|f| !files.contains(f)) {
            all.drop_pack(i);
        }
    }
    for f in files {
        let known = all.packs.iter().any(|p| p.there && p.file.as_ref() == Some(&f) && p.seen == stamp(&f));
        let full = all.packs.iter().skip(1).filter(|p| p.there && p.file.as_ref() != Some(&f)).count() >= MOST;
        if known || full {
            continue;
        }
        if let Ok(p) = read(&f).and_then(|t| parse(&t)) {
            all.add(p, Some(f));
        }
    }
}

/// The folder may have changed.
pub(crate) fn refresh() {
    if !cfg!(test) {
        scan(&mut ALL.write().unwrap());
    }
}

/// The imported packs: number, name, how many characters.
pub(crate) fn list() -> Vec<(usize, String, usize)> {
    let all = ALL.read().unwrap();
    let n = |i: usize| all.items.iter().filter(|t| t.pack == i).count();
    all.packs.iter().enumerate().skip(1).filter(|(_, p)| p.there).map(|(i, p)| (i, p.name.clone(), n(i))).collect()
}

/// Is a pack with this id in use?
pub(crate) fn has(id: &str) -> bool {
    ALL.read().unwrap().packs.iter().skip(1).any(|p| p.there && p.id == id)
}

/// Is this a file that can be imported?
pub(crate) fn is_pack(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case(ENDING))
}

/// What was taken in: its name, how many characters, the number of the first.
#[derive(Debug)]
pub(crate) struct Added {
    pub name: String,
    pub count: usize,
    pub first: usize,
}

/// Check a file and keep a copy of it.
pub(crate) fn import(from: &Path) -> Result<Added, Why> {
    import_to(from, &folder().ok_or("I do not know where to keep it.")?)
}

/// The same for one that has been downloaded.
pub(crate) fn import_bytes(bytes: Vec<u8>) -> Result<Added, Why> {
    let text = String::from_utf8(bytes).map_err(|_| "This is not a pack for Pixel Pet.")?;
    take_in(&text, &folder().ok_or("I do not know where to keep it.")?)
}

fn import_to(from: &Path, dir: &Path) -> Result<Added, Why> {
    take_in(&read(from)?, dir)
}

fn take_in(text: &str, dir: &Path) -> Result<Added, Why> {
    let p = parse(text)?;
    let mut all = ALL.write().unwrap();
    if p.id == all.packs[0].id || BUILT_IN.contains(&format!("\nid = {}\n", p.id)) {
        return Err("That pack comes with me already.".into());
    }
    if all.packs.iter().skip(1).filter(|o| o.there && o.id != p.id).count() >= MOST {
        return Err(format!("I keep {MOST} packs at most. Remove one first."));
    }
    let to = dir.join(format!("{}.{ENDING}", p.id));
    std::fs::create_dir_all(dir).map_err(|_| "I cannot write to my folder.")?;
    // all of it or nothing
    let part = dir.join(format!("{}.part", p.id));
    if std::fs::write(&part, text).is_err() || std::fs::rename(&part, &to).is_err() {
        let _ = std::fs::remove_file(&part);
        return Err("I cannot write to my folder.".into());
    }
    let (name, count) = (p.name.clone(), p.len());
    let first = all.add(p, Some(to));
    Ok(Added { name, count, first })
}

/// Forget a pack. The file it was imported from stays where it is.
pub(crate) fn remove(pack: usize) -> bool {
    let mut all = ALL.write().unwrap();
    let Some(file) = all.packs.get(pack).filter(|p| p.there).and_then(|p| p.file.clone()) else { return false };
    if std::fs::remove_file(&file).is_err() && file.exists() {
        return false;
    }
    all.drop_pack(pack);
    true
}

// ---- choosing a file ----

/// What came of asking for a file.
pub(crate) enum Answer {
    File(PathBuf),
    Nothing,
    #[cfg_attr(windows, allow(dead_code))] // Windows always can
    CannotAsk,
}

static ANSWER: Mutex<Option<(Want, Answer)>> = Mutex::new(None);

pub(crate) fn answer() -> Option<(Want, Answer)> {
    ANSWER.lock().unwrap().take()
}

/// What a file is wanted for; comes back with the answer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Want {
    Pack,
    Calendar,
}

impl Want {
    fn title(self) -> &'static str {
        match self {
            Want::Pack => "Import a character pack",
            Want::Calendar => "Add a calendar file",
        }
    }

    fn kind(self) -> (&'static str, &'static str) {
        match self {
            Want::Pack => ("Pixel Pet packs", "*.petpack"),
            Want::Calendar => ("Calendars", "*.ics"),
        }
    }
}

/// Ask for a file, on a thread of its own so that the pet goes on living. The answer comes as `WM_PACK`.
pub(crate) fn choose(hwnd: HWND, want: Want) {
    static ASKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ASKING.swap(true, Relaxed) {
        return; // it is open already
    }
    let to = hwnd as isize;
    std::thread::spawn(move || {
        let a = ask(want);
        *ANSWER.lock().unwrap() = Some((want, a));
        ASKING.store(false, Relaxed);
        unsafe { PostMessageW(to as HWND, WM_PACK, 0, 0) };
    });
}

#[cfg(windows)]
fn ask(want: Want) -> Answer {
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let (name, ending) = want.kind();
    let filter = wide(&format!("{name} ({ending})\0{ending}\0All files\0*.*\0"));
    let title = wide(want.title());
    let mut name = [0u16; 1024];
    let ok = unsafe {
        let com = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) >= 0;
        let mut o: OPENFILENAMEW = zeroed();
        o.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        o.lpstrFilter = filter.as_ptr();
        o.lpstrTitle = title.as_ptr();
        o.lpstrFile = name.as_mut_ptr();
        o.nMaxFile = name.len() as u32;
        o.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_HIDEREADONLY | OFN_EXPLORER;
        let ok = GetOpenFileNameW(&mut o) != 0;
        if com {
            CoUninitialize();
        }
        ok
    };
    let len = name.iter().position(|c| *c == 0).unwrap_or(0);
    if ok && len > 0 { Answer::File(PathBuf::from(String::from_utf16_lossy(&name[..len]))) } else { Answer::Nothing }
}

#[cfg(not(windows))]
fn ask(want: Want) -> Answer {
    let (title, ending) = (want.title(), want.kind().1);
    let script = format!("POSIX path of (choose file with prompt \"{title}\")");
    let (named, only) = (format!("--title={title}"), format!("--file-filter={ending}"));
    let mac: [(&str, &[&str]); 1] = [("osascript", &["-e", &script])];
    let linux: [(&str, &[&str]); 2] = [("zenity", &["--file-selection", &named, &only]), ("kdialog", &["--getopenfilename", ".", ending])];
    let ways: &[(&str, &[&str])] = if cfg!(target_os = "macos") { &mac } else { &linux };
    for (program, args) in ways {
        if let Ok(o) = std::process::Command::new(program).args(*args).output() {
            let path = String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string();
            return if o.status.success() && !path.is_empty() { Answer::File(PathBuf::from(path)) } else { Answer::Nothing };
        }
    }
    Answer::CannotAsk
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = "petpack 1
id = test
name = A test
colour 1 = #123456

[tool stick]
grip = 0 1
art =
| K
| 1

[move wiggle]
secs = 1
speed = 10 0.2 0.8
puff = spark 0.2
hit = front 0.5
dx = 0:0 0.5:2 1:0
arm2 = 0:10 0.5:90 1:10
eyes = 0:open 0.5:angry
gone = 0.2:yes 0.8:no
fx = 0:none 0.2:ray,0.2 0.6:ray,1 0.8:none

[pet bob]
name = Bob
tab = People
face = S
eye = K
head =
| .KKKK.
| KSSSSK
| KSEESK
| K1111K
| .KKKK.
legs = 1 2 1 K
arms = 3 1 S
back = cape 1
tool = stick
glow = 1
gift = leap
mouth = 0 3
voice = gentle
says = Hello.
says hungry = Food?
foods = apple milk cake
tricks = wiggle backflip
";

    fn why(change: impl Fn(String) -> String) -> String {
        parse(&change(SMALL.to_string())).err().unwrap_or_else(|| panic!("it was taken"))
    }

    #[test]
    fn a_small_pack_is_read() {
        let p = parse(SMALL).unwrap();
        assert_eq!((p.id.as_str(), p.len(), p.moves.len(), p.tools.len()), ("test", 1, 1, 1));
        let b = &p.pets[0];
        assert_eq!((b.name.as_str(), b.tab.as_str(), b.gift, b.foods), ("Bob", "PEOPLE", Gift::Leap, [APPLE, MILK, CAKE]));
        assert!(b.legs.c >= 128 && colour(b.legs.c) == Some(rgb(0x12, 0x34, 0x56)));
        assert_eq!(pet_color(b.legs.c), colour(b.legs.c));
        assert_eq!(b.says.len(), 2);
        // Windows line ends, a mark at the start and notes in between
        let p = parse(&format!("\u{feff}{}", SMALL.replace('\n', "\r\n").replace("[pet bob]", "# a note\r\n[pet bob]"))).unwrap();
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn a_move_goes_through_its_points() {
        let p = parse(SMALL).unwrap();
        let m = &p.moves[0];
        let look = |t: f32| {
            let mut r = Rig::REST;
            m.pose(&mut r, t);
            r
        };
        assert_eq!((look(0.0).dx, look(0.25).dx, look(0.5).dx, look(1.0).dx), (0.0, 1.0, 2.0, 0.0));
        assert_eq!((look(0.5).arms[1], look(0.1).eyes, look(0.6).eyes), (90.0, Eyes::Open, Eyes::Angry));
        assert!(look(0.5).gone && !look(0.9).gone);
        assert_eq!(look(0.1).fx, Fx::None);
        assert!(matches!(look(0.4).fx, Fx::Ray(k) if (k - 0.6).abs() < 1e-4));
        assert_eq!(look(0.7).fx, Fx::Ray(1.0));
        assert_eq!((m.travel(0.1), m.travel(0.5), m.travel(0.9)), (0.0, 10.0, 0.0));
    }

    #[test]
    fn mistakes_are_found_and_said() {
        assert!(why(|s| s.replace("petpack 1", "petpack 9")).contains("newer"));
        assert!(why(|_| "hello".into()).contains("not a pack"));
        for (a, b, says) in [
            ("id = test", "id = ../evil", "id"),
            ("id = test", "id = a b", "id"),
            ("KSEESK", "KSEES", "row"),
            ("KSEESK", "KS??SK", "colour"),
            ("foods = apple milk cake", "foods = apple milk rocks", "rocks"),
            ("tricks = wiggle", "tricks = wriggle", "wriggle"),
            ("tool = stick", "tool = stone", "stone"),
            ("name = Bob", "name = Bob the very long one", "name"),
            ("secs = 1", "secs = 900", "secs"),
            ("secs = 1", "secs = nan", "secs"),
            ("dx = 0:0 0.5:2", "dx = 0.5:0 0.2:2", "dx"),
            ("dx = 0:0 0.5:2", "dx = 0:0 0.5:900", "dx"),
            ("fx = 0:none", "fx = 0:fireworks", "fireworks"),
            ("0.2:ray,0.2", "0.2:ray,0.2,0.4", "ray"),
            ("gift = leap", "gift = leap\ngift = fly", "twice"),
            ("gift = leap", "magic = yes", "magic"),
            ("colour 1 = #123456", "colour K = #123456", "colour"),
            ("colour 1 = #123456", "colour 1 = red", "ff8800"),
            ("legs = 1 2 1 K", "legs = 1 19 1 K", "legs"),
            ("says hungry", "says loudly", "loudly"),
            ("[pet bob]", "[script bob]", "script"),
            ("mouth = 0 3", "mouth = 0 30", "mouth"),
            ("grip = 0 1", "grip = 5 1", "grip"),
        ] {
            assert!(SMALL.contains(a), "{a}");
            let w = why(|s| s.replacen(a, b, 1));
            assert!(w.contains(says), "{b}: {w}");
        }
        assert!(why(|s| format!("{s}{}", &s[s.find("[pet bob]").unwrap()..])).contains("two"));
    }

    #[test]
    fn no_file_however_broken_brings_it_down() {
        // cut short, and with things swapped, left out and changed
        let mut seed = 7u32;
        let mut rnd = |n: usize| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as usize % n.max(1)
        };
        for text in [SMALL, BUILT_IN] {
            let odd: &[u8] = b"=:|[]#.,-9eKE \n\r\0\xff";
            for _ in 0..if text.len() > 5000 { 150 } else { 3000 } {
                let mut b = text.as_bytes().to_vec();
                for _ in 0..1 + rnd(6) {
                    let i = rnd(b.len());
                    match rnd(4) {
                        0 => b.truncate(i),
                        1 => b[i.min(text.len() - 1)] = odd[rnd(odd.len())],
                        2 => drop(b.drain(i..(i + rnd(40)).min(text.len()).min(b.len()))),
                        _ => b.insert(i, odd[rnd(odd.len())]),
                    }
                    if b.is_empty() {
                        b.push(b'p');
                    }
                }
                let t = String::from_utf8_lossy(&b).into_owned();
                if let Ok(p) = parse(&t) {
                    for m in &p.moves {
                        for i in 0..20 {
                            let mut r = Rig::REST;
                            m.pose(&mut r, m.secs * i as f32 / 19.0);
                            assert!(r.sx.is_finite() && r.sy > 0.4 && r.dy.abs() < 14.0 && (0.0..=1.0).contains(&r.alpha));
                        }
                    }
                }
            }
        }
        assert!(parse("").is_err() && parse("petpack 1").is_err() && parse("petpack 1\n| KK").is_err());
    }

    /// Packs that come and go are tried with one of their own, so that other tests see no change.
    fn own_all() -> All {
        let mut all = All { items: Vec::new(), packs: vec![Info { id: "pixel-pet".into(), name: "Pixel Pet".into(), file: None, seen: None, there: true }] };
        all.items.push(Item { pet: &OURS[0], key: "0".into(), tab: "CREATURES", pack: 0, was: None, there: true });
        all
    }

    #[test]
    fn a_newer_pack_takes_the_place_of_the_older_one() {
        let mut all = own_all();
        let a = all.add(parse(SMALL).unwrap(), Some("a/test.petpack".into()));
        let b = all.add(parse(&SMALL.replace("name = Bob", "name = Rob")).unwrap(), Some("a/test.petpack".into()));
        assert!(!all.items[a].there && all.items[b].there && all.items[a].key == all.items[b].key);
        assert_eq!((all.items[b].pet.name, all.items[a].pet.name), ("Rob", "Bob"));
        all.drop_pack(all.items[b].pack);
        assert!(!all.items[b].there && all.items[0].there);
        // its own moves are the pack's, not somebody else's
        let Clip::Own(n) = all.items[b].pet.tricks[0] else { panic!("not its own move") };
        assert_eq!((own(n).secs, own(n).hit), (1.0, Some((Hit::Front, 0.5))));
        assert_eq!(all.items[b].pet.tricks[1], Clip::Backflip);
    }

    #[test]
    fn settings_keep_meaning_the_same_character() {
        assert_eq!((find("0"), find("15"), find("99"), find("nobody/here")), (Some(0), Some(15), None, None));
        for i in 0..PETS.len() {
            assert_eq!(find(&key_of(i)), Some(i));
            assert_eq!(still(i), i);
        }
        assert_eq!(still(9999), 0);
        assert_eq!(key_of(3), "3", "older versions understand these");
    }

    #[test]
    fn importing_needs_a_good_file() {
        let dir = std::env::temp_dir().join(format!("pixel-pet-packs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.petpack");
        std::fs::write(&bad, "not a pack").unwrap();
        assert!(import_to(&bad, &dir.join("in")).unwrap_err().contains("not a pack"));
        assert!(import_to(&dir.join("missing.petpack"), &dir.join("in")).is_err());
        std::fs::write(&bad, BUILT_IN).unwrap();
        assert!(import_to(&bad, &dir.join("in")).unwrap_err().contains("already"));
        assert!(!dir.join("in").exists(), "nothing was written");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(is_pack("C:\\x\\a.PetPack") && !is_pack("a.txt") && !is_pack("petpack"));
    }

    #[test]
    fn the_example_and_the_list_of_moves_are_right() {
        let p = parse(include_str!("../docs/example.petpack")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((p.len(), p.moves.len()), (1, 1));
        // everything the guide should tell about
        let guide = include_str!("../docs/PACKS.md");
        let said = |w: &str| guide.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).any(|g| g == w);
        for (name, _) in ACTS {
            assert!(said(name), "PACKS.md does not tell about the move {name}");
        }
        for name in FX.iter().map(|f| f.0).chain(PROPS.iter().map(|p| p.0)).chain(GIFTS.iter().map(|g| g.0)).chain(ALOFT.iter().map(|a| a.0)).chain(PUFFS.iter().map(|p| p.0)).chain(EVENTS.iter().map(|e| e.0)) {
            assert!(said(name), "PACKS.md does not tell about {name}");
        }
        for f in FOODS.iter() {
            assert!(said(&f.name.to_lowercase().replace(' ', "-")), "PACKS.md does not tell about {}", f.name);
        }
    }

    #[test]
    fn what_comes_with_the_pet_is_in_order() {
        let p = parse(BUILT_IN).unwrap_or_else(|e| panic!("{e}"));
        assert!(p.len() >= 20, "{} characters", p.len());
        for t in tabs() {
            assert!(!t.pets.is_empty() && t.pets.len() <= PER_TAB && t.name.len() <= 9, "{}", t.name);
        }
        assert_eq!(tabs()[tab_of(3)].name, "PEOPLE");
        assert!(tabs().iter().filter(|t| t.pack == 0).count() <= MOST_TABS);
    }
}
