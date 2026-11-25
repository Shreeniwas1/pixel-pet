//! Animation: what a pet's body looks like at any moment of any act. Pure maths, no Win32, so a
//! contributor can add an act by adding one match arm. Lengths are in sprite cells, angles in
//! degrees, time in seconds; everything is described for a pet facing right.

use crate::prelude::*;
use std::f32::consts::TAU;

/// Everything a pet can be seen doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Clip {
    // what the mood asks for (loops)
    Stand,
    Sleep,
    Cheer,
    Love,
    Stretch,
    Tired,
    Sad,
    // moving about
    Walk,
    Run,
    Pull,
    Held,
    Fall,
    Land,
    Boing,
    // what was done to it
    Flinch,
    Dizzy,
    Angry,
    Eat,
    // small acts everyone does
    Look,
    Wave,
    Dance,
    Hop,
    Yawn,
    Sneeze,
    Sit,
    Spin,
    Hungry,
    // one character's own acts
    Groom,
    Wag,
    Pounce,
    Jiggle,
    Phone,
    Sip,
    Backflip,
    Rev,
    Drift,
    Honk,
    Glitch,
    Slide,
    Roar,
    Spook,
    Loop,
    Vanish,
    Float,
    Hops,
    Tongue,
    Beam,
    Wheelie,
    // to music
    Groove,
    Headbang,
    Disco,
    // between monitors
    Climb,
    Glide,
}

/// Small acts for anything with arms and legs, and for the machines.
pub(crate) const COMMON: &[Clip] = &[Clip::Look, Clip::Wave, Clip::Dance, Clip::Hop, Clip::Yawn, Clip::Sneeze, Clip::Sit, Clip::Spin];
/// Moves for when music is playing.
pub(crate) const DANCES: &[Clip] = &[Clip::Groove, Clip::Disco, Clip::Dance, Clip::Headbang, Clip::Groove, Clip::Spin];
pub(crate) const DANCES_MACHINE: &[Clip] = &[Clip::Groove, Clip::Dance, Clip::Headbang];
pub(crate) const COMMON_MACHINE: &[Clip] = &[Clip::Look, Clip::Hop, Clip::Dance, Clip::Sneeze];

impl Clip {
    /// Seconds it lasts, or None for something that goes on until it is stopped.
    pub fn secs(self) -> Option<f32> {
        Some(match self {
            Clip::Stand | Clip::Sleep | Clip::Cheer | Clip::Love | Clip::Stretch | Clip::Tired | Clip::Sad => return None,
            Clip::Walk | Clip::Run | Clip::Pull | Clip::Held | Clip::Fall | Clip::Climb | Clip::Glide => return None,
            Clip::Groove => 4.0,
            Clip::Headbang => 3.0,
            Clip::Disco => 4.0,
            Clip::Land => 0.45,
            Clip::Boing => 0.9,
            Clip::Flinch => 0.5,
            Clip::Dizzy => 3.0,
            Clip::Angry => 1.6,
            Clip::Eat => 2.4,
            Clip::Look => 2.6,
            Clip::Wave => 1.8,
            Clip::Dance => 3.6,
            Clip::Hop => 1.4,
            Clip::Yawn => 2.2,
            Clip::Sneeze => 1.6,
            Clip::Sit => 5.0,
            Clip::Spin => 1.6,
            Clip::Hungry => 3.0,
            Clip::Groom => 3.2,
            Clip::Wag => 2.4,
            Clip::Pounce => 1.5,
            Clip::Jiggle => 2.0,
            Clip::Phone => 4.5,
            Clip::Sip => 3.6,
            Clip::Backflip => 1.3,
            Clip::Rev => 2.0,
            Clip::Drift => 2.2,
            Clip::Honk => 1.2,
            Clip::Glitch => 1.6,
            Clip::Slide => 2.0,
            Clip::Roar => 2.0,
            Clip::Spook => 2.4,
            Clip::Loop => 1.8,
            Clip::Vanish => 1.8,
            Clip::Float => 4.0,
            Clip::Hops => 1.8,
            Clip::Tongue => 1.8,
            Clip::Beam => 2.2,
            Clip::Wheelie => 2.0,
        })
    }

