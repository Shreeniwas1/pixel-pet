//! Powers: how somebody moves while using one, and the beams, shields, threads and portals drawn
//! around them. Packs (pack.rs) use the same effects for moves of their own. Like `anim.rs` this is plain maths and memory.

use crate::prelude::*;
use std::f32::consts::{PI, TAU};

/// What is drawn around somebody using a power. Amounts run 0..1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Fx {
    None,
    Ray(f32),        // from the front hand
    EyeRay(f32),     // from the head
    Rays(f32),       // from both hands
    Ring(f32),       // a wave running outwards
    Portal(f32),     // a ring of sparks behind
    Bolt(f32),       // lightning into the raised hand
    Orbs(f32, f32),  // amount, time
    Sigil(f32, f32),
    Thrown(f32),     // how far the thrown disc is out
    Arrow(f32, f32), // string drawn, arrow flown
    Thread,          // a line to the ceiling
    Hang,            // a line from the feet
    Slash(f32),
    Cut(f32),        // one sweep of a blade
    Claws,           // out, but not slashing
    Fist(f32),
    Hoops(f32, f32), // amount out, time
    Spikes(f32),
    Moon(f32),
    Flash(f32),      // muzzle fire
    Jets(f32),       // flames from the feet
    Lights(f32),     // little lights round the hand
    Heart(f32),
    Radar(f32, f32),
    Flowers(f32),
    Dome(f32),          // a force field
    Fire(f32, f32),
    Board(f32),
    Limbs(f32, f32),    // four extra arms
    Bomb(f32),
    Cards(f32),
    FireWings(f32, f32),
    Scrap(f32, f32),    // metal going round
}

impl Fx {
    /// Changes whenever the picture does.
    pub fn key(self) -> f32 {
        let d = |n: f32, v: f32| n * 1000.0 + v * 40.0;
        match self {
            Fx::None => 0.0,
            Fx::Ray(k) => d(1.0, k),
            Fx::EyeRay(k) => d(2.0, k),
            Fx::Rays(k) => d(3.0, k),
            Fx::Ring(k) => d(4.0, k),
            Fx::Portal(k) => d(5.0, k),
            Fx::Bolt(k) => d(6.0, k),
            Fx::Orbs(k, t) => d(7.0, k + t * 3.0),
            Fx::Sigil(k, t) => d(8.0, k + t * 3.0),
            Fx::Thrown(k) => d(9.0, k),
            Fx::Arrow(a, b) => d(10.0, a + b * 2.0),
            Fx::Thread => d(11.0, 0.0),
            Fx::Hang => d(12.0, 0.0),
            Fx::Slash(k) => d(14.0, k),
            Fx::Claws => d(15.0, 0.0),
            Fx::Fist(k) => d(16.0, k),
            Fx::Hoops(k, t) => d(17.0, k + t * 3.0),
            Fx::Spikes(k) => d(18.0, k),
            Fx::Moon(k) => d(19.0, k),
            Fx::Flash(k) => d(20.0, k),
            Fx::Jets(k) => d(21.0, k),
            Fx::Lights(k) => d(22.0, k),
            Fx::Heart(k) => d(23.0, k),
            Fx::Radar(k, t) => d(24.0, k + t * 3.0),
            Fx::Flowers(k) => d(25.0, k),
            Fx::Cut(k) => d(26.0, k),
            Fx::Dome(k) => d(27.0, k),
            Fx::Fire(k, t) => d(28.0, k + t * 4.0),
            Fx::Board(k) => d(29.0, k),
            Fx::Limbs(k, t) => d(30.0, k + t * 3.0),
            Fx::Bomb(k) => d(31.0, k),
            Fx::Cards(k) => d(32.0, k),
            Fx::FireWings(k, t) => d(33.0, k + t * 4.0),
            Fx::Scrap(k, t) => d(34.0, k + t * 3.0),
        }
    }
}

/// What keeps somebody up in the air.
pub(crate) fn flight(pet: &Pet, k: f32, t: f32) -> Fx {
    match pet.gear.aloft {
        Aloft::Jets => Fx::Jets(k), // wings and capes need none, and draw none
        Aloft::Board => Fx::Board(k),
        Aloft::Fire => Fx::Fire(k, t),
        Aloft::FireWings => Fx::FireWings(k, t),
        Aloft::Orbs => Fx::Orbs(k, t),
        Aloft::Ring => Fx::Ring((t * 0.9).fract()),
    }
}

/// Seconds of each power, or None if this is not one.
pub(crate) fn secs(c: Clip) -> Option<f32> {
    Some(match c {
        Clip::Own(n) => pack::own(n).secs,
        Clip::Blast | Clip::Flare | Clip::Portal | Clip::Fade | Clip::Reach => 2.6,
        Clip::Soar | Clip::Surf => 3.2,
        Clip::Kneel | Clip::Echo => 2.4,
        Clip::Zap | Clip::Pulse | Clip::BigFist => 1.8,
        Clip::Punch => 1.2,
        Clip::Stomp | Clip::Line | Clip::Slice | Clip::Dash => 1.6,
        Clip::Swing | Clip::Shrink | Clip::Giant | Clip::Bloom | Clip::Lull | Clip::Doubles => 3.0,
        Clip::Hang => 4.0,
        Clip::Levitate => 3.6,
        Clip::Spikes | Clip::Lob => 2.2,
        Clip::Moonrise | Clip::Flick => 2.0,
        Clip::Storm | Clip::Bubble | Clip::Limbs | Clip::Blaze | Clip::Orbit => 2.8,
        _ => return None,
    })
}

/// Cells per second it travels by itself.
pub(crate) fn travel(c: Clip, t: f32) -> f32 {
    match c {
        Clip::Own(n) => pack::own(n).travel(t),
        Clip::Soar if (0.5..2.7).contains(&t) => 30.0,
        Clip::Swing if (0.3..2.7).contains(&t) => 22.0,
        Clip::Fade if (0.6..2.0).contains(&t) => 9.0,
        Clip::Shrink if (0.8..2.2).contains(&t) => 14.0,
        Clip::Surf if (0.5..2.7).contains(&t) => 28.0,
        Clip::Blaze if (0.8..2.2).contains(&t) => 14.0,
        Clip::Dash if (0.25..1.25).contains(&t) => 120.0,
        Clip::Stomp if (0.15..0.6).contains(&t) => 8.0,
        _ => 0.0,
    }
}

fn osc(t: f32, hz: f32) -> f32 {
    (t * hz * TAU).sin()
}

