//! The pet's body on the desktop: walking, being pulled, thrown and hit, eating, random acts.
//! `step` runs 30 times a second and only repaints when something visibly changed.

use crate::prelude::*;

pub(crate) const FRAME_MS: u32 = 33;
pub(crate) const SLOW_MS: u32 = 110;
/// Room around the pet inside its window, so it can wave, stretch and tumble without being cut off.
pub(crate) const PAD_X: i32 = 40;
pub(crate) const PAD_TOP: i32 = 28;
pub(crate) const PAD_BOTTOM: i32 = 10;
/// How close to the side of the screen the pet stands.
pub(crate) const EDGE: i32 = 8;

const GRAVITY: f32 = 1900.0; // px per second squared at 100%
const MAX_THROW: f32 = 2400.0;
const DIZZY_IMPACT: f32 = 1150.0;
const HITS_TO_DIZZY: u32 = 4;
const HITS_TO_FLY: u32 = 7;

/// Seconds without being touched before the pet dozes off (0 = never).
pub(crate) const NAP_AFTER: [u32; 4] = [60, 180, 600, 0];
pub(crate) const NAP_NAMES: [&str; 4] = ["1 MIN", "3 MIN", "10 MIN", "NEVER"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    Rest,
    Walk,
    Pull, // grabbed, feet still on the ground
    Held, // picked up
    Air,  // thrown or falling
    Route, // on the way to another monitor
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Bit {
    Heart,
    Spark,
    Zed,
    Dust,
    Note,
    Drop,
    Burst,
    Smoke,
    Alarm,
    Crumb,
    Cell, // an almost empty battery
}

#[derive(Clone, Copy)]
pub(crate) struct Part {
    pub x: f32, // screen position
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub g: f32,
    pub age: f32,
    pub life: f32,
    pub bit: Bit,
    pub c: u32,
}

/// Food on its way to the pet.
#[derive(Clone, Copy)]
pub(crate) struct Meal {
    pub food: usize,
    pub x: f32,
    pub y: f32,
    pub floor: f32,
    pub vy: f32,
    pub age: f32,
    pub landed: bool,
}

pub(crate) struct Body {
    pub mode: Mode,
    pub clip: Option<Clip>, // an act with an end; None = whatever the mode and mood say
    pub shown: Clip,        // what was shown last, to notice changes
    pub t: f32,             // seconds into it
    pub facing: i32,
    pub feet: (f32, f32), // screen position
    pub vel: (f32, f32),
    pub pull: (f32, f32), // in body heights, screen directions (x right, y up)
    pub swing: f32,
    pub swing_v: f32,
    pub spin: f32,
    pub hit: f32, // which way the last blow pushes, 1 = right
    pub hits: u32,
    pub hit_cd: f32,
    pub food: usize,
    pub meal: Option<Meal>,
    pub parts: Vec<Part>,
    pub walk_left: f32, // px still to walk
    pub run: bool,
    pub boost: f32,   // seconds of extra energy
    pub ignored: f32, // seconds since anyone touched or hovered it
    pub next: f32,    // seconds until it does something by itself
    pub grab: (f32, f32), // feet minus cursor when it was grabbed
    pub on_pet: bool,     // the mouse button went down on the pet
    pub cursor: (f32, f32),
    pub last_ms: u32,
    pub emit: f32,
    pub seed: u32,
    pub key: u64,
    pub since: f32, // seconds since the last repaint
    pub area: RECT, // what the last repaint covered
    pub screens: Vec<Screen>,
    pub screens_ms: u32,
    pub mon: isize, // the monitor its size was fitted to
    pub route: Vec<Leg>, // still to come, next first
    pub trail: Vec<((f32, f32), (f32, f32), Way)>, // the stairs and ladders of this journey
    pub through: Option<f32>, // falling past floors down to here
    pub glide: bool, // under an umbrella
    pub soft: bool,  // jumped by itself, so landing does not hurt
    pub beat: f32,
    pub beats: u32,
    pub keys: buddy::Keys,
    pub finished: u32, // what it last knew of buddy::FINISHED and ASKED
    pub asked: u32,
    pub carrying: bool, // has the ball
    pub meet: bool, // walking over to the other pet
    pub other: Option<(f32, f32)>, // where the other pet stands
    pub bumped: f32, // seconds until it can bump into it again
    pub perches: Vec<Perch>,
    pub perch_ms: u32,
    pub perch: Option<(isize, RECT)>, // the window it stands on, and where that was
}

impl Body {
    pub const fn new() -> Body {
        Body {
            mode: Mode::Rest,
            clip: None,
            shown: Clip::Stand,
            t: 0.0,
            facing: 1,
            feet: (0.0, 0.0),
            vel: (0.0, 0.0),
            pull: (0.0, 0.0),
            swing: 0.0,
            swing_v: 0.0,
            spin: 0.0,
            hit: 1.0,
            hits: 0,
            hit_cd: 0.0,
            food: 0,
            meal: None,
            parts: Vec::new(),
            walk_left: 0.0,
            run: false,
            boost: 0.0,
            ignored: 0.0,
            next: 4.0,
            grab: (0.0, 0.0),
            on_pet: false,
            cursor: (0.0, 0.0),
            last_ms: 0,
            emit: 0.0,
            seed: 0x9e37_79b9,
            key: 0,
            since: 0.0,
            area: NO_RECT,
            screens: Vec::new(),
            screens_ms: 0,
            mon: 0,
            route: Vec::new(),
            trail: Vec::new(),
            through: None,
            glide: false,
            soft: false,
            beat: 0.0,
            beats: 0,
            keys: buddy::Keys::new(),
            finished: 0,
            asked: 0,
            carrying: false,
            meet: false,
            other: None,
            bumped: 0.0,
            perches: Vec::new(),
            perch_ms: 0,
            perch: None,
        }
    }

    /// What is it doing right now?
    pub fn clip(&self, mood: Pose) -> Clip {
        match (self.mode, self.clip) {
            (Mode::Pull, _) => Clip::Pull,
            (Mode::Held, _) => Clip::Held,
            (Mode::Air, _) => if self.glide { Clip::Glide } else { Clip::Fall },
            (Mode::Route, _) => match self.route.first().map(|l| l.way) {
                Some(Way::Ladder) => Clip::Climb,
                Some(Way::Float) => Clip::Run,
                _ => if self.run { Clip::Run } else { Clip::Walk },
            },
            (_, Some(c)) => c,
            (Mode::Walk, _) => if self.run { Clip::Run } else { Clip::Walk },
            (Mode::Rest, _) => match mood {
                Pose::Idle | Pose::Hungry => Clip::Stand,
                Pose::Sleep => Clip::Sleep,
                Pose::Cheer => Clip::Cheer,
                Pose::Love => Clip::Love,
                Pose::Stretch => Clip::Stretch,
                Pose::Tired => Clip::Tired,
                Pose::Sad => Clip::Sad,
            },
        }
    }

    /// How steep the ground is.
    fn tilt(&self) -> f32 {
        match self.route.first() {
            Some(l) if self.mode == Mode::Route && l.way == Way::Stairs => if l.to.1 > self.feet.1 { 40.0 } else { -40.0 },
            _ => 0.0,
        }
    }

    pub fn push(&self) -> Push {
        let f = self.facing as f32;
        Push { pull: (self.pull.0 * f, self.pull.1), swing: self.swing * f, spin: self.spin * f, hit: self.hit * f, food: self.food, beat: self.beat, tilt: self.tilt() }
    }

    fn start(&mut self, c: Clip) {
        self.clip = Some(c);
        self.t = 0.0;
        self.emit = 0.0;
    }

    /// Drop whatever it was up to by itself.
    pub fn stop(&mut self) {
        if matches!(self.mode, Mode::Walk | Mode::Route) {
            self.mode = Mode::Rest;
        }
        self.route.clear();
        self.trail.clear();
        self.run = false;
        self.meet = false;
        self.walk_left = 0.0;
        if self.mode == Mode::Rest && self.clip != Some(Clip::Eat) {
            self.clip = None;
        }
    }

    fn rnd(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn puff(&mut self, bit: Bit, x: f32, y: f32, vx: f32, vy: f32, g: f32, life: f32, c: u32) {
        if self.parts.len() < 60 {
            self.parts.push(Part { x, y, vx, vy, g, age: 0.0, life, bit, c });
        }
    }
}

// ---- rules (pure, so they can be tested) ----

/// Which side of the screen's middle the pet counts as being on, with some give so that it does
/// not flicker there. `low` = left or top half.
pub(crate) fn side(low: bool, at: i32, mid: i32, give: i32) -> bool {
    if low {
        at <= mid + give
    } else {
        at < mid - give
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Flight {
    pub x: f32,
    pub y: f32, // feet
    pub vx: f32,
    pub vy: f32,
}

/// Advance a thrown pet by `dt`. Returns the landing speed once it comes to rest on the floor.
pub(crate) fn fly(f: &mut Flight, dt: f32, g: f32, area: &RECT, half: f32, tall: f32) -> Option<f32> {
    f.vy += g * dt;
    f.x += f.vx * dt;
    f.y += f.vy * dt;
    let (l, r) = (area.left as f32 + half, (area.right as f32 - half).max(area.left as f32 + half));
    if f.x < l {
        f.x = l;
        f.vx = f.vx.abs() * 0.6;
    } else if f.x > r {
        f.x = r;
        f.vx = -f.vx.abs() * 0.6;
    }
    let ceiling = area.top as f32 + tall;
    if f.y < ceiling && f.vy < 0.0 {
        f.y = ceiling;
        f.vy = -f.vy * 0.4;
    }
    if f.y >= area.bottom as f32 {
        f.y = area.bottom as f32;
        let speed = f.vy;
        if speed > 520.0 {
            f.vy = -speed * 0.36;
            f.vx *= 0.65;
            None
        } else {
            f.vx = 0.0;
            f.vy = 0.0;
            Some(speed)
        }
    } else {
        None
    }
}

/// Like `fly`, for something round: it rolls on until it loses its way. True once it lies still.
pub(crate) fn bounce(f: &mut Flight, dt: f32, g: f32, area: &RECT, half: f32, tall: f32) -> bool {
    let on_floor = (f.y - area.bottom as f32).abs() < 1.0 && f.vy.abs() < 1.0;
    if on_floor {
        f.vx *= (1.0 - 1.1 * dt).max(0.0);
        f.x += f.vx * dt;
        let (l, r) = (area.left as f32 + half, (area.right as f32 - half).max(area.left as f32 + half));
        if f.x < l || f.x > r {
            f.x = f.x.clamp(l, r);
            f.vx = -f.vx * 0.6;
        }
        return f.vx.abs() < 12.0;
    }
    let vx = f.vx;
    if fly(f, dt, g, area, half, tall).is_some() {
        f.vx = vx; // keeps rolling
        f.vy = 0.0;
    } else if f.vy < 0.0 && f.y >= area.bottom as f32 {
        f.vy *= 1.75; // bouncier than a pet
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Blow {
    Flinch,
    Dizzy,
    Fly,
}

/// What the `n`th blow in a row does.
pub(crate) fn blow(n: u32) -> Blow {
    match n {
        n if n >= HITS_TO_FLY => Blow::Fly,
        HITS_TO_DIZZY => Blow::Dizzy,
        _ => Blow::Flinch,
    }
}

/// How fast it gets around, in cells per second.
pub(crate) fn pace(machine: bool, run: bool) -> f32 {
    let v = if run { 30.0 } else { 11.0 };
    if machine { v * 1.5 } else { v }
}

/// Heavy load counts up, anything else down; `hot` once it has lasted ten seconds.
pub(crate) fn heat(now: u32, cpu: u32) -> u32 {
    if cpu >= 80 {
        (now + 1).min(40)
    } else {
        now.saturating_sub(2)
    }
}

const HOT: u32 = 20;

pub(crate) fn just_plugged(before: Option<(u32, bool)>, now: Option<(u32, bool)>) -> bool {
    matches!((before, now), (Some((_, false)), Some((_, true))))
}

/// The battery is nearly empty and nothing is charging it.
pub(crate) fn running_low(b: Option<(u32, bool)>) -> bool {
    b.is_some_and(|(p, plugged)| p <= 20 && !plugged)
}

/// Moved further than this in one step, a window shakes off whoever stands on it.
pub(crate) fn shaken(dx: i32, dy: i32, scale: f32) -> bool {
    (dx.abs() + dy.abs()) as f32 > 90.0 * scale
}

// ---- geometry ----

#[derive(Clone, Copy)]
pub(crate) struct Geo {
    pub w: i32,
    pub h: i32,
    pub px: i32, // one cell
    pub cw: i32, // pet size
    pub ch: i32,
    pub pad_x: i32,
    pub pad_top: i32,
    pub pad_bottom: i32,
    pub edge: i32,
    pub flip: (bool, bool), // pet on the left / at the top of its window
}

impl Geo {
    /// Top left of the standing pet inside the window.
    pub fn origin(&self) -> (i32, i32) {
        (
            if self.flip.0 { self.pad_x } else { self.w - self.cw - self.pad_x },
            if self.flip.1 { self.pad_top } else { self.h - self.ch - self.pad_bottom },
        )
    }

    /// The middle of the floor under it, inside the window.
    pub fn feet(&self) -> (i32, i32) {
        let (x, y) = self.origin();
        (x + self.cw / 2, y + self.ch)
    }

    /// The screen area its window may cover; the see-through margins may hang over the edge.
    pub fn bounds(&self, wa: &RECT) -> RECT {
        let side = self.pad_x - self.edge;
        RECT {
            left: wa.left - if self.flip.0 { side } else { 0 },
            right: wa.right + if self.flip.0 { 0 } else { side },
            top: wa.top - if self.flip.1 { self.pad_top - self.edge } else { 0 },
            bottom: wa.bottom + if self.flip.1 { 0 } else { self.pad_bottom },
        }
    }
}

pub(crate) fn geo(s: &State) -> Geo {
    let scale = eff_scale(s);
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let (w, h) = win_dims_s(s);
    let px = u(if s.minimal { presence::MINI_PX } else { PX }).max(1);
    let pet = &PETS[s.pet];
    Geo {
        w,
        h,
        px,
        cw: pet.w() * px,
        ch: pet.h() * px,
        pad_x: u(PAD_X),
        pad_top: u(PAD_TOP),
        pad_bottom: u(PAD_BOTTOM),
        edge: u(EDGE),
        flip: s.flip,
    }
}

// ---- what people do to it ----

/// Earn experience; a new level is celebrated.
pub(crate) fn gain(s: &mut State, n: u32) {
    let before = s.xp;
    s.xp = s.xp.saturating_add(n);
    if wardrobe::level(s.xp) == wardrobe::level(before) {
        return;
    }
    s.mood.on_level();
    sound::play(Sound::Level);
    let b = &mut s.body;
    let (x, y) = b.feet;
    for i in 0..12 {
        let a = i as f32 * 0.524;
        let c = [PINK, UP_BLUE, GREEN, rgb(0xff, 0xe0, 0x66)][i % 4];
        b.puff(Bit::Spark, x, y - 60.0, a.cos() * 140.0, a.sin() * 120.0 - 60.0, 160.0, 1.1, c);
    }
    if s.talk_on && presence::chatty(s.minimal, s.hidden) {
        let text = match wardrobe::unlocked(before, s.xp) {
            Some(h) => format!("Level {}! I can wear the {} now.", wardrobe::level(s.xp), h.to_lowercase()),
            None => format!("Level {}!", wardrobe::level(s.xp)),
        };
        say_text(s, &text, 16);
    }
    save_cfg(s);
}

/// The hat it has on.
pub(crate) fn hat(s: &State) -> usize {
    wardrobe::worn(s.hat, s.xp)
}

fn touched(s: &mut State) {
    s.body.ignored = 0.0;
    s.mood.wake();
}

/// Hit it. `from` is where the blow came from: -1 left, 1 right.
pub(crate) fn hit(s: &mut State, from: f32, at: (f32, f32)) {
    touched(s);
    let machine = PETS[s.pet].machine();
    let scale = eff_scale(s);
    let gravity = s.gravity;
    let ev = s.mood.on_hit();
    let b = &mut s.body;
    b.stop();
    sound::play(Sound::Hit);
    b.hits += 1;
    b.hit_cd = 3.0;
    b.hit = -from;
    b.puff(Bit::Burst, at.0, at.1, 0.0, 0.0, 0.0, 0.22, rgb(0xff, 0xe0, 0x66));
    let c = if machine { rgb(0xff, 0xd2, 0x7a) } else { rgb(0xff, 0xf3, 0xb0) };
    for i in 0..5 {
        let a = i as f32 * 1.256 + b.rnd();
        b.puff(Bit::Spark, at.0, at.1, a.cos() * 130.0 * scale, a.sin() * 130.0 * scale - 40.0, 300.0, 0.5, c);
    }
    match blow(b.hits) {
        Blow::Fly if gravity => {
            b.hits = HITS_TO_DIZZY; // it will see stars when it lands
            b.hit_cd = 6.0;
            b.mode = Mode::Air;
            b.clip = None;
            b.vel = (-from * 620.0 * scale, -780.0 * scale);
            b.spin = 0.0;
        }
        Blow::Fly | Blow::Dizzy => {
            if b.hits >= HITS_TO_FLY {
                b.hits = 0;
            }
            b.start(Clip::Flinch);
            b.next = -1.0; // dizzy as soon as the flinch is over
        }
        Blow::Flinch => b.start(Clip::Flinch),
    }
    if let Some(ev) = ev {
        say(s, ev);
    }
}

/// Offer it food. It drops in beside the pet, which walks over and eats it.
pub(crate) fn feed(s: &mut State, food: usize, at_feet: (f32, f32), floor: f32) {
    touched(s);
    let g = geo(s);
    let scale = eff_scale(s);
    // on the open side of the window, unless the pet has to stay where it is
    let grounded = (at_feet.1 - floor).abs() < 3.0;
    let away = if s.focus || !grounded { 0.0 } else if g.flip.0 { 105.0 } else { -105.0 };
    s.body.stop();
    s.body.meal = Some(Meal { food: food.min(FOODS.len() - 1), x: at_feet.0 + away * scale, y: at_feet.1 - g.ch as f32 - 30.0 * scale, floor: at_feet.1, vy: 0.0, age: 0.0, landed: false });
}

/// The other pet wants to play. `x` is where it stands.
pub(crate) fn invited(s: &mut State, p: Play, x: f32) {
    let towards = if x > s.body.feet.0 { 1 } else { -1 };
    if p == Play::Bump {
        if matches!(s.body.mode, Mode::Rest | Mode::Walk) {
            let up = geo(s).ch as f32;
            s.body.stop();
            s.body.hit = -towards as f32;
            let (bx, by) = s.body.feet;
            s.body.puff(Bit::Burst, bx + towards as f32 * 20.0, by - up * 0.5, 0.0, 0.0, 0.0, 0.22, rgb(0xff, 0xe0, 0x66));
            s.body.start(Clip::Flinch);
            sound::play(Sound::Hit);
        }
        return;
    }
    if !free(s) || !matches!(s.body.mode, Mode::Rest | Mode::Walk) || s.body.meal.is_some() || s.body.carrying {
        return;
    }
    s.body.ignored = 0.0;
    s.body.stop();
    s.body.clip = None;
    match friend::clip_of(p) {
        Some(c) => {
            s.body.facing = towards;
            s.body.start(c);
        }
        None if s.walk_on => {
            // being chased: off it goes
            s.body.facing = -towards;
            s.body.mode = Mode::Walk;
            s.body.run = true;
            s.body.walk_left = 620.0;
        }
        None => {}
    }
}

/// Show something straight away, unless it is in the middle of something that matters.
pub(crate) fn react(s: &mut State, c: Clip) {
    if free(s) && s.body.mode == Mode::Rest && s.body.meal.is_none() && s.mood.act() != Pose::Sleep {
        s.body.start(c);
    }
}

/// Do something to amuse people, right now.
pub(crate) fn perform(s: &mut State) {
    touched(s);
    let pet = &PETS[s.pet];
    let b = &mut s.body;
    b.stop();
    if b.mode == Mode::Rest {
        let i = (b.rnd() * pet.tricks.len() as f32) as usize;
        b.start(pet.tricks[i.min(pet.tricks.len() - 1)]);
    }
}

/// The mouse went down on the pet.
pub(crate) fn grab(s: &mut State, cursor: (i32, i32), feet: (f32, f32)) {
    touched(s);
    let b = &mut s.body;
    b.on_pet = true;
    b.feet = feet;
    b.grab = (feet.0 - cursor.0 as f32, feet.1 - cursor.1 as f32);
    b.cursor = (cursor.0 as f32, cursor.1 as f32);
    b.vel = (0.0, 0.0);
}

/// The mouse moved while holding the pet: stretch it, and past a point it comes off the ground.
pub(crate) fn drag(s: &mut State, d: (i32, i32)) {
    let tall = geo(s).ch as f32;
    let b = &mut s.body;
    match b.mode {
        Mode::Held => {}
        _ => {
            let p = (d.0 as f32 / tall, -d.1 as f32 / tall);
            b.clip = None;
            b.meal = None;
            if p.0.hypot(p.1) > 0.55 {
                b.mode = Mode::Held;
                b.swing = 0.0;
                b.swing_v = 0.0;
                b.pull = (0.0, 0.0);
            } else {
                b.mode = Mode::Pull;
                b.pull = p;
            }
        }
    }
}

/// The mouse let go of the pet.
pub(crate) fn release(s: &mut State) {
    touched(s);
    let scale = eff_scale(s);
    let gravity = s.gravity;
    let b = &mut s.body;
    b.on_pet = false;
    match b.mode {
        Mode::Pull => {
            b.mode = Mode::Rest;
            b.hit = if b.pull.0 < 0.0 { -1.0 } else { 1.0 };
            b.pull = (0.0, 0.0);
            sound::play(Sound::Boing);
            b.start(Clip::Boing);
        }
        Mode::Held => {
            b.spin = 0.0;
            if gravity {
                let max = MAX_THROW * scale.max(1.0);
                b.vel = (b.vel.0.clamp(-max, max), b.vel.1.clamp(-max, max));
                if b.vel.0.hypot(b.vel.1) > 500.0 {
                    sound::play(Sound::Whoosh);
                }
                b.mode = Mode::Air;
            } else {
                b.mode = Mode::Rest;
                b.start(Clip::Land);
            }
        }
        _ => {}
    }
}

/// Set out for another monitor on foot. False if there is no way there.
pub(crate) fn travel(s: &mut State) -> bool {
    let g = geo(s);
    let pet = &PETS[s.pet];
    let who = Traveller { half: g.cw as f32 / 2.0, tall: g.ch as f32, flies: pet.build == Build::Hover, climbs: pet.build == Build::Legs };
    if s.focus || !s.gravity {
        return false;
    }
    let b = &mut s.body;
    let here = b.feet;
    let Some(from) = world::standing_on(&b.screens, here.0, here.1) else { return false };
    if !matches!(b.mode, Mode::Rest | Mode::Walk) || (b.screens[from].work.bottom as f32 - here.1).abs() > 3.0 {
        return false;
    }
    let n = b.screens.len();
    let first = (b.rnd() * n as f32) as usize;
    for k in 0..n {
        let to = (first + k) % n;
        let taste = b.rnd();
        let Some(legs) = world::plan(&b.screens, from, here.0, to, &who, taste) else { continue };
        b.stop();
        b.clip = None;
        let mut at = here;
        for l in &legs {
            if matches!(l.way, Way::Stairs | Way::Ladder) {
                b.trail.push((at, l.to, l.way));
            }
            at = l.to;
        }
        b.route = legs;
        b.mode = Mode::Route;
        b.run = b.boost > 0.0;
        b.t = 0.0;
        return true;
    }
    false
}

/// Set out for the top of a window. False if there is none in reach.
fn climb(s: &mut State) -> bool {
    let g = geo(s);
    let pet = &PETS[s.pet];
    if pet.build == Build::Wheels || !s.gravity || s.focus {
        return false;
    }
    let half = g.cw as f32 / 2.0 + 8.0;
    let b = &mut s.body;
    let Some(i) = world::standing_on(&b.screens, b.feet.0, b.feet.1) else { return false };
    let floor = b.screens[i].work.bottom as f32;
    if (floor - b.feet.1).abs() > 3.0 || b.perches.is_empty() {
        return false;
    }
    let (l, r) = world::level_span(&b.screens, i);
    let first = (b.rnd() * b.perches.len() as f32) as usize;
    for k in 0..b.perches.len() {
        let p = b.perches[(first + k) % b.perches.len()];
        // somewhere on the ledge that can also be walked to
        let (a, z) = (p.l.max(l) as f32 + half, p.r.min(r) as f32 - half);
        if a > z {
            continue;
        }
        let x = b.feet.0.clamp(a, z);
        let top = p.y as f32;
        // the floor below the ledge must be the one it walks on
        let Some(y) = world::level_floor(&b.screens, x, floor).map(|y| y as f32) else { continue };
        if world::floor_below(&b.screens, x, top, None).map(|f| f as f32) != Some(y) {
            continue;
        }
        b.stop();
        b.clip = None;
        let way = if pet.build == Build::Hover { Way::Float } else { Way::Ladder };
        if way == Way::Ladder {
            b.trail.push(((x, y), (x, top), way));
        }
        b.route = vec![Leg { to: (x, y), way: Way::Walk }, Leg { to: (x, top), way }];
        b.mode = Mode::Route;
        b.run = false;
        b.t = 0.0;
        return true;
    }
    false
}

/// What to leap with to get from `a` to `b` (feet positions).
pub(crate) fn leap(a: (f32, f32), b: (f32, f32), g: f32, up: f32) -> (f32, f32) {
    let drop = (b.1 - a.1).max(0.0);
    let time = (up + (up * up + 2.0 * g * drop).sqrt()) / g;
    ((b.0 - a.0) / time, -up)
}

/// Run after the ball, and bring it back. True while there is a ball to see to.
fn fetch(s: &mut State, hovered: bool, half: f32, px: f32) -> bool {
    let mut ball = ball::BALL.lock().unwrap();
    let carrying = s.body.carrying;
    if carrying && !(ball.out && ball.carried) {
        s.body.carrying = false; // it was put away meanwhile
        return false;
    }
    let ready = !s.menu && !s.down && !s.focus && !s.quiet && s.alert.is_none() && s.body.meal.is_none();
    let busy = s.body.clip.is_some_and(|c| matches!(c, Clip::Flinch | Clip::Dizzy | Clip::Angry | Clip::Eat | Clip::Land | Clip::Boing));
    if !ball.out || !ready || busy {
        return false;
    }
    let b = &mut s.body;
    let goal = if carrying { ball.home } else { ball.x };
    if !carrying {
        // it has to be worth it, and on the same floor
        let Some(i) = world::standing_on(&b.screens, b.feet.0, b.feet.1) else { return false };
        let (l, r) = world::level_span(&b.screens, i);
        if b.perch.is_some() && ball.interest > 0.0 && !ball.held && ball.y > b.feet.1 + 60.0 {
            // the ball is down there: after it
            b.perch = None;
            b.mode = Mode::Air;
            b.vel = ((ball.x - b.feet.0).clamp(-420.0, 420.0), -320.0);
            b.spin = 0.0;
            b.soft = true;
            return true;
        }
        let level = ball.y > b.feet.1 - 400.0 && ball.y < b.feet.1 + world::STEP_OVER as f32;
        if ball.held || ball.interest <= 0.0 || !level || ball.x < l as f32 || ball.x > r as f32 || hovered {
            return false;
        }
    }
    let d = goal - b.feet.0;
    let near = half + px * if carrying { 3.0 } else { 1.5 };
    if d.abs() <= near {
        if carrying {
            b.carrying = false;
            b.mode = Mode::Rest;
            let (x, y, dir) = (b.feet.0, b.feet.1, b.facing as f32);
            drop(ball);
            ball::put_down(x + dir * half, y, dir);
            s.mood.on_cheer();
            gain(s, wardrobe::XP_BALL);
            return false;
        }
        if ball.y > b.feet.1 - half {
            ball.carried = true;
            b.carrying = true;
            b.mode = Mode::Rest;
            sound::play(Sound::Ball);
        }
        return true;
    }
    b.clip = None;
    b.mode = Mode::Walk;
    b.facing = if d > 0.0 { 1 } else { -1 };
    b.run = d.abs() > 70.0;
    b.walk_left = d.abs();
    true
}

// ---- the 30 Hz step ----

/// May it start something by itself? Never while anything else wants attention.
fn free(s: &State) -> bool {
    !s.menu && !s.focus && !s.down && !s.quiet && !s.hidden && s.alert.is_none() && s.msg_left == 0 && s.fact_left == 0 && s.intro == intro::DONE
}

/// Pick the next thing to do by itself.
fn choose(s: &mut State, grounded: bool) {
    let pet = &PETS[s.pet];
    let mood = s.mood.act();
    let (walks, tricks) = (s.walk_on && grounded, s.tricks_on);
    let b = &mut s.body;
    b.next = 2.5 + b.rnd() * 6.5;
    if !walks && !tricks {
        return;
    }
    let r = b.rnd();
    let far = b.rnd();
    if mood == Pose::Hungry && r < 0.5 && tricks {
        b.food = pet.foods[(b.rnd() * 3.0) as usize % 3];
        b.start(Clip::Hungry);
        return;
    }
    // how the PC is doing shows
    if s.react_on && s.hot >= HOT && r < 0.65 {
        b.start(Clip::Hot);
        return;
    }
    if s.react_on && running_low(s.battery) && r < 0.4 {
        b.start(Clip::Low);
        return;
    }
    if s.buddy_on && buddy::WORKING.load(Relaxed) && far < 0.35 {
        b.start(Clip::Think); // Claude is at it
        return;
    }
    if far > 0.72 {
        let mut ball = ball::BALL.lock().unwrap();
        if ball.out && !ball.held {
            ball.interest = 8.0; // let's play
            ball.home = b.feet.0;
            return;
        }
    }
    // machines without fuel stay put, tired pets take it easy
    let able = mood != Pose::Tired && mood != Pose::Sad && !(mood == Pose::Hungry && pet.machine());
    // the other pet is here: go and see it, or chase it
    if let Some((x, y)) = b.other.filter(|o| walks && able && (o.1 - b.feet.1).abs() < 40.0 && b.rnd() < 0.4) {
        let _ = y;
        let d = x - b.feet.0;
        b.facing = if d > 0.0 { 1 } else { -1 };
        b.mode = Mode::Walk;
        b.t = 0.0;
        let chase = b.rnd() < 0.35 && d.abs() > 150.0;
        b.run = chase;
        b.walk_left = if chase { d.abs() + 250.0 } else { (d.abs() - 85.0).max(1.0) };
        b.meet = !chase;
        if chase {
            unsafe { friend::tell(Play::Chase, b.feet.0) };
        }
        return;
    }
    if b.perch.is_some() && walks && able && far < 0.3 {
        // down again, with a hop
        let dir = if b.rnd() < 0.5 { 1.0 } else { -1.0 };
        b.facing = dir as i32;
        b.stop();
        b.clip = None;
        b.perch = None;
        b.mode = Mode::Air;
        b.vel = (dir * (200.0 + 260.0 * b.rnd()), -420.0);
        b.spin = 0.0;
        b.soft = true;
        return;
    }
    if walks && able && s.visit_on && b.screens.len() > 1 && far < 0.16 && travel(s) {
        return;
    }
    if walks && able && s.perch_on && (0.16..0.5).contains(&far) && climb(s) {
        return;
    }
    let b = &mut s.body;
    if walks && able && (r < 0.5 || !tricks) {
        b.facing = if b.rnd() < 0.5 { 1 } else { -1 };
        b.run = b.boost > 0.0 || b.rnd() < 0.15;
        b.walk_left = 120.0 + b.rnd() * if b.run { 800.0 } else { 420.0 };
        b.mode = Mode::Walk;
        b.t = 0.0;
    } else if tricks {
        let common = if pet.arms.is_none() { COMMON_MACHINE } else { COMMON };
        let pool: &[Clip] = if !able { &[Clip::Yawn, Clip::Look, Clip::Sit] } else if b.rnd() < 0.45 { pet.tricks } else { common };
        let i = (b.rnd() * pool.len() as f32) as usize;
        let c = pool[i.min(pool.len() - 1)];
        if c.travel(0.8) != 0.0 || c.travel(0.2) != 0.0 {
            b.facing = if b.rnd() < 0.5 { 1 } else { -1 };
        }
        b.start(c);
    }
}

/// An act just ended: what follows?
fn finished(s: &mut State, c: Clip, cursor_x: f32) {
    let b = &mut s.body;
    b.clip = None;
    b.t = 0.0;
    match c {
        Clip::Flinch if b.next < 0.0 => {
            b.next = 5.0;
            b.start(Clip::Dizzy);
        }
        Clip::Dizzy if b.hits > 0 => {
            b.hits = 0;
            b.start(Clip::Angry);
        }
        Clip::Angry => {
            // off in a huff
            b.facing = if cursor_x < b.feet.0 { 1 } else { -1 };
            if s.walk_on && !s.focus {
                b.mode = Mode::Walk;
                b.run = true;
                b.walk_left = 350.0;
            }
        }
        Clip::Eat => {
            let boost = FOODS[b.food].boost;
            s.mood.on_treat();
            if boost > 0.0 {
                b.boost = boost;
                b.next = 0.5;
            }
        }
        Clip::Wave | Clip::Dance | Clip::Cheer => b.next = b.next.max(4.0),
        _ => {}
    }
}

/// The little things that fly off: hearts, dust, notes, z's.
fn emit(b: &mut Body, c: Clip, pet: &Pet, px: f32, dt: f32) {
    let (t0, t1) = (b.t - dt, b.t);
    let passed = |at: f32| t0 < at && t1 >= at;
    let (x, y) = b.feet;
    let (w, h) = (pet.w() as f32 * px, pet.h() as f32 * px);
    let f = b.facing as f32;
    let top = y - h;
    let every = match c {
        Clip::Sleep => 1.3,
        Clip::Love => 0.35,
        Clip::Cheer | Clip::Glitch | Clip::Beam => 0.22,
        Clip::Hot => 0.45,
        Clip::Think => 0.6,
        Clip::Low => 1.1,
        Clip::Charge => 0.12,
        Clip::Sad => 1.2,
        Clip::Tired | Clip::Held => 1.6,
        Clip::Dance | Clip::Honk => 0.42,
        Clip::Angry => 0.18,
        Clip::Eat => 0.2,
        Clip::Run | Clip::Drift | Clip::Slide | Clip::Wheelie | Clip::Rev => 0.1,
        Clip::Roar => 0.35,
        Clip::Wag => 0.7,
        Clip::Loop => 0.12,
        Clip::Float => 0.6,
        Clip::Sip => 0.5,
        _ => 0.0,
    };
    if every > 0.0 {
        b.emit += dt;
        if b.emit >= every {
            b.emit = 0.0;
            let (r1, r2) = (b.rnd() - 0.5, b.rnd());
            let gold = rgb(0xff, 0xe0, 0x66);
            let gray = rgb(0xb8, 0xb6, 0xc4);
            match c {
                Clip::Sleep => b.puff(Bit::Zed, x + w * 0.3, top + h * 0.15, 9.0, -20.0, 0.0, 2.2, UP_BLUE),
                Clip::Love | Clip::Wag => b.puff(Bit::Heart, x + w * r1, top + 4.0, r1 * 20.0, -38.0, 0.0, 1.1, PINK),
                Clip::Cheer | Clip::Loop | Clip::Beam => b.puff(Bit::Spark, x + w * 1.2 * r1, top + h * r2 * 0.7, 0.0, -20.0, 0.0, 0.6, gold),
                Clip::Think => b.puff(Bit::Dust, x + w * 0.35, top - 2.0, 6.0, -16.0, 0.0, 1.0, CREAM),
                Clip::Hot => b.puff(Bit::Drop, x + w * r1 * 1.1, top + h * 0.15, r1 * 60.0, -40.0, 260.0, 0.8, rgb(0xb8, 0xe6, 0xff)),
                Clip::Low => b.puff(Bit::Cell, x, top - 6.0, 0.0, -12.0, 0.0, 1.0, TOMATO),
                Clip::Charge => b.puff(Bit::Spark, x + w * r1 * 1.3, top + h * r2, r1 * 120.0, -50.0, 120.0, 0.5, gold),
                Clip::Glitch => b.puff(Bit::Spark, x + w * r1, top + h * r2, r1 * 160.0, -60.0, 300.0, 0.5, rgb(0x6f, 0xf0, 0xd8)),
                Clip::Sad => b.puff(Bit::Drop, x - w * 0.22, top + h * 0.45, -8.0, 10.0, 160.0, 0.8, UP_BLUE),
                Clip::Tired | Clip::Held => b.puff(Bit::Drop, x + w * 0.45, top + h * 0.2, 14.0, -10.0, 160.0, 0.8, rgb(0xb8, 0xe6, 0xff)),
                Clip::Dance => b.puff(Bit::Note, x + w * r1 * 1.4, top, r1 * 24.0, -30.0, 0.0, 1.2, UP_BLUE),
                Clip::Honk => b.puff(Bit::Note, x + f * w * 0.5, top + h * 0.3, f * 40.0, -30.0, 0.0, 0.9, gold),
                Clip::Angry => b.puff(Bit::Smoke, x + if r2 < 0.5 { -w * 0.4 } else { w * 0.4 }, top, r1 * 60.0, -46.0, 0.0, 0.7, gray),
                Clip::Eat => {
                    let c = pet_color(FOODS[b.food].crumb).unwrap_or(CREAM);
                    let at = if pet.arms.is_some() { 0.0 } else { -f * w * 0.5 };
                    b.puff(Bit::Crumb, x + at + r1 * px * 5.0, y - h * 0.4, r1 * 90.0, -70.0, 420.0, 0.6, c);
                }
                Clip::Run | Clip::Drift | Clip::Slide | Clip::Wheelie => b.puff(Bit::Dust, x - f * w * 0.45, y - 3.0, -f * 30.0, -14.0 - 20.0 * r2, 0.0, 0.45, gray),
                Clip::Rev => b.puff(Bit::Smoke, x - f * w * 0.55, y - h * 0.25, -f * (40.0 + 30.0 * r2), -18.0, 0.0, 0.8, gray),
                Clip::Roar => b.puff(Bit::Alarm, x + w * r1 * 1.2, top - 4.0, r1 * 40.0, -24.0, 0.0, 0.6, TOMATO),
                Clip::Float => b.puff(Bit::Spark, x + w * r1 * 1.6, y - h * r2, 0.0, 12.0, 0.0, 0.9, CREAM),
                Clip::Sip => b.puff(Bit::Smoke, x + f * w * 0.15, y - h * 0.45, r1 * 8.0, -22.0, 0.0, 0.8, CREAM),
                _ => {}
            }
        }
    }
    let cloud = |b: &mut Body, n: u32, at: (f32, f32), c: u32, bit: Bit| {
        for i in 0..n {
            let a = i as f32 * std::f32::consts::TAU / n as f32;
            b.puff(bit, at.0, at.1, a.cos() * 55.0, a.sin() * 45.0 - 12.0, 0.0, 0.5, c);
        }
    };
    let gray = rgb(0xb8, 0xb6, 0xc4);
    match c {
        Clip::Sneeze if passed(0.82) => cloud(b, 5, (x + f * w * 0.4, top + h * 0.45), CREAM, Bit::Dust),
        Clip::Vanish if passed(0.3) || passed(1.22) => cloud(b, 9, (x, y - h * 0.5), gray, Bit::Smoke),
        Clip::Spook if passed(0.9) => cloud(b, 3, (x, top), UP_BLUE, Bit::Alarm),
        Clip::Hop if passed(0.65) => cloud(b, 4, (x, y - 3.0), gray, Bit::Dust),
        Clip::Backflip if passed(1.0) => cloud(b, 5, (x, y - 3.0), gray, Bit::Dust),
        Clip::Tongue if passed(1.05) => b.puff(Bit::Heart, x, top, 0.0, -30.0, 0.0, 1.0, PINK),
        Clip::Phone if passed(1.6) => b.puff(Bit::Alarm, x + f * w * 0.3, top, 0.0, -20.0, 0.0, 0.8, GREEN),
        Clip::Jiggle if passed(0.1) => cloud(b, 4, (x, y - 3.0), CREAM, Bit::Dust),
        _ => {}
    }
}

fn key_of(r: &Rig, c: Clip, px: f32, body_w: f32, body_h: f32, hover: bool) -> u64 {
    let mut k: u64 = c as u64;
    let mut put = |v: f32| k = (k.rotate_left(7) ^ (v.round() as i64 as u64)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for v in [r.dx * px, r.dy * px, r.sx * body_w * px, r.sy * body_h * px, r.shear * body_h * px, r.rot, r.head.0 * 3.0, r.head.1 * 3.0] {
        put(v);
    }
    for v in [r.legs[0].0 * 3.0, r.legs[0].1 * 3.0, r.legs[1].0 * 3.0, r.legs[1].1 * 3.0, r.crouch * 6.0, r.arms[0] / 5.0, r.arms[1] / 5.0] {
        put(v);
    }
    for v in [r.eyes as u32 as f32, r.look * 3.0, r.mouth * 5.0, r.tail * 4.0, r.wheel / 12.0, r.alpha * 16.0, r.red * 12.0, r.glitch as f32] {
        put(v);
    }
    if hover {
        put(r.hem * 8.0);
    }
    match r.prop {
        Prop::Food(_, gone) => put(gone * 8.0),
        Prop::Tongue(v) | Prop::Beam(v) => put(v * 20.0),
        Prop::Phone => put(101.0),
        Prop::Cup => put(102.0),
        Prop::Ball => put(120.0),
        Prop::Umbrella => put(103.0),
        Prop::Laptop(v) => put(104.0 + v * 6.0),
        Prop::None => {}
    }
    k
}

/// Is anything going on that needs every frame?
pub(crate) fn lively() -> bool {
    let s = ST.lock().unwrap();
    let b = &s.body;
    let still = b.mode == Mode::Rest && matches!(b.shown, Clip::Stand | Clip::Sleep | Clip::Tired | Clip::Sad);
    !still || s.down || !b.parts.is_empty() || b.meal.is_some() || b.clip.is_some() || b.perch.is_some() || ball::BALL.lock().unwrap().out
}

/// The part of the window the pet and the little things around it can be in, in its coordinates.
fn busy_area(s: &State, g: &Geo, win: (i32, i32)) -> RECT {
    let (fx, fy) = g.feet();
    let px = g.px;
    let mut r = RECT { left: fx - 20 * px, top: fy - 34 * px, right: fx + 20 * px, bottom: fy + 4 * px };
    for q in &s.body.parts {
        let (x, y) = (q.x as i32 - win.0, q.y as i32 - win.1);
        r = presence::union(r, RECT { left: x - 8 * px, top: y - 8 * px, right: x + 8 * px, bottom: y + 8 * px });
    }
    if let Some(m) = &s.body.meal {
        let (x, y) = (m.x as i32 - win.0, m.y as i32 - win.1);
        r = presence::union(r, RECT { left: x - 8 * px, top: y - 14 * px, right: x + 8 * px, bottom: y + 4 * px });
    }
    r
}

/// Returns the seconds since the last step.
pub(crate) unsafe fn step(hwnd: HWND) -> f32 {
    let visible = IsWindowVisible(hwnd) != 0;
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let mut p: POINT = zeroed();
    GetCursorPos(&mut p);
    let now = GetTickCount();
    let (target, repaint, area, dt) = {
        let mut st = ST.lock().unwrap();
        let s = &mut *st;
        if s.body.last_ms == 0 {
            // a different pet every time it starts
            friend::publish(hwnd, (wr.left as f32, wr.bottom as f32));
            s.body.seed = (now ^ std::process::id().rotate_left(16)).wrapping_mul(2654435761) | 1;
        }
        let dt = (now.wrapping_sub(s.body.last_ms) as f32 / 1000.0).clamp(0.0, 0.15);
        s.body.last_ms = now;
        let playing = s.dance_on && music::PLAYING.load(Relaxed) && !media::VIDEO.load(Relaxed);
        media::WANTED.store(visible && (s.menu || s.show_now || (s.dance_on && music::PLAYING.load(Relaxed))), Relaxed);
        music::WANTED.store(s.dance_on && visible, Relaxed);
        buddy::WANTED.store(s.buddy_on && visible, Relaxed);
        let (finished_now, asked_now) = (buddy::FINISHED.load(Relaxed), buddy::ASKED.load(Relaxed));
        let news = (finished_now != s.body.finished, asked_now != s.body.asked);
        (s.body.finished, s.body.asked) = (finished_now, asked_now);
        if !visible {
            s.body.parts.clear();
            return 0.0;
        }
        if s.body.screens.is_empty() || now.wrapping_sub(s.body.screens_ms) > 3000 {
            s.body.screens = screens();
            s.body.screens_ms = now;
        }
        let g_was = geo(s);
        let pet = &PETS[s.pet];
        let (fx, fy) = g_was.feet();
        let here = ((wr.left + fx) as f32, (wr.top + fy) as f32);
        if (wr.right - wr.left, wr.bottom - wr.top) != (g_was.w, g_was.h) {
            return 0.0; // about to be resized
        }
        if matches!(s.body.mode, Mode::Rest | Mode::Walk) && ((s.body.feet.0 - here.0).abs() > 1.5 || (s.body.feet.1 - here.1).abs() > 1.5) {
            s.body.feet = here; // somebody else moved the window
            friend::publish(hwnd, here);
        }
        // every monitor may have its own pixel size
        let mon = MonitorFromPoint(POINT { x: s.body.feet.0 as i32, y: s.body.feet.1 as i32 - 2 }, MONITOR_DEFAULTTONEAREST);
        if mon as isize != s.body.mon && s.body.mode != Mode::Pull {
            s.body.mon = mon as isize;
            (s.scale, s.max_scale) = scales(mon);
        }
        let scale = eff_scale(s);
        let g0 = geo(s);
        // the windows of other programs, as ground to stand on
        let mut fresh = false;
        if !s.perch_on {
            s.body.perches.clear();
            s.body.perch = None;
        } else if now.wrapping_sub(s.body.perch_ms) > 900 {
            s.body.perch_ms = now;
            s.body.perches = perch::scan(&s.body.screens, g0.ch + 16);
            fresh = true;
        }
        if matches!(s.body.mode, Mode::Rest | Mode::Walk) {
            let b = &mut s.body;
            match b.perch {
                Some((w, was)) => match perch::place_of(w as HWND) {
                    Some(is) => {
                        let (dx, dy) = (is.left - was.left, is.top - was.top);
                        b.perch = Some((w, is));
                        if shaken(dx, dy, scale) {
                            b.perch = None;
                            b.stop();
                            b.clip = None;
                            b.mode = Mode::Air;
                            b.vel = ((dx as f32 * 9.0).clamp(-900.0, 900.0), -260.0 * scale);
                            b.spin = 0.0;
                        } else {
                            // it rides along
                            b.feet.0 += dx as f32;
                            b.feet.1 = is.top as f32;
                            for p in b.perches.iter_mut().filter(|p| p.wnd == w && !fresh) {
                                (p.l, p.r, p.y) = (p.l + dx, p.r + dx, p.y + dy);
                            }
                            // walked or pushed off the end, or something covers the edge now
                            let covered = fresh && perch::under(&b.perches, b.feet.0, b.feet.1).is_none();
                            if (b.feet.0 as i32) < is.left || (b.feet.0 as i32) > is.right || covered {
                                b.perch = None;
                            }
                        }
                    }
                    None => b.perch = None, // closed or minimized under its feet
                },
                None => {
                    if let Some(p) = perch::under(&b.perches, b.feet.0, b.feet.1) {
                        b.perch = perch::place_of(p.wnd as HWND).map(|r| (p.wnd, r));
                    }
                }
            }
        }
        let mood = s.mood.act();
        let (px, half, tall) = (g0.px as f32, g0.cw as f32 / 2.0, g0.ch as f32);
        let cursor = (p.x as f32, p.y as f32);
        let hovered = !s.through && contains(&s.pet_rect, p.x - wr.left, p.y - wr.top);
        let free = free(s);
        let wa = work_at(s.body.feet.0 as i32, s.body.feet.1 as i32 - g0.ch / 2);
        let floor = match s.body.perch {
            Some((_, r)) => r.top,
            None => world::standing_on(&s.body.screens, s.body.feet.0, s.body.feet.1).map_or(wa.bottom, |i| s.body.screens[i].work.bottom),
        };
        let grounded = !s.gravity || (floor as f32 - s.body.feet.1).abs() < 3.0;
        let mut landed = false;

        if hovered {
            s.body.ignored = 0.0;
        }
        {
            let b = &mut s.body;
            b.ignored += dt;
            b.boost = (b.boost - dt).max(0.0);
            b.hit_cd = (b.hit_cd - dt).max(0.0);
            b.bumped = (b.bumped - dt).max(0.0);
            b.other = friend::other_feet();
            if b.hit_cd == 0.0 && b.clip != Some(Clip::Dizzy) {
                b.hits = 0;
            }
            for q in b.parts.iter_mut() {
                q.age += dt;
                q.vy += q.g * dt;
                q.x += q.vx * dt;
                q.y += q.vy * dt;
            }
            b.parts.retain(|q| q.age < q.life);
            if let Some(m) = &mut b.meal {
                m.age += dt;
                if !m.landed {
                    m.vy += GRAVITY * 0.6 * scale * dt;
                    m.y += m.vy * dt;
                    if m.y >= m.floor {
                        m.y = m.floor;
                        m.vy = if m.vy > 160.0 { -m.vy * 0.35 } else { 0.0 };
                        m.landed = m.vy == 0.0;
                    }
                }
                if m.age > 45.0 {
                    b.meal = None;
                }
            }
        }

        if matches!(s.body.mode, Mode::Held | Mode::Pull) && !(s.down && s.body.on_pet) {
            release(s); // nobody is holding it any more
        }

        // the coding buddy: types along, and tells when Claude Code is done or asks something
        if s.buddy_on {
            let mut li = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
            GetLastInputInfo(&mut li);
            let buttons = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|k| GetAsyncKeyState(*k as i32) < 0);
            s.body.keys.look(li.dwTime, (p.x, p.y), buttons, dt);
            if s.body.keys.heat > 0.5 {
                s.body.ignored = 0.0;
            }
            let at_rest = matches!(s.body.mode, Mode::Rest | Mode::Walk) && s.body.meal.is_none();
            if (news.0 || news.1) && at_rest && !s.menu && !s.down {
                let (x, y, up) = (s.body.feet.0, s.body.feet.1, g0.ch as f32);
                s.body.stop();
                s.body.clip = None;
                if news.0 {
                    s.mood.on_cheer();
                    sound::play(Sound::Done);
                    gain(s, wardrobe::XP_CODE);
                } else {
                    sound::play(Sound::Ask);
                    s.body.start(Clip::Wave);
                }
                let c = if news.0 { GREEN } else { ALERT_ORANGE };
                s.body.puff(Bit::Alarm, x, y - up - 8.0, 0.0, -10.0, 0.0, 2.5, c);
                if s.talk_on && presence::chatty(s.minimal, s.hidden) && s.msg_left == 0 {
                    say_text(s, if news.0 { "Claude is done." } else { "Claude has a question for you." }, 10);
                }
            }
            let typing = s.body.keys.heat > 0.55 && buddy::is_coding(&s.last_app);
            if typing && free && at_rest && !hovered && mood == Pose::Idle {
                if s.body.mode == Mode::Walk {
                    s.body.stop();
                }
                if s.body.clip.map_or(true, |c| DANCES.contains(&c)) {
                    s.body.start(Clip::Type);
                }
            }
        }

        // music: feel the beat, and dance if there is nothing better to do
        {
            let beats = music::BEATS.load(Relaxed);
            let new = playing && beats != s.body.beats;
            s.body.beats = beats;
            s.body.beat = if new { 1.0 } else { s.body.beat * (1.0 - 6.0 * dt).max(0.0) };
            if playing {
                s.body.ignored = 0.0;
            }
            let party = playing && free && mood != Pose::Sleep && s.body.meal.is_none() && grounded;
            if party && s.body.mode == Mode::Walk {
                s.body.stop();
            }
            if party && s.body.mode == Mode::Rest && s.body.clip.is_none() {
                let b = &mut s.body;
                let pool = if pet.arms.is_none() { DANCES_MACHINE } else { DANCES };
                let i = (b.rnd() * pool.len() as f32) as usize;
                b.facing = if b.rnd() < 0.5 { 1 } else { -1 };
                b.start(pool[i.min(pool.len() - 1)]);
            }
            if party && new && s.body.mode == Mode::Rest {
                let b = &mut s.body;
                let (r, up) = (b.rnd() - 0.5, g0.ch as f32);
                let c = [PINK, UP_BLUE, GREEN, rgb(0xff, 0xe0, 0x66)][(beats % 4) as usize];
                let (x, y) = b.feet;
                b.puff(Bit::Note, x + r * g0.cw as f32 * 1.3, y - up * 0.9, r * 40.0, -34.0, 0.0, 1.1, c);
            }
        }

        // what the body is doing
        match s.body.mode {
            Mode::Route => {
                let stop = s.menu || s.down || s.quiet || s.hidden || s.focus || s.alert.is_some() || s.body.route.is_empty();
                if stop {
                    s.body.stop();
                    landed = true;
                } else {
                    let here = s.body.feet;
                    let edge = world::standing_on(&s.body.screens, here.0, here.1).map(|i| s.body.screens[i].full.bottom as f32);
                    let b = &mut s.body;
                    let leg = b.route[0];
                    let d = (leg.to.0 - here.0, leg.to.1 - here.1);
                    if d.0.abs() > 1.0 {
                        b.facing = if d.0 > 0.0 { 1 } else { -1 };
                    }
                    match leg.way {
                        Way::Hop => {
                            b.vel = leap(here, leg.to, GRAVITY * scale.max(0.8), 430.0 * scale);
                            b.stop();
                            b.mode = Mode::Air;
                            b.spin = 0.0;
                            b.soft = true;
                        }
                        Way::Drop => {
                            b.stop();
                            b.mode = Mode::Air;
                            b.vel = (0.0, 40.0);
                            b.spin = 0.0;
                            b.through = edge;
                            b.glide = true;
                            b.soft = true;
                        }
                        way => {
                            let base = match way {
                                Way::Stairs => 16.0,
                                Way::Ladder => 14.0,
                                Way::Float => 24.0,
                                _ => pace(pet.machine(), b.run),
                            };
                            let far = d.0.hypot(d.1);
                            let go = base * px * dt * if far > 900.0 * scale { 1.6 } else { 1.0 };
                            if far <= go {
                                b.feet = leg.to;
                                b.route.remove(0);
                                if b.route.is_empty() {
                                    b.mode = Mode::Rest;
                                    b.trail.clear();
                                    b.run = false;
                                    b.next = 2.0;
                                    landed = true;
                                }
                            } else {
                                b.feet = (here.0 + d.0 / far * go, here.1 + d.1 / far * go);
                            }
                        }
                    }
                }
            }
            Mode::Held => {
                let b = &mut s.body;
                if dt > 0.0 {
                    let v = ((cursor.0 - b.cursor.0) / dt, (cursor.1 - b.cursor.1) / dt);
                    b.vel = (b.vel.0 * 0.55 + v.0 * 0.45, b.vel.1 * 0.55 + v.1 * 0.45);
                }
                b.cursor = cursor;
                let want = (b.vel.0 * 0.045).clamp(-55.0, 55.0);
                b.swing_v += (-(b.swing - want) * 90.0 - b.swing_v * 7.0) * dt;
                b.swing += b.swing_v * dt;
                let wa = work_at(p.x, p.y);
                b.feet = (
                    (cursor.0 + b.grab.0).clamp(wa.left as f32 + half, (wa.right as f32 - half).max(wa.left as f32 + half)),
                    (cursor.1 + b.grab.1).clamp(wa.top as f32 + tall, (wa.bottom as f32).max(wa.top as f32 + tall)),
                );
            }
            Mode::Air => {
                let b = &mut s.body;
                let mut f = Flight { x: b.feet.0, y: b.feet.1, vx: b.vel.0, vy: b.vel.1 };
                // the whole world is its playground, not just this monitor
                let mid = f.y - tall / 2.0;
                let mut area = wa;
                if let Some((l, r)) = world::span(&b.screens, f.x, mid) {
                    (area.left, area.right) = (l, r);
                }
                if let Some(y) = world::floor_below(&b.screens, f.x, f.y, b.through) {
                    area.bottom = y;
                }
                if let Some(y) = perch::below(&b.perches, f.x, f.y).filter(|_| b.vel.1 >= 0.0 && b.through.is_none()) {
                    area.bottom = area.bottom.min(y);
                }
                if let Some(y) = world::ceiling(&b.screens, f.x, mid) {
                    area.top = y;
                }
                if b.glide {
                    f.vy = f.vy.min(210.0 * scale);
                    f.vx = 26.0 * scale * (b.t * 2.0).sin();
                }
                let before = f.vy;
                let end = fly(&mut f, dt, GRAVITY * scale.max(0.8), &area, half, tall);
                b.spin = (b.spin + b.vel.0 * 0.35 * dt).clamp(-1440.0, 1440.0);
                if before > 520.0 && f.vy < 0.0 {
                    for i in 0..4 {
                        b.puff(Bit::Dust, f.x + (i as f32 - 1.5) * half * 0.5, f.y - 3.0, (i as f32 - 1.5) * 50.0, -20.0, 0.0, 0.4, rgb(0xb8, 0xb6, 0xc4));
                    }
                    b.spin *= 0.3;
                    if before > DIZZY_IMPACT * scale.max(0.8) && !b.soft {
                        b.hits = HITS_TO_DIZZY; // remembers that it hurt until it comes to rest
                        b.hit_cd = 6.0;
                    }
                }
                b.feet = (f.x, f.y);
                b.vel = (f.vx, f.vy);
                if let Some((ox, oy)) = b.other.filter(|_| b.bumped == 0.0 && b.vel.0.hypot(b.vel.1) > 500.0) {
                    if (ox - f.x).abs() < half * 1.3 && (oy - f.y).abs() < tall {
                        b.bumped = 1.5;
                        friend::tell(Play::Bump, f.x);
                        b.vel.0 *= -0.4;
                    }
                }
                if end.is_some() {
                    sound::play(Sound::Land);
                    b.mode = Mode::Rest;
                    b.through = None;
                    b.glide = false;
                    b.soft = false;
                    b.spin = 0.0;
                    landed = true;
                    b.start(if b.hits >= HITS_TO_DIZZY { Clip::Dizzy } else { Clip::Land });
                    if b.clip == Some(Clip::Dizzy) {
                        b.hits = 0;
                    }
                }
            }
            Mode::Walk | Mode::Rest => {
                // fetch a meal that has landed
                let reach = half + 2.0 * px;
                let can_eat = !s.menu && !s.down && s.body.clip.map_or(true, |c| c != Clip::Eat && c != Clip::Flinch && c != Clip::Dizzy);
                if let (Some(m), true) = (s.body.meal.filter(|m| m.landed), can_eat) {
                    let d = m.x - s.body.feet.0;
                    if d.abs() <= reach || s.focus || m.age > 20.0 {
                        let f = &FOODS[m.food];
                        let ev = s.mood.on_fed(f.gain, f.joy);
                        let b = &mut s.body;
                        b.meal = None;
                        b.mode = Mode::Rest;
                        b.food = m.food;
                        if d.abs() > 1.0 {
                            b.facing = if d > 0.0 { 1 } else { -1 };
                        }
                        b.start(Clip::Eat);
                        sound::play(Sound::Eat);
                        gain(s, wardrobe::XP_FED);
                        if let Some(ev) = ev {
                            say(s, ev);
                        }
                    } else {
                        let b = &mut s.body;
                        b.clip = None;
                        b.mode = Mode::Walk;
                        b.facing = if d > 0.0 { 1 } else { -1 };
                        b.run = d.abs() > 60.0 * scale;
                        b.walk_left = d.abs();
                    }
                }
                let hunting = s.body.meal.is_some() | fetch(s, hovered, half, px);
                if s.body.mode == Mode::Walk {
                    let speed = pace(pet.machine(), s.body.run) * px;
                    let b = &mut s.body;
                    if b.walk_left <= 0.0 || (!hunting && (!free || hovered)) || s.menu || s.down {
                        if b.meet && b.walk_left <= 0.0 && !hunting {
                            if let Some((x, _)) = b.other.filter(|o| (o.0 - b.feet.0).abs() < 260.0 && (o.1 - b.feet.1).abs() < 40.0) {
                                let play = friend::meeting(b.rnd());
                                friend::tell(play, b.feet.0);
                                b.facing = if x > b.feet.0 { 1 } else { -1 };
                                if let Some(c) = friend::clip_of(play) {
                                    b.start(c);
                                }
                                let (px0, py0) = (b.feet.0, b.feet.1 - g0.ch as f32);
                                b.puff(Bit::Heart, px0, py0, 0.0, -30.0, 0.0, 1.2, PINK);
                            }
                        }
                        b.meet = false;
                        b.mode = Mode::Rest;
                        b.run = false;
                        landed = true;
                    } else {
                        let ledge = b.perch.and_then(|_| perch::under(&b.perches, b.feet.0, b.feet.1)).map(|p| (p.l, p.r));
                        let reach = if ledge.is_some() { ledge } else if s.visit_on { world::standing_on(&b.screens, b.feet.0, b.feet.1).map(|i| world::level_span(&b.screens, i)) } else { None }.unwrap_or((wa.left, wa.right));
                        let (l, r) = (reach.0 as f32 + half + g0.edge as f32, reach.1 as f32 - half - g0.edge as f32);
                        let nx = b.feet.0 + b.facing as f32 * speed * dt;
                        b.walk_left -= speed * dt;
                        if nx < l && b.facing < 0 {
                            b.facing = 1;
                        } else if nx > r && b.facing > 0 {
                            b.facing = -1;
                        }
                        b.feet.0 = nx.clamp(l, r.max(l));
                        if s.gravity && grounded && b.perch.is_none() {
                            if let Some(y) = world::level_floor(&b.screens, b.feet.0, b.feet.1) {
                                b.feet.1 = y as f32; // up or down a kerb
                            }
                        }
                    }
                } else if let Some(c) = s.body.clip {
                    let moves = c.travel(s.body.t);
                    if moves != 0.0 && s.walk_on && !s.focus && !s.menu && !s.down {
                        let b = &mut s.body;
                        let (l, r) = (wa.left as f32 + half, wa.right as f32 - half);
                        let nx = b.feet.0 + b.facing as f32 * moves * px * dt;
                        if nx < l || nx > r {
                            b.facing = -b.facing;
                        }
                        b.feet.0 = nx.clamp(l, r.max(l));
                    }
                    if c == Clip::Vanish && s.walk_on && !s.focus && s.body.t < 0.8 && s.body.t + dt >= 0.8 {
                        let b = &mut s.body;
                        let jump = (90.0 + 160.0 * b.rnd()) * scale * if b.rnd() < 0.5 { -1.0 } else { 1.0 };
                        b.feet.0 = (b.feet.0 + jump).clamp(wa.left as f32 + half, (wa.right as f32 - half).max(wa.left as f32 + half));
                    }
                }
                // spontaneous acts never outlive the moment they fit
                if let Some(c) = s.body.clip {
                    let forced = matches!(c, Clip::Flinch | Clip::Dizzy | Clip::Angry | Clip::Eat | Clip::Land | Clip::Boing);
                    if !forced && (mood == Pose::Sleep || (s.focus && s.pomo_running)) {
                        s.body.clip = None;
                    }
                }
                let idle = s.body.mode == Mode::Rest && s.body.clip.is_none() && s.body.meal.is_none();
                let relaxed = matches!(mood, Pose::Idle | Pose::Hungry | Pose::Tired | Pose::Sad);
                let on_air = !grounded && (s.body.mode == Mode::Rest && free || s.body.mode == Mode::Walk);
                if on_air && s.body.meal.is_none() {
                    // nothing under its feet: the screen changed, or it was left up here
                    s.body.stop();
                    s.body.clip = None;
                    s.body.vel = (0.0, 0.0);
                    s.body.spin = 0.0;
                    s.body.mode = Mode::Air;
                } else if idle && free && relaxed && !hovered {
                    s.body.next -= dt;
                    if s.body.next <= 0.0 {
                        choose(s, grounded);
                    }
                }
            }
            Mode::Pull => {}
        }

        // the running act
        let clip = s.body.clip(mood);
        if clip != s.body.shown {
            s.body.shown = clip;
            s.body.t = 0.0;
            s.body.emit = 0.0;
        } else {
            s.body.t += dt;
        }
        emit(&mut s.body, clip, pet, px, dt);
        if let (Some(len), true) = (clip.secs(), s.body.clip == Some(clip)) {
            if s.body.t >= len {
                finished(s, clip, cursor.0);
            }
        }

        // which corner of its window the pet sits in, and where that puts the window
        let mut g = g0;
        let settled = !s.menu && s.body.mode != Mode::Route && s.alert.is_none() && s.body.meal.is_none() && s.msg_left == 0 && s.fact_left == 0;
        if settled {
            let give = (60.0 * scale) as i32;
            let at = (s.body.feet.0 as i32, s.body.feet.1 as i32 - g.ch / 2);
            let wa = work_at(at.0, at.1);
            g.flip = (side(g.flip.0, at.0, (wa.left + wa.right) / 2, give), side(g.flip.1, at.1, (wa.top + wa.bottom) / 2, give));
        }
        if s.body.mode == Mode::Route && !s.menu {
            // keep what it is heading for inside the window
            if let Some(l) = s.body.route.first().filter(|l| matches!(l.way, Way::Stairs | Way::Ladder | Way::Float)) {
                g.flip.1 = l.to.1 > s.body.feet.1 + 4.0;
                if (l.to.0 - s.body.feet.0).abs() > 4.0 {
                    g.flip.0 = l.to.0 > s.body.feet.0;
                }
            }
        } else if s.body.glide {
            g.flip.1 = false; // room for the umbrella
        }
        s.flip = g.flip;
        let (fx, fy) = g.feet();
        let target = (s.body.feet.0.round() as i32 - fx, s.body.feet.1.round() as i32 - fy);
        let resized = (g.w, g.h) != (g_was.w, g_was.h);
        let moved = target != (wr.left, wr.top) || resized;
        if moved {
            s.pos = Some(target);
        }
        if landed {
            save_cfg(s);
        }

        // repaint only when the picture changed
        let mut rig = sample(pet, clip, s.body.t, &s.body.push());
        if s.body.carrying {
            rig.prop = Prop::Ball;
        }
        let key = key_of(&rig, clip, px, pet.w() as f32, pet.h() as f32, pet.build == Build::Hover).wrapping_add(s.body.facing as u64).wrapping_add(s.hat as u64 * 977);
        let alive = !s.body.parts.is_empty() || s.body.meal.is_some() || clip == Clip::Dizzy || clip == Clip::Hungry;
        // the calm poses breathe slowly, so a few pictures a second are plenty
        let calm = matches!(clip, Clip::Stand | Clip::Sleep | Clip::Tired | Clip::Sad | Clip::Sit | Clip::Look | Clip::Phone);
        s.body.since += dt;
        // acts look fine at 20 pictures a second; only real movement gets every frame
        let moving = matches!(s.body.mode, Mode::Walk | Mode::Route | Mode::Held | Mode::Pull | Mode::Air) || clip.travel(s.body.t) != 0.0;
        let due = s.body.since >= if calm { 0.12 } else if moving { 0.0 } else { 0.045 };
        let repaint = moved || (due && (alive || key != s.body.key));
        if repaint {
            s.body.key = key;
            s.body.since = 0.0;
        }
        // only where something changed, unless a panel or a bubble is open as well
        let area = (!moved && !s.menu && !s.focus && s.alert.is_none() && s.body.trail.is_empty()).then(|| {
            let now = busy_area(s, &g, target);
            let both = presence::union(now, s.body.area);
            s.body.area = now;
            both
        });
        (moved.then_some((target.0, target.1, g.w, g.h)), repaint, area, dt)
    };
    if target.is_some() {
        let feet = ST.lock().unwrap().body.feet;
        friend::publish(hwnd, feet);
    }
    if let Some((x, y, w, h)) = target {
        SetWindowPos(hwnd, null_mut(), x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER);
    }
    if repaint {
        match area {
            Some(r) => InvalidateRect(hwnd, &r, 0),
            None => InvalidateRect(hwnd, null(), 0),
        };
        if target.is_some() {
            UpdateWindow(hwnd); // in the same breath as the move, so the pet does not jitter
        }
    }
    dt
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: RECT = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };

    fn throw(x: f32, y: f32, vx: f32, vy: f32) -> (Flight, f32, u32) {
        let mut f = Flight { x, y, vx, vy };
        for n in 0..2000 {
            if let Some(v) = fly(&mut f, 0.033, GRAVITY, &AREA, 48.0, 110.0) {
                return (f, v, n);
            }
            assert!(f.x >= 48.0 && f.x <= 1920.0 - 48.0, "left the screen at {}", f.x);
            assert!(f.y <= 1040.0, "fell through the floor");
        }
        panic!("never landed");
    }

    #[test]
    fn a_thrown_pet_always_lands_on_the_floor_inside_the_screen() {
        for vx in [-2400.0, -300.0, 0.0, 900.0, 2400.0] {
            for vy in [-2400.0, 0.0, 2400.0] {
                let (f, speed, n) = throw(900.0, 500.0, vx, vy);
                assert_eq!(f.y, 1040.0);
                assert!(speed <= 520.0 && n < 400, "took {n} steps");
            }
        }
    }

    #[test]
    fn dropping_from_higher_up_takes_longer() {
        assert!(throw(900.0, 200.0, 0.0, 0.0).2 > throw(900.0, 900.0, 0.0, 0.0).2);
    }

    #[test]
    fn the_side_changes_only_well_past_the_middle() {
        assert!(side(true, 1000, 960, 60));
        assert!(!side(true, 1030, 960, 60));
        assert!(!side(false, 920, 960, 60));
        assert!(side(false, 890, 960, 60));
    }

    #[test]
    fn a_leap_lands_where_it_should() {
        for (from, to) in [((100.0, 500.0), (200.0, 900.0)), ((500.0, 300.0), (380.0, 1400.0))] {
            let g = 1900.0;
            let v = leap(from, to, g, 430.0);
            let (mut x, mut y, mut vy) = (from.0, from.1, v.1);
            let dt = 0.0005;
            while y < to.1 || vy < 0.0 {
                vy += g * dt;
                y += vy * dt;
                x += v.0 * dt;
            }
            assert!((x - to.0).abs() < 3.0f32, "landed at {x} instead of {}", to.0);
        }
    }

    #[test]
    fn the_pc_is_hot_only_after_a_while_and_cools_down_faster() {
        let mut h = 0;
        for _ in 0..HOT - 1 {
            h = heat(h, 95);
        }
        assert!(h < HOT);
        h = heat(h, 95);
        assert!(h >= HOT);
        for _ in 0..HOT {
            h = heat(h, 10);
        }
        assert_eq!(h, 0);
    }

    #[test]
    fn plugging_in_is_noticed_once_and_low_means_low() {
        assert!(just_plugged(Some((30, false)), Some((30, true))));
        assert!(!just_plugged(Some((30, true)), Some((31, true))) && !just_plugged(None, Some((30, true))) && !just_plugged(None, None));
        assert!(running_low(Some((15, false))) && !running_low(Some((15, true))) && !running_low(Some((60, false))) && !running_low(None));
    }

    #[test]
    fn a_ball_bounces_higher_than_a_pet_rolls_and_comes_to_rest() {
        let mut f = Flight { x: 300.0, y: 300.0, vx: 700.0, vy: 0.0 };
        let (mut top, mut bounced, mut n) = (f32::MAX, false, 0);
        while !bounce(&mut f, 0.033, GRAVITY, &AREA, 16.0, 32.0) {
            bounced |= f.vy < 0.0;
            if bounced {
                top = top.min(f.y);
            }
            assert!(f.x >= 16.0 && f.x <= 1904.0 && f.y <= 1040.0);
            n += 1;
            assert!(n < 1200, "never stops");
        }
        assert!(bounced && top < 900.0, "only bounced up to {top}");
        assert_eq!(f.y, 1040.0);
    }

    #[test]
    fn a_window_moved_gently_carries_the_pet_one_yanked_away_does_not() {
        assert!(!shaken(20, 10, 1.0) && !shaken(60, -20, 1.0));
        assert!(shaken(200, 0, 1.0) && shaken(-60, 60, 1.0));
    }

    #[test]
    fn blows_get_worse() {
        assert_eq!(blow(1), Blow::Flinch);
        assert_eq!(blow(HITS_TO_DIZZY), Blow::Dizzy);
        assert_eq!(blow(HITS_TO_FLY), Blow::Fly);
        assert_eq!(blow(HITS_TO_FLY + 3), Blow::Fly);
    }

    #[test]
    fn running_is_faster_and_so_are_machines() {
        assert!(pace(false, true) > pace(false, false));
        assert!(pace(true, false) > pace(false, false));
    }

    #[test]
    fn turning_the_window_around_the_pet_keeps_its_feet_in_place() {
        let mut g = Geo { w: 380, h: 290, px: 6, cw: 96, ch: 108, pad_x: 40, pad_top: 40, pad_bottom: 10, edge: 8, flip: (false, false) };
        let feet = (900, 1040);
        for flip in [(false, false), (true, false), (false, true), (true, true)] {
            g.flip = flip;
            let (fx, fy) = g.feet();
            let win = (feet.0 - fx, feet.1 - fy);
            let (ox, oy) = g.origin();
            assert_eq!((win.0 + ox + g.cw / 2, win.1 + oy + g.ch), feet);
            assert!(ox >= 0 && oy >= 0 && ox + g.cw <= g.w && oy + g.ch <= g.h);
        }
    }

    #[test]
    fn the_pet_can_reach_the_floor_and_both_sides_of_the_screen() {
        let mut g = Geo { w: 380, h: 290, px: 6, cw: 96, ch: 108, pad_x: 40, pad_top: 40, pad_bottom: 10, edge: 8, flip: (false, false) };
        let b = g.bounds(&AREA);
        let (x, y) = (b.right - g.w, b.bottom - g.h); // furthest right and down
        assert_eq!(x + g.origin().0 + g.cw, AREA.right - g.edge);
        assert_eq!(y + g.feet().1, AREA.bottom);
        g.flip = (true, true);
        let b = g.bounds(&AREA);
        assert_eq!(b.left + g.origin().0, AREA.left + g.edge);
        assert_eq!(b.top + g.origin().1, AREA.top + g.edge);
    }

    #[test]
    fn being_grabbed_beats_everything_else() {
        let mut b = Body::new();
        b.start(Clip::Dance);
        b.mode = Mode::Held;
        assert_eq!(b.clip(Pose::Sleep), Clip::Held);
        b.mode = Mode::Rest;
        assert_eq!(b.clip(Pose::Sleep), Clip::Dance);
        b.stop();
        assert_eq!(b.clip(Pose::Sleep), Clip::Sleep);
    }

    #[test]
    fn pulling_stretches_then_lifts_and_letting_go_snaps_back_or_throws() {
        let mut st = ST.lock().unwrap();
        let s = &mut *st;
        s.body = Body::new();
        s.gravity = true;
        let tall = geo(s).ch;
        grab(s, (500, 900), (500.0, 1000.0));
        drag(s, (tall / 5, -tall / 5));
        assert_eq!(s.body.mode, Mode::Pull);
        assert!(s.body.pull.0 > 0.0 && s.body.pull.1 > 0.0, "stretches up and to the right");
        release(s);
        assert_eq!((s.body.mode, s.body.clip), (Mode::Rest, Some(Clip::Boing)));

        grab(s, (500, 900), (500.0, 1000.0));
        drag(s, (0, -tall));
        assert_eq!(s.body.mode, Mode::Held);
        drag(s, (0, -2)); // it does not stick to the ground again while held
        assert_eq!(s.body.mode, Mode::Held);
        s.body.vel = (9000.0, -9000.0);
        release(s);
        assert_eq!(s.body.mode, Mode::Air);
        assert!(s.body.vel.0 <= MAX_THROW && s.body.vel.1 >= -MAX_THROW);

        s.body = Body::new();
        s.gravity = false;
        grab(s, (500, 900), (500.0, 1000.0));
        drag(s, (tall, 0));
        release(s);
        assert_eq!(s.body.mode, Mode::Rest, "without gravity it stays where it is put");
        s.gravity = true;

        // a beating: flinch, then dizzy, then it flies
        s.body = Body::new();
        for _ in 0..HITS_TO_FLY - 1 {
            hit(s, -1.0, (0.0, 0.0));
            assert_eq!(s.body.clip, Some(Clip::Flinch));
        }
        hit(s, -1.0, (0.0, 0.0));
        assert_eq!(s.body.mode, Mode::Air);
        assert!(s.body.vel.0 > 0.0, "hit from the left, flies to the right");
        s.body = Body::new();
    }

    #[test]
    fn stopping_does_not_interrupt_a_meal() {
        let mut b = Body::new();
        b.start(Clip::Eat);
        b.stop();
        assert_eq!(b.clip, Some(Clip::Eat));
    }
}
