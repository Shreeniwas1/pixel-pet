//! How somebody with powers gets about and saves itself from a fall: no ladders, no stairs, no
//! umbrella. Pure rules; `body.rs` does the moving.

use crate::body::Flight;
use crate::prelude::*;

/// What a pet can do that others need a ladder for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Gift {
    None,
    Fly,
    Leap,  // jumps as high as it likes, and lands on its feet
    Web,   // these three pull themselves up and let themselves down on a line
    Cable,
    Limb,
    Warp,  // is suddenly somewhere else
}

impl Gift {
    /// Hangs from something.
    pub fn reels(self) -> bool {
        matches!(self, Gift::Web | Gift::Cable | Gift::Limb)
    }

    /// What it uses instead of a ladder.
    pub fn way(self) -> Option<Way> {
        match self {
            Gift::None => None,
            Gift::Fly => Some(Way::Float),
            Gift::Leap => Some(Way::Leap),
            Gift::Warp => Some(Way::Warp),
            _ => Some(Way::Reel),
        }
    }
}

/// Saving itself from a fall.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Save {
    pub t: f32,
    pub anchor: (f32, f32), // where the line holds
}

/// Half the time a jump through nowhere takes: gone at this moment, there again a moment later.
pub(crate) const WARP: f32 = 0.35;
/// How long a line takes to reach the ceiling.
const SHOOT: f32 = 0.15;

/// Is this fall worth saving itself from?
pub(crate) fn needed(gift: Gift, vy: f32, height: f32, tall: f32, scale: f32) -> bool {
    gift != Gift::None && vy > 380.0 * scale && height > tall * 1.2
}

/// Change the speed of a fall; what is left is how much of the gravity still applies.
pub(crate) fn steer(gift: Gift, s: &Save, f: &mut Flight, dt: f32, scale: f32) -> f32 {
    let towards = |v: f32, goal: f32, rate: f32| v + (goal - v) * (1.0 - (-rate * dt).exp());
    match gift {
        Gift::None => 1.0,
        // ready for it
        Gift::Leap => {
            f.vx = towards(f.vx, 0.0, 0.6);
            1.0
        }
        // stops in the air, looks round, and comes down at its leisure
        Gift::Fly => {
            f.vy = if s.t < 0.5 { towards(f.vy, 0.0, 9.0) } else { towards(f.vy, 250.0 * scale, 4.0) };
            f.vx = towards(f.vx, 0.0, 1.6);
            0.0
        }
        Gift::Warp => {
            f.vy = towards(f.vy, 0.0, 9.0);
            f.vx = towards(f.vx, 0.0, 6.0);
            0.0
        }
        _ if s.t < SHOOT => 1.0,
        // caught by the line: a swing, then down along it
        _ => {
            f.vy = if s.t < 0.65 { towards(f.vy, 0.0, 12.0) } else { towards(f.vy, 320.0 * scale, 5.0) };
            f.vx += ((s.anchor.0 - f.x) * 9.0 - f.vx * 1.3) * dt;
            0.0
        }
    }
}

/// How much of the line is out.
pub(crate) fn line_out(s: &Save) -> f32 {
    (s.t / SHOOT).min(1.0)
}

/// A leap from `a` to `b`: where it is at `k` (0..1), and how long the whole of it takes.
pub(crate) fn vault(a: (f32, f32), b: (f32, f32), k: f32, scale: f32) -> (f32, f32) {
    let up = 70.0 * scale + (b.1 - a.1).abs() * 0.30;
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k - 4.0 * up * k * (1.0 - k))
}