fn ease(t: f32, len: f32, e: f32) -> f32 {
    let k = (t / e).min((len - t) / e).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn ramp(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

/// Put the body into the pose of a power at second `t`.
pub(crate) fn pose(r: &mut Rig, c: Clip, t: f32) {
    let len = secs(c).unwrap_or(1.0);
    let k = (t / len).clamp(0.0, 1.0);
    match c {
        Clip::Own(n) => pack::own(n).pose(r, t),
        Clip::Blast => {
            // hovering on its jets, one palm forward
            let e = ease(t, len, 0.4);
            r.dy += 3.0 * e + 0.4 * osc(t, 1.3) * e;
            r.rot = 5.0 * e;
            r.arms = [35.0 * e, 10.0 + 82.0 * e];
            r.look = e;
            r.eyes = Eyes::Angry;
            let fire = ((t * 1.6).fract() > 0.35) && (0.6..2.1).contains(&t);
            r.fx = if fire { Fx::Ray(0.5 + 0.5 * osc(t, 9.0).abs()) } else { Fx::Jets(e) };
        }
        Clip::Soar => {
            let e = ease(t, len, 0.5);
            r.dy += 7.0 * e + 0.5 * osc(t, 0.9) * e;
            r.rot = 62.0 * e;
            r.pivot = 0.5;
            r.arms = [10.0 + 8.0 * e, 10.0 + 150.0 * e];
            r.turn = e;
            r.wing = e;
            r.legs = [(-0.3 * e, 0.2), (0.3 * e, 0.0)];
            r.look = e;
            r.fx = Fx::Jets(e);
        }
        Clip::Flare => {
            let e = ease(t, len, 0.5);
            r.dy += 4.0 * e + 0.4 * osc(t, 1.1) * e;
            r.arms = [10.0 + 80.0 * e, 10.0 + 80.0 * e];
            r.aura = e;
            r.eyes = Eyes::Angry;
            r.squash(1.0 + 0.05 * e);
            r.fx = if (0.7..2.1).contains(&t) { Fx::Rays(0.6 + 0.4 * osc(t, 8.0).abs()) } else { Fx::None };
        }
        Clip::Punch => {
            let wind = ease(t, 0.5, 0.2);
            let hit = if t > 0.4 { (1.0 - ramp(t, 0.4, 1.0)).powi(2) } else { 0.0 };
            r.arms = [30.0, 92.0];
            r.dx = -1.2 * wind * (t < 0.4) as i32 as f32 + 3.0 * hit;
            r.rot = -8.0 * wind * (t < 0.4) as i32 as f32 + 14.0 * hit;
            r.reach = 1.0 + 0.5 * hit;
            r.legs = [(-1.2 * hit, 0.0), (1.4 * hit, 0.0)];
            r.eyes = Eyes::Angry;
            r.fx = if hit > 0.3 { Fx::Ring(1.0 - hit) } else { Fx::None };
        }
        Clip::Storm => {
            let e = ease(t, len, 0.4);
            r.arms = [20.0, 10.0 + 168.0 * e];
            r.head = (0.0, -0.4 * e);
            r.mouth = 0.6 * e;
            r.eyes = Eyes::Angry;
            let strike = (0.7..2.2).contains(&t) && (t * 3.1).fract() < 0.6;
            r.aura = if strike { 0.9 } else { 0.25 * e };
            r.dx = if strike { 0.3 * osc(t, 15.0) } else { 0.0 };
            r.fx = if strike { Fx::Bolt((t * 7.0).fract()) } else { Fx::None };
        }
        Clip::Stomp => {
            let up = if (0.15..0.7).contains(&t) { ((t - 0.15) / 0.55 * PI).sin() } else { 0.0 };
            let boom = if t > 0.7 { (1.0 - ramp(t, 0.7, 1.5)).powi(2) } else { 0.0 };
            r.dy += 6.0 * up;
            r.arms = if t < 0.7 { [150.0, 150.0] } else { [25.0 + 10.0 * boom, 25.0 + 10.0 * boom] };
            r.squash(1.0 - 0.28 * boom);
            r.dx = 0.5 * boom * osc(t, 13.0);
            r.mouth = 0.9 * (up + boom).min(1.0);
            r.eyes = Eyes::Angry;
            r.fx = if boom > 0.02 { Fx::Ring(1.0 - boom) } else { Fx::None };
        }
        Clip::Zap => {
            let e = ease(t, len, 0.25);
            r.arms = [15.0 + 60.0 * e, 10.0 + 85.0 * e];
            r.legs = [(-1.0 * e, 0.0), (1.2 * e, 0.0)];
            r.rot = 6.0 * e;
            r.eyes = Eyes::Angry;
            r.aura = if (t * 8.0).fract() < 0.5 { 0.6 * e } else { 0.0 };
            r.fx = if (0.4..1.4).contains(&t) { Fx::Ray(0.35 + 0.4 * osc(t, 11.0).abs()) } else { Fx::None };
        }
        Clip::Swing => {
            let e = ease(t, len, 0.35);
            let s = osc(t, 0.62);
            r.dy += 6.0 * e + 2.0 * s * s * e;
            r.dx = 2.0 * s * e;
            r.pivot = 1.0;
            r.rot = -28.0 * s * e;
            r.arms = [40.0 * e, 10.0 + 166.0 * e];
            r.legs = [(-1.0 + s, 0.5), (1.0 + s, 0.3)];
            r.look = e;
            r.fx = if e > 0.25 { Fx::Thread } else { Fx::None };
        }
        Clip::Hang => {
            let e = ease(t, len, 0.55);
            r.pivot = 0.5;
            r.rot = 180.0 * e;
            r.dy += 6.5 * e + 0.5 * osc(t, 0.4) * e;
            r.arms = [14.0 + 10.0 * osc(t, 0.5), 14.0 - 10.0 * osc(t, 0.5)];
            r.legs = [(0.6 * e, 0.0), (-0.6 * e, 0.0)];
            r.look = osc(t, 0.3);
            r.fx = if e > 0.6 { Fx::Hang } else { Fx::None };
        }
        Clip::Line => {
            let e = ease(t, len, 0.25);
            r.arms = [20.0, 10.0 + 80.0 * e];
            r.legs = [(-1.0 * e, 0.0), (1.0 * e, 0.0)];
            r.crouch = 0.3 * e;
            r.look = 1.0;
            r.arms[1] += 12.0 * e;
        }
        Clip::Portal => {
            let open = ramp(t, 0.2, 0.8).min(1.0 - ramp(t, 2.0, 2.5));
            r.arms = [10.0 + 60.0 * open, 10.0 + 90.0 * open + 25.0 * osc(t, 1.6)];
            let through = (1.1..1.5).contains(&t);
            r.alpha = if through { 0.0 } else { 1.0 };
            r.squash(1.0 - 0.25 * ramp(t, 0.9, 1.1) * (t < 1.3) as i32 as f32);
            r.look = 1.0;
            r.fx = if open > 0.02 { Fx::Portal(open) } else { Fx::None };
        }
        Clip::Levitate => {
            let e = ease(t, len, 0.7);
            r.dy += 6.0 * e + 0.7 * osc(t, 0.5) * e;
            r.arms = [10.0 + 50.0 * e + 8.0 * osc(t, 0.7), 10.0 + 50.0 * e - 8.0 * osc(t, 0.7)];
            r.legs = [(-0.3 * e, 0.0), (0.2 * e, 0.4 * e)];
            r.aura = 0.45 * e;
            r.eyes = if e > 0.5 { Eyes::Half } else { Eyes::Open };
            r.fx = Fx::Orbs(e, t);
        }
        Clip::Pulse => {
            let gather = ramp(t, 0.0, 0.8);
            let burst = ramp(t, 0.8, 1.5);
            r.crouch = 0.6 * gather * (t < 0.8) as i32 as f32;
            r.arms = if t < 0.8 { [20.0, 20.0] } else { [110.0, 110.0] };
            r.aura = if t < 0.8 { gather } else { 1.0 - burst };
            r.squash(if t < 0.8 { 1.0 - 0.12 * gather } else { 1.0 + 0.1 * (1.0 - burst) });
            r.eyes = Eyes::Angry;
            r.fx = if (0.8..1.5).contains(&t) { Fx::Ring(burst) } else { Fx::None };
        }
        Clip::Fade => {
            let e = ease(t, len, 0.5);
            r.alpha = 1.0 - 0.62 * e;
            r.dy += 2.0 * e + 0.4 * osc(t, 0.6) * e;
            r.arms = [18.0, 18.0];
            r.look = e;
        }
        Clip::Shrink => {
            let s = ramp(t, 0.3, 0.7).min(1.0 - ramp(t, 2.3, 2.7));
            let z = 1.0 - 0.56 * s;
            (r.sx, r.sy) = (r.sx * z, r.sy * z);
            let run = t * 5.0;
            r.legs = if s > 0.9 { [(run.sin() * 1.6, run.cos().max(0.0)), (-run.sin() * 1.6, (-run.cos()).max(0.0))] } else { [(0.0, 0.0); 2] };
            r.arms = [10.0 + 60.0 * s, 10.0 + 60.0 * s];
            r.look = s;
            r.fx = if (0.25..0.8).contains(&t) || (2.25..2.8).contains(&t) { Fx::Ring((t * 2.0).fract()) } else { Fx::None };
        }
        Clip::Giant => {
            let s = ramp(t, 0.3, 0.9).min(1.0 - ramp(t, 2.2, 2.8));
            let z = 1.0 + 0.42 * s;
            (r.sx, r.sy) = (r.sx * z, r.sy * z);
            r.arms = [10.0 + 60.0 * s, 10.0 + 60.0 * s + 15.0 * osc(t, 1.0) * s];
            r.mouth = 0.7 * s;
            r.dx = if s > 0.95 { 0.25 * osc(t, 10.0) } else { 0.0 };
        }
        Clip::Slice => {
            let swing = (t * 1.9).fract();
            let on = (0.2..1.4).contains(&t);
            r.arms = if on { [40.0, 165.0 - 120.0 * swing] } else { [20.0, 70.0] };
            r.dx = if on { 2.0 * swing } else { 0.0 };
            r.rot = if on { -8.0 + 20.0 * swing } else { 0.0 };
            r.legs = [(-1.2, 0.0), (1.2, 0.0)];
            r.eyes = Eyes::Angry;
            r.fx = if on { Fx::Cut(swing) } else { Fx::None };
        }
        Clip::Reach => {
            let e = ease(t, len, 0.6);
            r.arms = [10.0 + 75.0 * e, 10.0 + 80.0 * e];
            r.reach = 1.0 + 1.9 * e;
            r.squash(1.0 + 0.08 * e);
            r.eyes = Eyes::Happy;
        }
        Clip::Bloom => {
            let e = ease(t, len, 0.7);
            r.arms = [10.0 + 120.0 * e, 10.0 + 120.0 * e];
            r.eyes = Eyes::Happy;
            r.rot = 3.0 * osc(t, 0.6) * e;
            r.squash(1.0 + 0.05 * e);
            r.fx = Fx::Flowers(ramp(t, 0.5, 1.4).min(1.0 - ramp(t, 2.6, 3.0)));
        }
        Clip::Lull => {
            let e = ease(t, len, 0.6);
            r.arms = [10.0, 10.0 + 85.0 * e];
            r.eyes = if e > 0.4 { Eyes::Closed } else { Eyes::Half };
            r.aura = 0.4 * e * (0.6 + 0.4 * osc(t, 0.9));
            r.head = (0.0, 0.4 * e);
            r.fx = Fx::Ring((t * 0.7).fract());
        }
        Clip::Doubles => {
            let e = ease(t, len, 0.5);
            r.clones = e;
            r.arms = [20.0 + 30.0 * e, 20.0 + 30.0 * e];
            r.eyes = Eyes::Happy;
            r.alpha = if e > 0.2 { 0.86 } else { 1.0 };
            if e <= 0.02 {
                r.alpha = 1.0;
            }
        }
        Clip::Moonrise => {
            let e = ease(t, len, 0.4);
            r.arms = [10.0 + 70.0 * e, 10.0 + 70.0 * e];
            r.dy += 2.5 * e;
            r.wing = e;
            r.fx = Fx::Moon(e);
        }
        Clip::BigFist => {
            let grow = ramp(t, 0.2, 0.6).min(1.0 - ramp(t, 1.3, 1.7));
            let hit = if t > 0.7 { (1.0 - ramp(t, 0.7, 1.2)).powi(2) } else { 0.0 };
            r.arms = [20.0, 80.0];
            r.rot = -10.0 * ramp(t, 0.2, 0.6) * (t < 0.7) as i32 as f32 + 12.0 * hit;
            r.dx = 2.0 * hit;
            r.mouth = 0.5 * grow;
            r.fx = if grow > 0.05 { Fx::Fist(grow) } else { Fx::None };
        }
        Clip::Kneel => {
            // the three-point landing
            let drop = ramp(t, 0.2, 0.45);
            let up = ramp(t, 1.8, 2.2);
            r.dy += if t < 0.45 { 4.0 * (t / 0.2).min(1.0) * (1.0 - drop) } else { 0.0 };
            let low = if t < 0.45 { 0.0 } else { 1.0 - up };
            r.crouch = 0.9 * low;
            r.legs = [(-1.8 * low, 0.0), (1.2 * low, 0.0)];
            r.arms = [10.0 + 110.0 * low, 10.0 + 12.0 * low];
            r.squash(1.0 - 0.14 * (1.0 - ramp(t, 0.45, 0.8)) * (t > 0.45) as i32 as f32);
            r.eyes = Eyes::Angry;
            r.look = 0.6 * low;
        }
        Clip::Echo => {
            let e = ease(t, len, 0.4);
            r.arms = [12.0, 12.0 + 40.0 * e];
            r.head = (0.0, 0.5 * e);
            r.eyes = Eyes::Half;
            r.fx = Fx::Radar(e, t);
        }
        Clip::Dash => {
            let e = ease(t, len, 0.22);
            r.turn = e;
            r.shear = 0.42 * e;
            r.legs = [((t * 60.0).sin() * 2.4, 0.6), (-(t * 60.0).sin() * 2.4, 0.6)];
            r.arms = [10.0 + 60.0 * e, 10.0 - 10.0 * e];
            r.alpha = if (0.25..1.25).contains(&t) { 0.8 } else { 1.0 };
            r.clones = 0.5 * e;
            r.look = e;
        }
        Clip::Spikes => {
            let e = ease(t, len, 0.35);
            r.arms = [10.0 + 130.0 * e, 10.0 + 130.0 * e];
            r.eyes = Eyes::Angry;
            r.head = (0.0, -0.4 * e);
            r.fx = Fx::Spikes(ramp(t, 0.5, 0.8).min(1.0 - ramp(t, 1.7, 2.1)));
        }
        Clip::Bubble => {
            let e = ease(t, len, 0.4);
            r.arms = [10.0 + 78.0 * e, 10.0 + 78.0 * e];
            r.eyes = Eyes::Angry;
            r.dx = 0.2 * e * osc(t, 8.0);
            r.fx = Fx::Dome(e * (0.94 + 0.06 * osc(t, 1.6)));
        }
        Clip::Orbit => {
            let e = ease(t, len, 0.5);
            r.dy += 3.0 * e + 0.5 * osc(t, 0.6) * e;
            r.arms = [10.0 + 60.0 * e, 10.0 + 125.0 * e];
            r.eyes = Eyes::Angry;
            r.fx = Fx::Scrap(e, t);
        }
        Clip::Blaze => {
            let e = ease(t, len, 0.4);
            r.dy += 5.0 * e + 0.5 * osc(t, 1.0) * e;
            r.rot = 24.0 * e;
            r.pivot = 0.5;
            r.aura = 0.9 * e;
            r.arms = [10.0 + 40.0 * e, 10.0 + 110.0 * e];
            r.mouth = 0.6 * e;
            r.fx = Fx::Fire(e, t);
        }
        Clip::Surf => {
            let e = ease(t, len, 0.45);
            r.dy += 5.5 * e + 0.7 * osc(t, 0.8) * e;
            r.rot = 9.0 * e + 4.0 * osc(t, 0.7) * e;
            r.crouch = 0.35 * e;
            r.legs = [(-1.6 * e, 0.0), (1.6 * e, 0.0)];
            r.arms = [10.0 + 80.0 * e, 10.0 + 65.0 * e];
            r.look = e;
            r.wing = e;
            r.fx = Fx::Board(e);
        }
        Clip::Limbs => {
            let e = ease(t, len, 0.4);
            r.dy += 2.5 * e;
            r.arms = [10.0 + 40.0 * e, 10.0 + 60.0 * e];
            r.eyes = Eyes::Angry;
            r.mouth = 0.5 * e;
            r.fx = Fx::Limbs(e, t);
        }
        Clip::Lob => {
            let wind = ramp(t, 0.1, 0.5);
            let fly = ramp(t, 0.55, 1.25);
            r.arms = [20.0, if t < 0.55 { 10.0 + 140.0 * wind } else { 70.0 }];
            r.rot = if t < 0.55 { -10.0 * wind } else { 8.0 * (1.0 - ramp(t, 0.55, 1.0)) };
            r.look = 1.0;
            r.eyes = Eyes::Happy;
            r.mouth = 0.7 * ramp(t, 1.2, 1.5) * (1.0 - ramp(t, 1.9, 2.2));
            r.fx = if t > 0.35 && t < 2.0 { Fx::Bomb(fly + ramp(t, 1.25, 1.9)) } else { Fx::None };
        }
        Clip::Flick => {
            let flick = (t * 1.6).fract();
            let on = (0.3..1.7).contains(&t);
            r.arms = [30.0, if on { 130.0 - 60.0 * flick } else { 60.0 }];
            r.rot = if on { -5.0 + 10.0 * flick } else { 0.0 };
            r.look = 1.0;
            r.aura = 0.3 * ease(t, len, 0.3);
            r.fx = if on { Fx::Cards(flick) } else { Fx::None };
        }
        _ => {}
    }
    let _ = k;
}

/// Where things are on the canvas, in cells from the middle of the floor (y down).
pub(crate) struct At {
    pub hand: (f32, f32), // front hand
    pub back: (f32, f32), // back hand
    pub head_y: f32,
    pub eye_y: f32,
    pub torso_y: f32,
    pub bottom: f32,
    pub hw: f32,
    pub tw: f32,
}

fn col(c: u8) -> u32 {
    pet_color(c).unwrap_or(CREAM)
}

/// A circle line, one cell thick, seen through a coarse pattern so it glows.
fn ring(cv: &mut Canvas, x: f32, y: f32, rad: f32, c: u32, thick: f32) {
    let n = (rad * 7.0).max(10.0) as i32;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        cv.cell(x + a.cos() * rad - thick / 2.0, y + a.sin() * rad * 0.95 - thick / 2.0, thick, thick, c);
    }
}

fn disc(cv: &mut Canvas, x: f32, y: f32, rad: f32, c: u32) {
    let n = (rad * 3.0) as i32;
    for j in -n..=n {
        let v = j as f32 / 3.0;
        let half = (rad * rad - v * v).max(0.0).sqrt();
        cv.cell(x - half, y + v, 2.0 * half, 0.34, c);
    }
}

fn shield(cv: &mut Canvas, pet: &Pet, x: f32, y: f32, rad: f32) {
    let Some((dot, rings)) = pet.gear.disc.split_last() else { return };
    for (k, c) in [1.0, 0.9, 0.68, 0.46, 0.26].into_iter().zip(rings) {
        disc(cv, x, y, rad * k, col(*c));
    }
    cv.cell(x - 0.5, y - 0.5, 1.0, 1.0, col(*dot));
}

/// A straight line of little blocks.
fn line(cv: &mut Canvas, a: (f32, f32), b: (f32, f32), w: f32, c: u32) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 3.0).max(1.0) as i32;
    for i in 0..=n {
        let k = i as f32 / n as f32;
        cv.cell(a.0 + (b.0 - a.0) * k - w / 2.0, a.1 + (b.1 - a.1) * k - w / 2.0, w, w, c);
    }
}