    /// Cells per second it travels forward by itself at this moment.
    pub fn travel(self, t: f32) -> f32 {
        match self {
            Clip::Slide if (0.35..1.7).contains(&t) => 34.0 * (1.0 - (t - 0.35) / 1.5),
            Clip::Hops if (t * 2.0).fract() < 0.7 => 14.0,
            Clip::Pounce if (0.6..0.95).contains(&t) => 30.0,
            Clip::Drift => 22.0,
            Clip::Wheelie if t < 1.7 => 12.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Eyes {
    Open,
    Half,
    Closed,
    Happy,
    Wide,
    Angry,
    Dizzy,
}

/// Something held in front of the pet.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Prop {
    None,
    Phone,
    Cup,
    Food(usize, f32), // which food, how much of it is gone (0..1)
    Tongue(f32),      // how far it is out (0..1)
    Beam(f32),
    Umbrella,
}

/// One moment of one act.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Rig {
    pub dx: f32,
    pub dy: f32, // up is positive
    pub sx: f32,
    pub sy: f32,
    pub shear: f32,
    pub rot: f32,
    pub pivot: f32, // height the body turns about: 0 = feet, 1 = top
    pub legs: [(f32, f32); 2], // each foot: forward, lifted
    pub crouch: f32,           // 0 standing .. 1 legs tucked away
    pub arms: [f32; 2],        // 0 hanging, 90 straight out, 180 up (back arm, front arm)
    pub arms_front: bool,      // drawn over the face, bent towards it
    pub head: (f32, f32),
    pub eyes: Eyes,
    pub look: f32,  // -1..1
    pub turn: f32,  // 0 facing us .. 1 turned to walk
    pub mouth: f32, // 0 shut .. 1 wide open
    pub tail: f32,  // sideways swing, -1..1
    pub wheel: f32, // turn, in degrees
    pub hem: f32,   // phase of a ghost's hem
    pub alpha: f32,
    pub red: f32, // 0..1
    pub glitch: u32,
    pub prop: Prop,
    pub hat: usize,
}

impl Rig {
    pub const REST: Rig = Rig {
        dx: 0.0,
        dy: 0.0,
        sx: 1.0,
        sy: 1.0,
        shear: 0.0,
        rot: 0.0,
        pivot: 0.0,
        legs: [(0.0, 0.0); 2],
        crouch: 0.0,
        arms: [10.0; 2],
        arms_front: false,
        head: (0.0, 0.0),
        eyes: Eyes::Open,
        look: 0.0,
        turn: 0.0,
        mouth: 0.0,
        tail: 0.0,
        wheel: 0.0,
        hem: 0.0,
        alpha: 1.0,
        red: 0.0,
        glitch: 0,
        prop: Prop::None,
        hat: 0,
    };

    fn squash(&mut self, sy: f32) {
        self.sy *= sy;
        self.sx /= sy.max(0.3).sqrt();
    }
}

/// What the physics adds: how the body is being swung, stretched or spun.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Push {
    pub pull: (f32, f32), // how far it is pulled, in body heights (x to its right, y up)
    pub swing: f32,       // degrees while dangling
    pub spin: f32,        // degrees while flying
    pub hit: f32,         // -1 hit from its right .. 1 hit from its left
    pub food: usize,
    pub beat: f32, // 1 on the beat of the music, fading to 0
    pub tilt: f32, // degrees, nose down, on a ramp
}

#[cfg(test)]
impl Push {
    pub const NONE: Push = Push { pull: (0.0, 0.0), swing: 0.0, spin: 0.0, hit: 1.0, food: 0, beat: 0.0, tilt: 0.0 };
}

fn osc(t: f32, hz: f32) -> f32 {
    (t * hz * TAU).sin()
}