pub(crate) fn vault_secs(a: (f32, f32), b: (f32, f32), scale: f32) -> f32 {
    (0.40 + (b.0 - a.0).hypot(b.1 - a.1) / (1700.0 * scale)).clamp(0.5, 1.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::fly;

    const AREA: RECT = RECT { left: 0, top: 0, right: 1920, bottom: 1048 };
    const ALL: [Gift; 6] = [Gift::Fly, Gift::Leap, Gift::Web, Gift::Cable, Gift::Limb, Gift::Warp];

    /// Fall from `y` with this speed: (seconds, speed on landing, fastest while saving itself).
    fn fall(gift: Gift, y: f32, v: (f32, f32)) -> (f32, f32) {
        let mut f = Flight { x: 900.0, y, vx: v.0, vy: v.1 };
        let mut save: Option<Save> = None;
        for n in 0..2000 {
            let dt = 0.033;
            if save.is_none() && needed(gift, f.vy, 1048.0 - f.y, 120.0, 1.0) {
                save = Some(Save { t: 0.0, anchor: (f.x + f.vx * 0.15, 0.0) });
            }
            let mut g = 1.0;
            if let Some(s) = &mut save {
                s.t += dt;
                g = steer(gift, s, &mut f, dt, 1.0);
                if gift == Gift::Warp && s.t >= WARP {
                    return (n as f32 * dt, 0.0); // it is down
                }
            }
            let before = f.vy;
            let end = fly(&mut f, dt, 1900.0 * g, &AREA, 48.0, 120.0);
            assert!(f.x.is_finite() && f.y.is_finite() && f.y <= 1048.5);
            if end.is_some() || (save.is_some() && before > 0.0 && f.y >= 1047.5) {
                return (n as f32 * dt, before);
            }
        }
        panic!("{gift:?} never comes down from {y} at {v:?}")
    }

    #[test]
    fn everyone_with_a_gift_lands_gently_however_hard_they_were_thrown() {
        for gift in ALL {
            for v in [(0.0, 0.0), (900.0, 400.0), (-2400.0, -900.0), (0.0, 1500.0), (300.0, 2400.0)] {
                let (secs, speed) = fall(gift, 150.0, v);
                assert!(secs < 12.0, "{gift:?} takes {secs} s");
                if gift != Gift::Leap {
                    assert!(speed < 520.0, "{gift:?} hits the floor at {speed} after {v:?}");
                }
            }
        }
    }

    #[test]
    fn the_rest_of_them_still_fall() {
        assert!(!needed(Gift::None, 5000.0, 900.0, 120.0, 1.0));
        let (_, speed) = fall(Gift::None, 150.0, (0.0, 0.0));
        assert!(speed < 600.0, "bounces until it has calmed down: {speed}");
    }

    #[test]
    fn a_small_drop_is_not_worth_it() {
        for gift in ALL {
            assert!(!needed(gift, 900.0, 60.0, 120.0, 1.0));
            assert!(!needed(gift, 100.0, 900.0, 120.0, 1.0), "going up, or only just falling");
            assert!(needed(gift, 900.0, 600.0, 120.0, 1.0));
        }
    }

    #[test]
    fn a_leap_begins_and_ends_where_it_should_and_clears_both() {
        for (a, b) in [((100.0, 1000.0), (300.0, 400.0)), ((100.0, 400.0), (300.0, 1000.0)), ((0.0, 500.0), (900.0, 500.0))] {
            assert_eq!(vault(a, b, 0.0, 1.0), a);
            assert_eq!(vault(a, b, 1.0, 1.0), b);
            let top = (0..=100).map(|i| vault(a, b, i as f32 / 100.0, 1.0).1).fold(f32::MAX, f32::min);
            assert!(top < a.1.min(b.1) - 20.0, "{top} does not clear {a:?} {b:?}");
            assert!((0.4..1.3).contains(&vault_secs(a, b, 1.0)));
        }
    }

    #[test]
    fn the_first_pets_have_no_gift_and_some_of_the_others_do() {
        for (i, p) in PETS.iter().enumerate() {
            assert!(i >= OURS.len() || p.gear.gift == Gift::None, "{}", p.name);
        }
        for g in [Gift::Fly, Gift::Leap, Gift::Cable, Gift::Limb, Gift::Warp] {
            assert!(PETS.iter().any(|p| p.gear.gift == g), "nobody has {g:?}");
        }
    }
}