fn beam(cv: &mut Canvas, from: (f32, f32), to_x: f32, k: f32, c: u32) {
    let (x0, x1) = (from.0.min(to_x), from.0.max(to_x));
    for (h, c) in [(3.0 * k, c), (1.6 * k, rig::mix(c, rgb(255, 255, 255), 0.6)), (0.67, rgb(0xfb, 0xfb, 0xff))] {
        cv.cell(x0, from.1 - h / 2.0, x1 - x0, h.max(0.34), c);
    }
    disc(cv, from.0, from.1, 1.4 * k, rgb(0xfb, 0xfb, 0xff));
}

/// Capes, wings, shields on the back, portals, moons.
pub(crate) fn behind(cv: &mut Canvas, pet: &Pet, r: &Rig, at: &At) {
    let glow = col(pet.gear.glow);
    let mid = (at.head_y + at.bottom) / 2.0;
    match r.fx {
        Fx::Portal(k) => {
            disc(cv, 0.0, mid, 12.5 * k, rgb(0x2a, 0x1c, 0x3c));
            ring(cv, 0.0, mid, 13.0 * k, glow, 1.2);
            ring(cv, 0.0, mid, 14.2 * k, col(b'Y'), 0.34);
        }
        Fx::Moon(k) => {
            disc(cv, 0.0, at.head_y + 4.0, 11.0 * k, col(b'W'));
            disc(cv, 4.0 * k, at.head_y + 2.5, 9.6 * k, rgb(0x3a, 0x36, 0x55));
        }
        Fx::Orbs(k, t) => orbs(cv, k, t, mid, glow, false),
        Fx::Scrap(k, t) => scrap(cv, k, t, mid, false),
        Fx::FireWings(k, t) => {
            for side in [-1.0f32, 1.0] {
                for i in 0..9 {
                    let f = i as f32;
                    let x = side * (3.0 + f * 1.55 * k);
                    let flick = (t * 9.0 + f * 1.7).sin();
                    let tall = (9.5 - f * 0.7 + flick) * k;
                    let y = at.torso_y - 3.0 - f * 0.9 * k;
                    disc(cv, x, y, tall * 0.36, col(b'Q'));
                    disc(cv, x, y - 0.4, tall * 0.26, glow);
                    disc(cv, x, y - 0.6, tall * 0.14, col(b'Y'));
                }
            }
        }
        Fx::Fire(k, t) => {
            for i in 0..14 {
                let f = i as f32;
                let rise = (t * 1.7 + f * 0.37).fract();
                let x = ((f * 2.4).sin() * 7.0) * (1.0 - 0.4 * rise);
                let y = at.bottom + 1.0 - rise * (at.bottom - at.head_y + 5.0);
                disc(cv, x, y, (2.6 * (1.0 - rise) + 0.5) * k, if i % 3 == 0 { col(b'Y') } else { glow });
            }
        }
        Fx::Limbs(k, t) => limbs(cv, pet, at, k, t),
        _ => {}
    }
    match pet.gear.back {
        Back::Cape(c) => {
            // from the shoulders to just above the floor, wider than the arms
            let n = (-2.4 - at.torso_y).max(1.0) as i32;
            let back = 2.4 * r.turn;
            for pass in 0..2 {
                for i in 0..=n {
                    let d = i as f32;
                    let half = at.tw / 2.0 + 0.4 + d * 0.5 + r.wing * d * 0.45;
                    let off = r.tail * d * 0.16 - back - r.turn * d * 0.25;
                    let y = at.torso_y + 0.6 + d;
                    if pass == 0 {
                        cv.cell(off - half - 1.0, y - 0.34, 2.0 * half + 2.0, 1.67, PET_INK);
                    } else {
                        cv.cell(off - half, y, 2.0 * half, 1.0, col(c));
                    }
                }
            }
        }
        Back::Wings(a, b) => {
            let open = 0.35 + 0.65 * r.wing;
            for side in [-1.0f32, 1.0] {
                for pass in 0..2 {
                    for i in 0..9 {
                        let d = i as f32 * open * 1.45;
                        let tall = 5.0 - i as f32 * 0.38;
                        let x = side * (at.tw / 2.0 - 1.0 + d);
                        let y = at.torso_y - 1.0 - d * 0.35 + 0.5 * r.tail * i as f32 * 0.3;
                        if pass == 0 {
                            cv.cell(x - 1.4, y - 0.67, 2.8, tall + 1.34, PET_INK);
                        } else {
                            cv.cell(x - 0.8, y, 1.6, tall, col(if i % 3 == 2 { b } else { a }));
                        }
                    }
                }
            }
        }
        Back::Pack(c) => {
            // a cannon over the shoulder
            let (x, y) = (at.tw / 2.0 - 1.0, at.torso_y - 3.4);
            cv.cell(x - 3.0, y - 1.0, 9.0, 4.0, PET_INK);
            cv.cell(x - 2.0, y, 7.0, 2.0, col(c));
            cv.cell(x + 4.0, y, 1.0, 2.0, col(b'V'));
            cv.cell(x - 2.0, y + 2.0, 2.0, 4.0, PET_INK);
        }
        Back::Limbs(..) if !matches!(r.fx, Fx::Limbs(..)) => limbs(cv, pet, at, 0.55, r.hem * 0.5),
        Back::Swords => {
            for s in [-1.0f32, 1.0] {
                line(cv, (s * 4.0, at.torso_y + 3.0), (-s * 6.5, at.torso_y - 8.0), 1.67, PET_INK);
                line(cv, (s * 4.0, at.torso_y + 3.0), (-s * 6.0, at.torso_y - 7.4), 0.67, col(b'V'));
                line(cv, (-s * 5.4, at.torso_y - 6.8), (-s * 6.6, at.torso_y - 8.0), 1.2, col(b'B'));
            }
        }
        _ => {}
    }
}