/// 0 at the ends of the act, 1 in the middle, with smooth edges `e` seconds long.
fn ease(t: f32, len: f32, e: f32) -> f32 {
    let k = (t / e).min((len - t) / e).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// A jump that starts at `t0` and lasts `len`: height 0..1.
fn arc(t: f32, t0: f32, len: f32) -> f32 {
    let k = (t - t0) / len;
    if (0.0..=1.0).contains(&k) {
        4.0 * k * (1.0 - k)
    } else {
        0.0
    }
}

fn blinking(t: f32) -> bool {
    let k = t % 3.7;
    k < 0.12 || (1.9..2.0).contains(&k)
}

/// The body of `pet` at second `t` of `clip`.
pub(crate) fn sample(pet: &Pet, clip: Clip, t: f32, p: &Push) -> Rig {
    let mut r = Rig::REST;
    let len = clip.secs().unwrap_or(1.0);
    let k = (t / len).clamp(0.0, 1.0);
    r.hem = t;
    r.tail = osc(t, 0.5) * 0.5;
    if blinking(t) {
        r.eyes = Eyes::Closed;
    }
    // breathing
    r.sy = 1.0 + 0.018 * osc(t, 0.4);
    if pet.build == Build::Hover {
        r.dy = 0.7 * osc(t, 0.45);
    }
    if pet.build == Build::Wheels {
        r.sy = 1.0 + 0.01 * osc(t, 1.3); // a running engine
    }
    match clip {
        Clip::Stand => {}
        Clip::Sleep => {
            r.eyes = Eyes::Closed;
            r.crouch = 1.0;
            r.sy = 1.0;
            r.squash(0.95 + 0.04 * osc(t, 0.28));
            r.head = (0.0, 0.6 + 0.4 * osc(t, 0.28));
            r.arms = [4.0; 2];
            r.tail = 0.1 * osc(t, 0.2);
            if pet.build == Build::Hover {
                r.dy = -2.0 + 0.4 * osc(t, 0.2);
            }
        }
        Clip::Cheer => {
            r.eyes = Eyes::Happy;
            r.dy += 3.0 * osc(t, 1.4).abs();
            r.arms = [155.0 + 15.0 * osc(t, 2.8), 155.0 - 15.0 * osc(t, 2.8)];
            r.legs = [(-0.5, 0.0), (0.5, 0.0)];
            r.mouth = 0.7;
            r.tail = osc(t, 2.5);
        }
        Clip::Love => {
            r.eyes = Eyes::Happy;
            r.rot = 5.0 * osc(t, 0.9);
            r.squash(0.97 + 0.02 * osc(t, 1.8));
            r.tail = osc(t, 2.0);
            r.arms = [25.0, 25.0];
        }
        Clip::Stretch => {
            r.eyes = Eyes::Closed;
            r.squash(1.08 + 0.03 * osc(t, 0.7));
            r.arms = [165.0, 165.0];
            r.mouth = 0.6;
        }
        Clip::Tired => {
            r.eyes = Eyes::Half;
            r.squash(0.95);
            r.head = (0.0, 0.6 + 0.4 * osc(t, 0.3));
            r.shear = 0.04 * osc(t, 0.2);
        }
        Clip::Sad => {
            r.eyes = Eyes::Half;
            r.squash(0.96);
            r.head = (0.0, 0.8);
            r.look = -0.6;
            r.tail = 0.0;
        }
        Clip::Walk | Clip::Run => {
            let run = clip == Clip::Run;
            let hz = if run { 3.4 } else { 1.9 };
            let (s, c) = ((t * hz * TAU).sin(), (t * hz * TAU).cos());
            let step = if run { 2.2 } else { 1.5 };
            r.legs = [(s * step, (c * 1.2).max(0.0)), (-s * step, (-c * 1.2).max(0.0))];
            r.dy = if pet.build == Build::Legs { s.abs() * if run { 1.3 } else { 0.6 } } else { r.dy };
            r.shear = if run { 0.16 } else { 0.05 };
            r.rot = if pet.build == Build::Legs { c * if run { 3.0 } else { 2.5 } } else { 0.0 };
            let sw = if run { 60.0 } else { 32.0 };
            r.arms = [10.0 - s * sw, 10.0 + s * sw];
            r.look = 0.8;
            r.turn = (t / 0.16).min(1.0);
            r.tail = osc(t, hz);
            r.wheel = t * if run { 1400.0 } else { 620.0 };
            if pet.build == Build::Wheels {
                r.dy = 0.18 * osc(t, 7.0).abs();
                r.shear = 0.0;
                r.rot = p.tilt + if run { -3.0 } else { 0.0 };
            }
            if pet.build == Build::Hover {
                r.rot = if run { 14.0 } else { 7.0 };
                r.shear = 0.0;
            }
        }
        Clip::Pull => {
            let (px, py) = p.pull;
            r.eyes = Eyes::Wide;
            r.sy = 1.0;
            r.squash((1.0 + py).clamp(0.55, 1.6));
            r.shear = px.clamp(-0.8, 0.8);
            r.mouth = (px.abs() + py.abs()).min(0.8);
            r.arms = [60.0 + 90.0 * py.max(0.0), 60.0 + 90.0 * py.max(0.0)];
            r.look = px.signum() * 0.8;
        }
        Clip::Held => {
            r.eyes = Eyes::Wide;
            r.pivot = 1.0;
            r.rot = p.swing;
            r.squash(1.07);
            let kick = osc(t, 2.2);
            r.legs = [(-1.0 + kick, 0.6 + 0.6 * kick), (1.0 + kick, 0.6 - 0.6 * kick)];
            r.arms = [125.0 + 25.0 * osc(t, 2.6), 125.0 - 25.0 * osc(t, 2.6)];
            r.mouth = 0.4;
            r.tail = osc(t, 1.6);
            r.wheel = t * 200.0;
        }
        Clip::Fall => {
            r.eyes = Eyes::Wide;
            r.pivot = 0.5;
            r.rot = p.spin;
            r.squash(1.06);
            r.arms = [150.0 + 20.0 * osc(t, 5.0), 150.0 - 20.0 * osc(t, 5.0)];
            r.legs = [(-1.2, 0.5 + 0.5 * osc(t, 4.0)), (1.2, 0.5 - 0.5 * osc(t, 4.0))];
            r.mouth = 1.0;
            r.wheel = t * 900.0;
        }
        Clip::Land => {
            r.squash(1.0 - 0.38 * (1.0 - k) * (k * TAU * 1.2).cos().abs());
            r.eyes = Eyes::Closed;
            r.arms = [70.0 * (1.0 - k); 2];
            r.crouch = 0.8 * (1.0 - k);
        }
        Clip::Boing => {
            let w = (1.0 - k) * (1.0 - k);
            r.sy = 1.0;
            r.squash(1.0 - 0.3 * w * (t * 22.0).cos());
            r.shear = 0.45 * w * p.hit * (t * 24.0).cos();
            r.eyes = if k < 0.6 { Eyes::Wide } else { Eyes::Open };
            r.tail = osc(t, 4.0);
        }
        Clip::Flinch => {
            let w = 1.0 - k;
            r.eyes = Eyes::Closed;
            r.dx = p.hit * 2.2 * w * w * (1.0 + 0.4 * (t * 60.0).sin());
            r.rot = p.hit * 26.0 * w * w;
            r.sy = 1.0;
            r.squash(1.0 - 0.18 * w);
            r.arms = [80.0 * w; 2];
            r.mouth = w;
            r.red = 0.55 * (1.0 - (t / 0.18)).max(0.0);
        }
        Clip::Dizzy => {
            r.eyes = Eyes::Dizzy;
            r.rot = 9.0 * osc(t, 0.8) * (1.0 - 0.5 * k);
            r.dx = 0.8 * osc(t, 0.8);
            r.head = (0.7 * osc(t, 0.8 + 0.1), 0.3);
            r.arms = [35.0 + 20.0 * osc(t, 1.1), 35.0 - 20.0 * osc(t, 1.1)];
            r.mouth = 0.5;
        }
        Clip::Angry => {
            r.eyes = Eyes::Angry;
            r.red = 0.45 * ease(t, len, 0.25);
            r.dx = 0.35 * osc(t, 11.0);
            r.dy += 0.8 * osc(t, 3.0).abs();
            r.arms = [60.0, 60.0];
            r.squash(1.04);
            r.mouth = 0.4;
        }
        Clip::Eat => {
            let gone = (k * 1.12 - 0.08).clamp(0.0, 1.0);
            r.prop = Prop::Food(p.food, (gone * 4.0).floor() / 4.0);
            r.arms_front = true;
            r.arms = [95.0, 95.0];
            let chew = osc(t, 3.0);
            r.mouth = if chew > 0.0 { 0.9 } else { 0.1 };
            r.eyes = if k > 0.25 { Eyes::Happy } else { Eyes::Open };
            r.head = (0.0, 0.3 * chew.max(0.0));
            r.sy = 1.0;
            r.squash(1.0 + 0.03 * chew);
            r.tail = osc(t, 2.0);
            if pet.machine() {
                r.dx = 0.15 * osc(t, 9.0); // gulping it down
            }
        }
        Clip::Look => {
            r.look = if k < 0.4 { -1.0 } else if k < 0.8 { 1.0 } else { 0.0 };
            r.head = (r.look * 0.6, 0.0);
            r.shear = r.look * 0.03;
        }
        Clip::Wave => {
            let e = ease(t, len, 0.25);
            r.arms[1] = 10.0 + (140.0 + 22.0 * osc(t, 2.6)) * e;
            r.eyes = Eyes::Happy;
            r.rot = -3.0 * e;
            r.mouth = 0.6 * e;
        }
        Clip::Dance => {
            let beat = osc(t, 1.1);
            r.dx = 1.4 * beat;
            r.rot = 7.0 * beat;
            r.dy += 1.0 * osc(t, 2.2).abs();
            r.arms = [90.0 + 60.0 * beat, 90.0 - 60.0 * beat];
            r.legs = [(0.0, (1.2 * beat).max(0.0)), (0.0, (-1.2 * beat).max(0.0))];
            r.eyes = Eyes::Happy;
            r.tail = osc(t, 2.2);
            r.wheel = 60.0 * beat;
            r.look = beat;
        }
        Clip::Hop => {
            let h = arc(t, 0.2, 0.45) + 0.7 * arc(t, 0.75, 0.4);
            r.dy += 5.0 * h;
            r.squash(if h > 0.05 { 1.06 } else if (0.1..1.25).contains(&t) { 0.88 } else { 1.0 });
            r.arms = [20.0 + 90.0 * h; 2];
            r.legs = [(-0.6 * h, 0.4 * h), (0.6 * h, 0.4 * h)];
        }
        Clip::Yawn => {
            let e = ease(t, len, 0.5);
            r.eyes = if e > 0.4 { Eyes::Closed } else { Eyes::Half };
            r.mouth = e;
            r.squash(1.0 + 0.05 * e);
            r.arms = [10.0 + 130.0 * e, 10.0 + 60.0 * e];
            r.head = (0.0, -0.4 * e);
        }
        Clip::Sneeze => {
            if k < 0.5 {
                r.eyes = Eyes::Half;
                r.head = (-0.3, -1.0 * k * 2.0);
                r.mouth = k;
                r.rot = -7.0 * k * 2.0;
            } else {
                let w = (1.0 - k) * 2.0;
                r.eyes = Eyes::Closed;
                r.rot = 14.0 * w * w;
                r.dx = 0.4 * osc(t, 12.0) * w;
                r.squash(1.0 - 0.14 * w);
                r.dy += 1.0 * arc(t, 0.8, 0.2);
            }
        }
        Clip::Sit => {
            let e = ease(t, len, 0.3);
            r.crouch = e;
            r.legs = [(-1.6 * e, 0.0), (1.6 * e, 0.0)];
            r.look = 0.8 * osc(t, 0.2);
            r.head = (0.0, 0.3 * osc(t, 0.6) * e);
        }
        Clip::Spin => {
            // turns on the spot like a card seen from the side
            let a = (k * TAU * 3.0).cos();
            r.sx = if a.abs() < 0.12 { 0.12 * a.signum() } else { a };
            r.dy += 0.6 * osc(t, 3.0).abs();
            r.arms = [70.0; 2];
            r.tail = 1.0;
        }
        Clip::Hungry => {
            r.eyes = if osc(t, 0.5) > -0.5 { Eyes::Half } else { Eyes::Closed };
            r.arms_front = true;
            r.arms = [35.0 + 12.0 * osc(t, 1.6), 35.0 - 12.0 * osc(t, 1.6)];
            r.squash(0.96);
            r.look = 0.6;
            if pet.machine() {
                r.dx = if (t * 3.0).fract() < 0.3 { 0.3 * osc(t, 14.0) } else { 0.0 }; // sputter
            }
        }
        Clip::Groom => {
            r.arms_front = true;
            r.arms = [10.0, 120.0 + 18.0 * osc(t, 2.4)];
            r.eyes = Eyes::Closed;
            r.head = (0.3 * osc(t, 2.4), 0.2);
            r.rot = 3.0;
        }
        Clip::Wag => {
            r.tail = osc(t, 4.5);
            r.rot = 4.0 * osc(t, 4.5);
            r.eyes = Eyes::Happy;
            r.mouth = 0.5 + 0.3 * osc(t, 5.0);
            r.dy += 0.8 * osc(t, 2.2).abs();
        }
        Clip::Pounce => {
            if t < 0.6 {
                r.squash(0.82);
                r.crouch = 0.8;
                r.dx = 0.4 * osc(t, 5.0); // wiggle
                r.eyes = Eyes::Wide;
                r.look = 1.0;
                r.tail = osc(t, 3.0);
            } else {
                r.dy += 4.0 * arc(t, 0.6, 0.35);
                r.rot = 10.0 * ease(t, len, 0.3);
                r.arms = [100.0 * (1.0 - k), 100.0 * (1.0 - k)];
            }
        }
        Clip::Jiggle => {
            let w = 1.0 - k;
            r.sy = 1.0;
            r.squash(1.0 + 0.32 * w * osc(t, 3.2));
            r.shear = 0.12 * w * osc(t, 1.6);
            r.eyes = Eyes::Happy;
        }
        Clip::Phone | Clip::Sip => {
            let e = ease(t, len, 0.35);
            r.arms_front = true;
            let up = clip == Clip::Sip && (t * 0.8).fract() > 0.45;
            r.arms = [10.0, 10.0 + e * if up { 125.0 } else { 95.0 }];
            r.prop = if e < 0.6 { Prop::None } else if clip == Clip::Phone { Prop::Phone } else { Prop::Cup };
            r.head = if up { (0.0, -0.5) } else { (0.0, 0.4 * e) };
            r.eyes = if up { Eyes::Closed } else { Eyes::Half };
            r.look = 0.8;
        }
        Clip::Backflip => {
            let h = arc(t, 0.25, 0.75);
            r.squash(if t < 0.25 { 0.8 } else if t > 1.0 { 0.9 } else { 1.0 });
            r.dy += 9.0 * h;
            r.pivot = 0.5;
            r.rot = -360.0 * ((t - 0.25) / 0.75).clamp(0.0, 1.0);
            r.arms = [40.0 + 110.0 * h; 2];
            r.crouch = 0.7 * h;
        }
        Clip::Rev => {
            r.dx = 0.22 * osc(t, 16.0);
            r.dy = 0.25 * osc(t, 11.0).abs();
            r.rot = -2.5 * ease(t, len, 0.3);
            r.wheel = t * 2200.0;
            r.eyes = Eyes::Angry;
        }
        Clip::Drift => {
            r.wheel = t * 2000.0;
            r.rot = -6.0 + 3.0 * osc(t, 2.5);
            r.shear = 0.14;
            r.dx = 0.3 * osc(t, 8.0);
            r.eyes = Eyes::Angry;
        }
        Clip::Honk => {
            let beat = (t * 2.5).fract();
            r.squash(1.0 - 0.14 * (1.0 - beat));
            r.eyes = Eyes::Wide;
        }
        Clip::Glitch => {
            r.glitch = if (t * 3.0).fract() < 0.6 { 1 + (t * 14.0) as u32 } else { 0 };
            r.eyes = if (t * 5.0).fract() < 0.4 { Eyes::Dizzy } else { Eyes::Wide };
            r.dx = if r.glitch > 0 { 0.5 * osc(t, 9.0) } else { 0.0 };
            r.arms = [10.0 + 100.0 * (t * 4.0).fract(), 10.0];
        }
        Clip::Slide => {
            let e = ease(t, len, 0.35);
            r.rot = 78.0 * e;
            r.pivot = 0.35;
            r.turn = e;
            r.dy += -3.2 * e;
            r.arms = [10.0 + 60.0 * e; 2];
            r.eyes = Eyes::Happy;
        }
        Clip::Roar => {
            let e = ease(t, len, 0.4);
            r.mouth = 1.3 * e;
            r.rot = -8.0 * e;
            r.squash(1.0 + 0.06 * e);
            r.dx = 0.35 * osc(t, 13.0) * e;
            r.arms = [10.0 + 70.0 * e; 2];
            r.eyes = Eyes::Angry;
            r.tail = osc(t, 2.0);
        }
        Clip::Spook => {
            let e = ease(t, len, 0.5);
            r.alpha = 1.0 - 0.75 * e * (0.5 + 0.5 * osc(t, 0.8).abs());
            r.squash(1.0 + 0.16 * e);
            r.arms = [10.0 + 130.0 * e; 2];
            r.mouth = e;
            r.eyes = Eyes::Wide;
            r.dy += 2.0 * e;
        }
        Clip::Loop => {
            let a = k * TAU;
            r.dx = 4.0 * a.sin();
            r.dy += 4.0 * (1.0 - a.cos());
            r.rot = -360.0 * k;
            r.pivot = 0.5;
            r.eyes = Eyes::Happy;
            r.arms = [90.0; 2];
        }
        Clip::Vanish => {
            r.alpha = if (0.35..1.25).contains(&t) { 0.0 } else { 1.0 };
            if t < 0.35 {
                r.squash(1.0 - 0.5 * t);
                r.arms_front = true;
                r.arms = [95.0, 95.0];
                r.eyes = Eyes::Closed;
            }
        }
        Clip::Float => {
            let e = ease(t, len, 0.9);
            r.dy += 7.0 * e + 0.6 * osc(t, 0.6);
            r.rot = 14.0 * osc(t, 0.35) * e;
            r.pivot = 0.5;
            r.arms = [10.0 + 75.0 * e, 10.0 + 65.0 * e];
            r.legs = [(-1.0 * e, 0.4 * e), (1.0 * e, 0.0)];
            r.eyes = Eyes::Happy;
        }
        Clip::Hops => {
            let f = (t * 2.0).fract();
            let h = if f < 0.7 { 4.0 * (f / 0.7) * (1.0 - f / 0.7) } else { 0.0 };
            r.dy += 4.0 * h;
            r.squash(if f < 0.7 { 1.0 + 0.08 * h } else { 0.82 });
            r.arms = [20.0 + 50.0 * h; 2];
            r.look = 0.8;
            r.turn = 1.0f32.min(t / 0.16).min((len - t) / 0.16).max(0.0);
        }
        Clip::Tongue => {
            let out = arc(t, 0.5, 0.5);
            r.look = 1.0;
            r.prop = Prop::Tongue(out);
            r.mouth = if out > 0.02 { 0.8 } else { 0.0 };
            r.squash(1.0 - 0.06 * out);
            r.eyes = if t > 1.05 { Eyes::Happy } else { Eyes::Wide };
        }
        Clip::Beam => {
            r.dy += 1.5;
            r.prop = Prop::Beam(ease(t, len, 0.4));
            r.eyes = Eyes::Happy;
        }
        Clip::Groove => {
            // knees and hips, with a bounce on the beat
            let sway = osc(t, 1.0);
            r.squash(1.0 - 0.13 * p.beat);
            r.dy += 1.1 * (1.0 - p.beat) * osc(t, 2.0).abs();
            r.rot = 6.0 * sway;
            r.dx = 1.0 * sway;
            r.arms = [70.0 + 55.0 * sway, 70.0 - 55.0 * sway];
            r.legs = [(-0.4, (0.9 * sway).max(0.0)), (0.4, (-0.9 * sway).max(0.0))];
            r.head = (0.6 * sway, 0.4 * p.beat);
            r.eyes = if k < 0.5 { Eyes::Happy } else { Eyes::Open };
            r.look = sway;
            r.tail = osc(t, 2.0);
            if pet.build == Build::Wheels {
                r.rot = 5.0 * sway; // a lowrider
                r.dy = 1.6 * p.beat;
                r.squash(1.0 + 0.06 * p.beat);
            }
        }
        Clip::Headbang => {
            r.head = (0.0, -0.8 + 2.0 * p.beat.max(0.5 + 0.5 * osc(t, 2.0)));
            r.rot = 5.0 * osc(t, 2.0);
            r.shear = 0.05 * osc(t, 2.0);
            r.eyes = Eyes::Closed;
            r.arms = [125.0 + 14.0 * osc(t, 2.0), 125.0 - 14.0 * osc(t, 2.0)];
            r.legs = [(-0.8, 0.0), (0.8, 0.0)];
            r.mouth = 0.5;
            r.tail = osc(t, 2.0);
        }
        Clip::Disco => {
            // point up, point down, swap
            let left = (t * 1.0).fract() < 0.5;
            let up = 150.0 + 10.0 * p.beat;
            r.arms = if left { [up, 25.0] } else { [25.0, up] };
            r.rot = if left { -7.0 } else { 7.0 };
            r.dx = if left { -0.8 } else { 0.8 };
            r.dy += 0.9 * p.beat;
            r.look = if left { -1.0 } else { 1.0 };
            r.legs = if left { [(-0.9, 0.0), (0.0, 0.6 * p.beat)] } else { [(0.0, 0.6 * p.beat), (0.9, 0.0)] };
            r.eyes = Eyes::Happy;
            r.mouth = 0.5;
            r.tail = osc(t, 1.0);
        }
        Clip::Climb => {
            let s = osc(t, 1.5);
            r.arms = [140.0 + 28.0 * s, 140.0 - 28.0 * s];
            r.legs = [(-0.2, 0.5 + 0.5 * s), (0.2, 0.5 - 0.5 * s)];
            r.rot = 2.5 * s;
            r.look = 0.0;
        }
        Clip::Glide => {
            r.rot = 7.0 * osc(t, 0.35);
            r.pivot = 1.0;
            r.arms = [35.0 + 10.0 * osc(t, 0.8), 172.0];
            r.legs = [(-0.8 + 0.3 * osc(t, 0.9), 0.3), (0.6, 0.2)];
            r.prop = Prop::Umbrella;
            r.eyes = Eyes::Happy;
        }
        Clip::Wheelie => {
            let e = ease(t, len, 0.4);
            r.rot = -24.0 * e;
            r.dx = -1.0 * e;
            r.dy = 2.2 * e;
            r.wheel = t * 1200.0;
            r.eyes = Eyes::Happy;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const ALL: &[Clip] = &[
        Clip::Stand, Clip::Sleep, Clip::Cheer, Clip::Love, Clip::Stretch, Clip::Tired, Clip::Sad, Clip::Walk, Clip::Run, Clip::Pull, Clip::Held,
        Clip::Fall, Clip::Land, Clip::Boing, Clip::Flinch, Clip::Dizzy, Clip::Angry, Clip::Eat, Clip::Look, Clip::Wave, Clip::Dance, Clip::Hop,
        Clip::Yawn, Clip::Sneeze, Clip::Sit, Clip::Spin, Clip::Hungry, Clip::Groom, Clip::Wag, Clip::Pounce, Clip::Jiggle, Clip::Phone, Clip::Sip,
        Clip::Backflip, Clip::Rev, Clip::Drift, Clip::Honk, Clip::Glitch, Clip::Slide, Clip::Roar, Clip::Spook, Clip::Loop, Clip::Vanish,
        Clip::Float, Clip::Hops, Clip::Tongue, Clip::Beam, Clip::Wheelie, Clip::Groove, Clip::Headbang, Clip::Disco, Clip::Climb, Clip::Glide,
    ];

    fn each(mut f: impl FnMut(&Pet, Clip, f32, Rig)) {
        for pet in PETS.iter() {
            for &c in ALL {
                let len = c.secs().unwrap_or(6.0);
                let mut t = 0.0;
                while t <= len {
                    f(pet, c, t, sample(pet, c, t, &Push::NONE));
                    t += 0.031;
                }
            }
        }
    }

    #[test]
    fn every_pose_of_every_act_is_sane() {
        each(|pet, c, t, r| {
            let what = format!("{} {c:?} at {t:.2}", pet.name);
            assert!(r.sy.is_finite() && r.sy > 0.4 && r.sy < 1.8, "{what}: height {}", r.sy);
            assert!(r.sx.is_finite() && r.sx.abs() > 0.05 && r.sx.abs() < 1.8, "{what}: width {}", r.sx);
            assert!((0.0..=1.0).contains(&r.alpha) && (0.0..=1.0).contains(&r.red), "{what}");
            assert!((0.0..=1.0).contains(&r.crouch), "{what}: crouch {}", r.crouch);
            assert!(r.dx.abs() <= 6.0 && (-4.0..=13.0).contains(&r.dy), "{what}: moved to {},{}", r.dx, r.dy);
            assert!(r.look.abs() <= 1.0 && r.tail.abs() <= 1.01 && (0.0..=1.0).contains(&r.turn), "{what}");
        });
    }

    #[test]
    fn acts_start_and_end_close_to_standing_so_nothing_snaps() {
        for pet in PETS.iter() {
            for &c in COMMON.iter().chain(COMMON_MACHINE).chain(pet.tricks) {
                let len = c.secs().expect("an act must end by itself");
                let stand = sample(pet, Clip::Stand, len, &Push::NONE);
                let end = sample(pet, c, len, &Push::NONE);
                let turn = end.rot.rem_euclid(360.0);
                assert!(turn < 8.0 || turn > 352.0, "{} {c:?} ends turned by {turn}", pet.name);
                assert!((end.dy - stand.dy).abs() < 2.5, "{} {c:?} ends at height {}", pet.name, end.dy);
                assert_eq!(end.alpha, 1.0, "{} {c:?} ends see-through", pet.name);
            }
        }
    }

    #[test]
    fn legs_alternate_while_walking() {
        let cat = &PETS[0];
        let mut lead = [false; 2];
        for i in 0..40 {
            let r = sample(cat, Clip::Walk, i as f32 * 0.025, &Push::NONE);
            assert!(r.legs[0].1 == 0.0 || r.legs[1].1 == 0.0, "both feet off the ground while walking");
            lead[(r.legs[0].0 > r.legs[1].0) as usize] = true;
        }
        assert!(lead[0] && lead[1]);
    }

    #[test]
    fn sleeping_pets_have_their_eyes_shut_and_lie_low() {
        for i in 0..200 {
            let r = sample(&PETS[0], Clip::Sleep, i as f32 * 0.05, &Push::NONE);
            assert!(r.eyes == Eyes::Closed && r.crouch == 1.0);
        }
    }
}