/// Four long arms from the back, waving.
fn limbs(cv: &mut Canvas, pet: &Pet, at: &At, k: f32, t: f32) {
    let c = if let Back::Limbs(c, _) = pet.gear.back { col(c) } else { col(pet.gear.glow) };
    let metal = matches!(pet.gear.back, Back::Limbs(_, true));
    for i in 0..4 {
        let side = if i % 2 == 0 { -1.0f32 } else { 1.0 };
        let high = i < 2;
        let mut p = (side * 2.0, at.torso_y + 2.0);
        let n = 13;
        let mut pts = Vec::new();
        for j in 0..n {
            let f = j as f32 / n as f32;
            let wave = (t * 3.2 + j as f32 * 0.45 + i as f32 * 1.6).sin();
            let (dx, dy) = if high { (side * (1.25 - 0.9 * f), -0.75 - 0.5 * f) } else { (side * 1.25, -0.35 + 1.0 * f) };
            p = ((p.0 + dx * k + wave * 0.22).clamp(-17.0, 17.0), (p.1 + dy * k + wave * 0.32).min(-1.0));
            pts.push(p);
        }
        for q in &pts {
            cv.cell(q.0 - 1.34, q.1 - 1.34, 2.67, 2.67, PET_INK);
        }
        for (j, q) in pts.iter().enumerate() {
            cv.cell(q.0 - 0.67, q.1 - 0.67, 1.34, 1.34, if metal && j % 2 == 1 { col(b'N') } else { c });
        }
        if metal {
            for a in [-0.7f32, 0.0, 0.7] {
                line(cv, p, (p.0 + side * 2.0 * a.cos(), p.1 + 2.0 * a.sin() - 0.6), 0.8, col(b'V'));
            }
        }
    }
}

fn scrap(cv: &mut Canvas, k: f32, t: f32, mid: f32, front: bool) {
    for i in 0..8 {
        let a = t * 2.4 + i as f32 * TAU / 8.0;
        if (a.sin() > 0.0) != front {
            continue;
        }
        let (x, y) = (a.cos() * 13.0 * k, mid + a.sin() * 5.0 * k + 2.0 * (t * 1.7 + i as f32 * 2.0).sin());
        let (w, h) = [(2.6, 1.2), (1.2, 2.6), (2.0, 2.0)][i % 3];
        cv.cell(x - w / 2.0 - 0.67, y - h / 2.0 - 0.67, w + 1.34, h + 1.34, PET_INK);
        cv.cell(x - w / 2.0, y - h / 2.0, w, h, col([b'V', b'N', b'm'][i % 3]));
    }
}

fn orbs(cv: &mut Canvas, k: f32, t: f32, mid: f32, c: u32, front: bool) {
    for i in 0..5 {
        let a = t * 3.0 + i as f32 * TAU / 5.0;
        if (a.sin() > 0.0) != front {
            continue;
        }
        let (x, y) = (a.cos() * 11.0 * k, mid + a.sin() * 4.0 * k + 1.5 * (t * 2.0 + i as f32).sin());
        disc(cv, x, y, 1.7 * k, c);
        disc(cv, x, y, 0.8 * k, rgb(0xfb, 0xfb, 0xff));
    }
}

/// Everything that is drawn over the pet.
pub(crate) fn front(cv: &mut Canvas, pet: &Pet, r: &Rig, at: &At) {
    let glow = col(pet.gear.glow);
    let white = rgb(0xfb, 0xfb, 0xff);
    let mid = (at.head_y + at.bottom) / 2.0;
    let edge = 18.0;
    if let (Some(t), false) = (pet.gear.tool, r.gone || r.arms_front) {
        cv.grid(t.grid, at.hand.0 - t.grip.0 as f32 - 0.5, at.hand.1 - t.grip.1 as f32 - 0.5);
    }
    if pet.gear.back == Back::Disc && !r.gone && !r.arms_front {
        shield(cv, pet, at.back.0 - 0.6, at.back.1 - 0.6, 3.7); // on the arm
    }
    match r.fx {
        Fx::None | Fx::Portal(_) => {}
        Fx::Ray(k) => beam(cv, (at.hand.0 + 0.6, at.hand.1), edge, k, glow),
        Fx::EyeRay(k) => beam(cv, (at.hw * 0.12, at.eye_y - 2.0), edge, k * 0.8, glow),
        Fx::Rays(k) => {
            beam(cv, (at.hand.0 + 0.6, at.hand.1), edge, k, glow);
            beam(cv, (at.back.0 - 0.6, at.back.1), -edge, k, glow);
        }
        Fx::Ring(k) => {
            if k > 0.02 && k < 0.98 {
                ring(cv, 0.0, mid, 3.0 + 13.0 * k, glow, 1.2 * (1.0 - k) + 0.34);
                ring(cv, 0.0, mid, 2.0 + 11.5 * k, white, 0.34);
            }
        }
        Fx::Bolt(k) => {
            let seed = (k * 7.0) as i32;
            let mut p = (at.hand.0 + 1.0 + (seed % 3) as f32, -33.0);
            let n = 7;
            for i in 1..=n {
                let f = i as f32 / n as f32;
                let jig = if i == n { 0.0 } else { (((seed + i * 5) * 37 % 9) - 4) as f32 };
                let q = (at.hand.0 + jig, -33.0 + (at.hand.1 - 2.0 + 33.0) * f);
                line(cv, p, q, 1.67, glow);
                line(cv, p, q, 0.67, white);
                p = q;
            }
            disc(cv, at.hand.0, at.hand.1 - 2.0, 2.2, white);
        }
        Fx::Orbs(k, t) => orbs(cv, k, t, mid, glow, true),
        Fx::Scrap(k, t) => scrap(cv, k, t, mid, true),
        Fx::FireWings(..) | Fx::Fire(..) | Fx::Limbs(..) => {}
        Fx::Dome(k) => {
            let rad = 14.5 * k;
            for j in 0..40 {
                // a pattern of little lights, denser at the rim
                let a = j as f32 * 2.399;
                let d = rad * (j as f32 / 40.0).sqrt();
                cv.cell(a.cos() * d - 0.17, mid + a.sin() * d * 0.95 - 0.17, 0.34, 0.34, glow);
            }
            ring(cv, 0.0, mid, rad, glow, 0.8);
            ring(cv, 0.0, mid, rad - 1.0, white, 0.34);
        }
        Fx::Board(k) => {
            let [body, top, flame] = pet.gear.board;
            let w = 9.5 * k;
            disc(cv, -w, 0.8, 1.6, PET_INK);
            disc(cv, w, 0.8, 1.6, PET_INK);
            cv.cell(-w, -0.8, 2.0 * w, 3.2, PET_INK);
            cv.cell(-w, 0.0, 2.0 * w, 1.6, col(body));
            cv.cell(-w, 0.0, 2.0 * w, 0.5, col(top));
            if flame != 0 {
                cv.cell(-w - 2.0, 0.4, 2.0, 1.0, col(flame));
            }
        }
        Fx::Bomb(k) => {
            // k: 0..1 in the air, 1..2 going off
            let (x0, y0) = (at.hand.0, at.hand.1 - 1.0);
            let body = pet.gear.lights.first().map_or(glow, |c| col(*c));
            if k < 1.0 {
                let (x, y) = (x0 + (15.0 - x0) * k, y0 - 7.0 * 4.0 * k * (1.0 - k) + (-2.5 - y0) * k);
                disc(cv, x, y, 2.3, PET_INK);
                disc(cv, x, y, 1.7, body);
                if let Some(c) = pet.gear.lights.get(1) {
                    cv.cell(x - 1.0, y - 0.6, 0.67, 0.67, col(*c));
                    cv.cell(x + 0.4, y - 0.6, 0.67, 0.67, col(*c));
                }
                cv.cell(x - 0.3, y - 3.0, 0.67, 1.0, col(b'g'));
            } else {
                let b = k - 1.0;
                disc(cv, 14.0, -3.5, 4.6 * (1.0 - (b - 0.35).abs()).max(0.0) * (b < 0.95) as i32 as f32, body);
                disc(cv, 14.0, -3.5, 2.8 * (1.0 - b), col(b'Y'));
                disc(cv, 14.0, -3.5, 1.4 * (1.0 - b), white);
            }
        }
        Fx::Cards(k) => {
            for i in 0..3 {
                let x = at.hand.0 + (16.0 - at.hand.0) * (k - i as f32 * 0.12).clamp(0.0, 1.0);
                let y = at.hand.1 - 1.0 + (i as f32 - 1.0) * 2.6 * (0.3 + k);
                cv.cell(x - 1.4, y - 1.7, 2.8, 3.4, glow);
                cv.cell(x - 0.8, y - 1.2, 1.7, 2.4, white);
                cv.cell(x - 0.34, y - 0.34, 0.67, 0.67, col(b'R'));
            }
        }
        Fx::Sigil(k, t) => {
            for h in [at.hand, at.back] {
                let rad = 4.6 * k;
                ring(cv, h.0, h.1, rad, glow, 0.67);
                ring(cv, h.0, h.1, rad * 0.55, col(b'Y'), 0.34);
                for i in 0..4 {
                    let (a, b) = (t * 1.4 + i as f32 * TAU / 4.0, t * 1.4 + (i + 1) as f32 * TAU / 4.0);
                    line(cv, (h.0 + a.cos() * rad, h.1 + a.sin() * rad), (h.0 + b.cos() * rad, h.1 + b.sin() * rad), 0.34, col(b'Y'));
                }
            }
        }
        Fx::Thrown(k) => {
            let (x, y) = (at.hand.0 + 1.0 + (14.0 - at.hand.0) * k, at.hand.1);
            if pet.gear.back == Back::Disc {
                shield(cv, pet, x, y, 3.6);
            } else {
                // a stick, spinning
                let a = k * 14.0;
                let (dx, dy) = (a.cos() * 2.8, a.sin() * 2.8);
                line(cv, (x - dx, y - dy), (x + dx, y + dy), 1.67, PET_INK);
                line(cv, (x - dx, y - dy), (x + dx, y + dy), 0.67, glow);
            }
        }
        Fx::Arrow(draw, fly) => {
            let (x, y) = at.hand;
            for (a, b) in [((x - 1.0, y - 5.0), (x + 1.2, y - 2.4)), ((x + 1.2, y - 2.4), (x + 1.2, y + 2.4)), ((x + 1.2, y + 2.4), (x - 1.0, y + 5.0))] {
                line(cv, a, b, 1.4, PET_INK);
            }
            for (a, b) in [((x - 1.0, y - 5.0), (x + 1.2, y - 2.4)), ((x + 1.2, y - 2.4), (x + 1.2, y + 2.4)), ((x + 1.2, y + 2.4), (x - 1.0, y + 5.0))] {
                line(cv, a, b, 0.67, col(b'v'));
            }
            let pull = (x - 1.0 - 3.0 * draw, y);
            line(cv, (x - 1.0, y - 5.0), pull, 0.34, white);
            line(cv, pull, (x - 1.0, y + 5.0), 0.34, white);
            if fly < 1.0 {
                let tip = x + 3.0 + (edge - x - 3.0) * fly;
                let tail = if fly > 0.0 { tip - 5.0 } else { pull.0 };
                line(cv, (tail, y), (tip - 1.0, y), 0.67, col(b'd'));
                cv.cell(tip - 1.0, y - 0.67, 1.0, 1.34, col(b'V'));
            }
        }
        Fx::Thread => line(cv, (at.hand.0, at.hand.1), (at.hand.0 + 2.0, -34.0), 0.5, white),
        Fx::Hang => line(cv, (0.0, -0.4), (0.0, 15.0), 0.5, white),
        Fx::Slash(k) => {
            for i in 0..3 {
                let x = 7.0 + i as f32 * 2.6;
                let (a, b) = ((x + 2.0, mid - 7.0 + 2.0 * i as f32 * 0.4), (x - 2.0, mid - 7.0 + 12.0 * k));
                line(cv, a, b, 1.2, glow);
                line(cv, a, b, 0.5, white);
            }
        }
        Fx::Cut(k) => {
            // the blade leaves an arc behind it
            let (x, y, rad) = (2.0, at.torso_y + 1.0, 11.5);
            for i in 0..22 {
                let f = i as f32 / 22.0;
                let a = (-75.0 + 130.0 * k - 60.0 * (1.0 - f)).to_radians();
                let w = 0.4 + 1.6 * f;
                cv.cell(x + a.cos() * rad - w / 2.0, y + a.sin() * rad * 0.9 - w / 2.0, w, w, if f > 0.6 { white } else { glow });
            }
        }
        Fx::Claws => {
            for h in [at.hand, at.back] {
                for i in -1..=1 {
                    line(cv, (h.0 + i as f32 * 0.9, h.1), (h.0 + i as f32 * 0.9, h.1 + 3.2), 0.34, white);
                }
            }
        }
        Fx::Fist(k) => {
            let (x, y, s) = ((at.hand.0 + 2.6 * k).min(13.0), at.hand.1, 1.5 + 3.2 * k);
            disc(cv, x, y, s + 1.0, PET_INK);
            disc(cv, x, y, s, glow);
            disc(cv, x - s * 0.25, y - s * 0.25, s * 0.55, rig::mix(glow, white, 0.5));
            for i in 0..3 {
                cv.cell(x + s * 0.35, y - s * 0.7 + i as f32 * s * 0.5, s * 0.5, 0.34, PET_INK);
            }
        }
        Fx::Hoops(out, t) => {
            for i in 0..5 {
                let f = i as f32;
                for (h, d) in [(at.hand, 1.0f32), (at.back, -1.0)] {
                    let home = (h.0 - d * (f * 0.9), h.1);
                    let x = home.0 + d * out * (3.0 + f * 1.6);
                    let x = x.clamp(-16.0, 16.0);
                    let y = home.1 + (t * 6.0 + f).sin() * 1.6 * out;
                    ring(cv, x, y, 1.9, PET_INK, 1.0);
                    ring(cv, x, y, 1.9, if out > 0.1 { col(b'z') } else { col(b'A') }, 0.5);
                }
            }
        }
        Fx::Spikes(k) => {
            for i in -3i32..=3 {
                if i == 0 {
                    continue;
                }
                let x = i as f32 * 4.4;
                let top = -(9.0 + (i.abs() % 2) as f32 * 4.0) * k;
                line(cv, (x, 0.0), (x, top), 1.9, PET_INK);
                line(cv, (x, 0.0), (x, top + 0.6), 0.8, col(b'V'));
                cv.cell(x - 0.34, top - 0.6, 0.67, 1.0, glow);
            }
        }
        Fx::Moon(k) => {
            for (s, d) in [(1.0f32, 0.0f32), (1.0, 4.0)] {
                let x = s * (8.0 + 6.0 * k);
                let y = mid - 3.0 + d;
                ring(cv, x, y, 1.6, col(b'V'), 0.67);
                cv.cell(x, y - 1.2, 2.2, 2.4, rig::CLEAR);
            }
        }
        Fx::Flash(k) => {
            let gun = matches!(pet.gear.back, Back::Pack(_));
            let (x, y) = if gun { (at.tw / 2.0 + 5.4, at.torso_y - 2.4) } else { ((at.hand.0 + 8.0).min(12.5), at.hand.1 - 1.0) };
            let s = 2.6 * (1.0 - k);
            disc(cv, x + 1.0, y, s + 0.6, glow);
            disc(cv, x + 1.0, y, s * 0.6, white);
            for i in 0..3 {
                let bx = x + 2.0 + ((k + i as f32 * 0.33).fract()) * (edge - x - 2.5);
                cv.cell(bx, y - 0.34, 1.4, 0.67, col(b'Y'));
            }
        }
        Fx::Jets(k) => {
            if matches!(pet.gear.back, Back::None | Back::Pack(_)) && k > 0.1 {
                for x in [-pet.legs.hip * 0.6, pet.legs.hip * 0.6] {
                    let l = 2.6 * k * (0.7 + 0.3 * (r.hem * 40.0 + x).sin());
                    cv.cell(x - 1.0, 0.0, 2.0, l, glow);
                    cv.cell(x - 0.34, 0.0, 0.67, l * 0.7, white);
                }
            }
        }
        Fx::Lights(k) => {
            let own = [pet.gear.glow; 6];
            let lights = if pet.gear.lights.is_empty() { &own[..] } else { pet.gear.lights };
            for (i, c) in lights.iter().enumerate() {
                let a = i as f32 * TAU / lights.len() as f32 + r.hem * 2.0;
                disc(cv, at.hand.0 + a.cos() * 3.4 * k, at.hand.1 + a.sin() * 3.4 * k, 0.9 * k, col(*c));
            }
        }
        Fx::Heart(k) => {
            let s = 0.9 * k;
            let (x, y) = (0.0, at.torso_y + 1.0);
            for (j, row) in HEART.iter().enumerate() {
                for (i, c) in row.iter().enumerate() {
                    if *c == b'X' {
                        cv.cell(x + (i as f32 - 2.5) * s * 1.3, y + j as f32 * s * 1.3, s * 1.4, s * 1.4, PINK);
                    }
                }
            }
        }
        Fx::Radar(k, t) => {
            for i in 0..3 {
                let f = ((t * 0.6 + i as f32 / 3.0).fract()) * k;
                if f > 0.03 {
                    ring(cv, 0.0, at.head_y + 5.0, 6.0 + 10.0 * f, glow, 0.5 * (1.0 - f) + 0.2);
                }
            }
        }
        Fx::Flowers(k) => {
            for (i, (x, y)) in [(-6.0f32, at.head_y + 0.5), (5.5, at.head_y + 1.0), (at.hand.0, at.hand.1), (at.back.0, at.back.1), (0.0, at.head_y - 1.0)].into_iter().enumerate() {
                let c = [PINK, col(b'Y'), white, col(b'p'), col(b'z')][i];
                for a in 0..5 {
                    let a = a as f32 * TAU / 5.0;
                    disc(cv, x + a.cos() * 1.3 * k, y + a.sin() * 1.3 * k, 0.9 * k, c);
                }
                disc(cv, x, y, 0.7 * k, col(b'A'));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everybody who came in a pack.
    fn others() -> impl Iterator<Item = &'static Pet> {
        PETS.iter().skip(OURS.len())
    }

    #[test]
    fn everybody_with_powers_has_one_to_show() {
        for h in others() {
            assert!(h.tricks.iter().any(|c| secs(*c).is_some()), "{} has no power", h.name);
            assert!(pet_color(h.gear.glow).is_some() && pet_color(h.eye).is_some(), "{}", h.name);
        }
    }

    #[test]
    fn powers_show_something_and_put_everything_back() {
        for h in others() {
            for &c in h.tricks.iter().filter(|c| secs(**c).is_some()) {
                let len = secs(c).unwrap();
                let rest = sample(h, Clip::Stand, 0.5, &Push::NONE);
                let mut biggest = 0.0f32;
                for i in 0..60 {
                    let r = sample(h, c, len * i as f32 / 60.0, &Push::NONE);
                    let d = (r.dx - rest.dx).abs() + (r.dy - rest.dy).abs() + r.rot.abs() / 10.0 + (r.arms[1] - rest.arms[1]).abs() / 20.0 + r.aura + r.clones + (1.0 - r.alpha) + (r.sy - 1.0).abs() * 4.0;
                    biggest = biggest.max(d + (r.fx != Fx::None) as i32 as f32);
                }
                assert!(biggest > 1.0, "{} {c:?} does nothing to see", h.name);
                let end = sample(h, c, len, &Push::NONE);
                assert!(end.alpha > 0.99 && end.clones < 0.05 && end.aura < 0.05 && (end.reach - 1.0).abs() < 0.05, "{} {c:?} does not end", h.name);
            }
        }
    }
}
