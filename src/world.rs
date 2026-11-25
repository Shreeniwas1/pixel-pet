//! The world the pet lives in: the connected monitors, how they touch, and ways to get from the
//! floor of one to the floor of another. Pure geometry, so any arrangement can be tested.

use crate::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct Screen {
    pub full: RECT,
    pub work: RECT, // without the taskbar; its bottom is the floor
}

/// How a stretch of a journey is covered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Way {
    Walk,
    Stairs,
    Ladder,
    Float, // for things that fly
    Hop,   // jump off the edge
    Drop,  // step off and come down under an umbrella
    Leap,  // one great jump
    Reel,  // up or down on a line
    Warp,  // not there any more, and there
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Leg {
    pub to: (f32, f32),
    pub way: Way,
}

/// Monitors count as touching when their edges are this close.
const TOUCH: i32 = 8;
/// A difference in floor height this small is just stepped over.
pub(crate) const STEP_OVER: i32 = 30;

fn near(a: i32, b: i32) -> bool {
    (a - b).abs() <= TOUCH
}

/// The monitor this point is on.
pub(crate) fn screen_at(screens: &[Screen], x: f32, y: f32) -> Option<usize> {
    let (x, y) = (x as i32, y as i32);
    screens.iter().position(|s| contains(&s.full, x, y))
}

/// The monitor a pet with its feet here is standing on (a point just above the floor counts).
pub(crate) fn standing_on(screens: &[Screen], x: f32, y: f32) -> Option<usize> {
    screen_at(screens, x, y - 2.0).or_else(|| {
        screens.iter().position(|s| x as i32 >= s.full.left && (x as i32) < s.full.right && (y as i32) > s.full.top && y as i32 <= s.full.bottom + 2)
    })
}

/// The neighbour directly beside screen `i` on that side, at this height.
pub(crate) fn beside(screens: &[Screen], i: usize, dir: i32, y: f32) -> Option<usize> {
    let a = &screens[i].full;
    screens.iter().position(|s| {
        let b = &s.full;
        (if dir > 0 { near(b.left, a.right) } else { near(b.right, a.left) }) && (y as i32) > b.top && (y as i32) <= b.bottom
    })
}

/// How far left and right something can fly at this height before it hits the end of the world.
pub(crate) fn span(screens: &[Screen], x: f32, y: f32) -> Option<(i32, i32)> {
    let start = standing_on(screens, x, y)?;
    let mut out = (screens[start].full.left, screens[start].full.right);
    for dir in [-1, 1] {
        let mut i = start;
        for _ in 0..screens.len() {
            let Some(n) = beside(screens, i, dir, y) else { break };
            if dir < 0 { out.0 = screens[n].full.left } else { out.1 = screens[n].full.right }
            i = n;
        }
    }
    Some(out)
}

/// How far left and right one can simply walk from screen `i`.
pub(crate) fn level_span(screens: &[Screen], i: usize) -> (i32, i32) {
    let mut out = (screens[i].work.left, screens[i].work.right);
    for dir in [-1, 1] {
        let mut at = i;
        for _ in 0..screens.len() {
            let floor = screens[at].work.bottom;
            match beside(screens, at, dir, floor as f32 - 2.0) {
                Some(n) if (screens[n].work.bottom - floor).abs() <= STEP_OVER => at = n,
                _ => break,
            }
            if dir < 0 { out.0 = screens[at].work.left } else { out.1 = screens[at].work.right }
        }
    }
    out
}

/// The floor within a small step of these feet.
pub(crate) fn level_floor(screens: &[Screen], x: f32, y: f32) -> Option<i32> {
    screens.iter().filter(|s| (x as i32) >= s.full.left && (x as i32) < s.full.right).map(|s| s.work.bottom).find(|f| (*f as f32 - y).abs() <= STEP_OVER as f32)
}

/// The floor below this point. Floors at or above `through` are ignored (falling past them).
pub(crate) fn floor_below(screens: &[Screen], x: f32, y: f32, through: Option<f32>) -> Option<i32> {
    screens
        .iter()
        .filter(|s| (x as i32) >= s.full.left && (x as i32) < s.full.right)
        .map(|s| s.work.bottom)
        .filter(|f| *f as f32 >= y - 1.0 && through.map_or(true, |t| *f as f32 > t))
        .min()
}

/// The top of the topmost monitor reachable straight up from here.
pub(crate) fn ceiling(screens: &[Screen], x: f32, y: f32) -> Option<i32> {
    let mut i = standing_on(screens, x, y)?;
    for _ in 0..screens.len() {
        let top = screens[i].full.top;
        match screens.iter().position(|s| near(s.full.bottom, top) && (x as i32) >= s.full.left && (x as i32) < s.full.right) {
            Some(n) if n != i => i = n,
            _ => break,
        }
    }
    Some(screens[i].full.top)
}

/// The size and abilities of the traveller.
#[derive(Clone, Copy)]
pub(crate) struct Traveller {
    pub half: f32,
    pub tall: f32,
    pub flies: bool,
    pub climbs: bool, // can use a ladder
    pub power: Option<Way>, // needs neither: Leap, Reel or Warp
}

/// Stairs from `a` to `b` inside `l..r`, turning back at the ends if one flight is not enough.
fn flights(a: (f32, f32), b_y: f32, l: f32, r: f32) -> Vec<Leg> {
    let mut out = Vec::new();
    let (mut x, mut y) = a;
    let mut dir = if r - x > x - l { 1.0 } else { -1.0 };
    let down = if b_y > y { 1.0 } else { -1.0 };
    for _ in 0..24 {
        let room = if dir > 0.0 { r - x } else { x - l };
        let left = (b_y - y).abs();
        let go = left.min(room.max(1.0));
        x += dir * go;
        y += down * go;
        out.push(Leg { to: (x, y), way: Way::Stairs });
        if (b_y - y).abs() < 1.0 {
            break;
        }
        dir = -dir;
    }
    out
}

/// A way from the floor of screen `from` (standing at `x`) to the floor of screen `to`, or None
/// if they do not touch. `taste` (0..1) picks between equally good ways.
pub(crate) fn plan(screens: &[Screen], from: usize, x: f32, to: usize, who: &Traveller, taste: f32) -> Option<Vec<Leg>> {
    if from == to {
        return None;
    }
    let (a, b) = (&screens[from], &screens[to]);
    let (fa, fb) = (a.work.bottom as f32, b.work.bottom as f32);
    let margin = who.half + 6.0;

    // side by side
    for dir in [1.0f32, -1.0] {
        let edge = if dir > 0.0 { a.full.right } else { a.full.left };
        let other = if dir > 0.0 { b.full.left } else { b.full.right };
        let overlap = a.full.bottom.min(b.full.bottom) - a.full.top.max(b.full.top);
        if !near(edge, other) || (overlap as f32) < who.tall {
            continue;
        }
        let edge = edge as f32;
        let inside = edge - dir * margin; // last spot on this screen
        let beyond = edge + dir * margin; // first spot on the other
        let walk = |to| Leg { to, way: Way::Walk };
        let diff = fb - fa;
        if diff.abs() <= STEP_OVER as f32 {
            return Some(vec![walk((inside, fa)), walk((beyond, fb))]);
        }
        if diff > 0.0 {
            // the other floor is lower; the edge has to be beside the other picture
            if fa <= b.full.top as f32 + 4.0 {
                continue;
            }
            if who.flies {
                return Some(vec![walk((inside, fa)), walk((edge + dir * 2.0, fa)), Leg { to: (edge + dir * (margin + diff.min(200.0)), fb), way: Way::Float }]);
            }
            match who.power {
                Some(Way::Warp) => return Some(vec![walk((inside, fa)), Leg { to: (beyond, fb), way: Way::Warp }]),
                Some(w) => {
                    // somebody who hangs on a line steps off the edge and is let down
                    if w == Way::Reel {
                        return Some(vec![walk((inside, fa)), Leg { to: (beyond, fb), way: Way::Leap }]);
                    }
                    let far = beyond + dir * (diff * 0.25).min((b.full.right - b.full.left) as f32 * 0.3);
                    return Some(vec![walk((inside, fa)), Leg { to: (far, fb), way: Way::Leap }]);
                }
                None => {}
            }
            let room = (b.full.right - b.full.left) as f32 - 2.0 * margin;
            if taste < 0.5 && diff <= room {
                let mut legs = vec![walk((inside, fa)), walk((edge + dir * 2.0, fa))];
                legs.push(Leg { to: (edge + dir * (2.0 + diff), fb), way: Way::Stairs });
                return Some(legs);
            }
            return Some(vec![walk((inside, fa)), Leg { to: (beyond, fb), way: Way::Hop }]);
        }
        // the other floor is higher: climb on this side, then step across
        if fb - 4.0 <= a.full.top as f32 {
            continue;
        }
        let up = -diff;
        if who.flies {
            return Some(vec![walk((inside - dir * up.min(200.0), fa)), Leg { to: (inside, fb), way: Way::Float }, walk((beyond, fb))]);
        }
        match who.power {
            Some(Way::Reel) => return Some(vec![walk((inside, fa)), Leg { to: (inside, fb), way: Way::Reel }, walk((beyond, fb))]),
            Some(Way::Leap) => return Some(vec![walk((inside - dir * (up * 0.3).min(160.0), fa)), Leg { to: (beyond, fb), way: Way::Leap }]),
            Some(w) => return Some(vec![walk((inside, fa)), Leg { to: (beyond, fb), way: w }]),
            None => {}
        }
        let room = (a.full.right - a.full.left) as f32 - 2.0 * margin;
        if who.climbs && (taste < 0.4 || up > room) {
            return Some(vec![walk((inside, fa)), Leg { to: (inside, fb), way: Way::Ladder }, walk((beyond, fb))]);
        }
        if up <= room {
            return Some(vec![walk((inside - dir * up, fa)), Leg { to: (inside, fb), way: Way::Stairs }, walk((beyond, fb))]);
        }
        continue;
    }

    // one above the other
    let below = near(a.full.bottom, b.full.top);
    let above = near(a.full.top, b.full.bottom);
    if below || above {
        let l = a.full.left.max(b.full.left) as f32 + margin;
        let r = a.full.right.min(b.full.right) as f32 - margin;
        if r - l < 40.0 {
            return None;
        }
        let start = x.clamp(l, r);
        let first = Leg { to: (start, fa), way: Way::Walk };
        if who.flies {
            return Some(vec![first, Leg { to: ((start + 150.0).clamp(l, r), fb), way: Way::Float }]);
        }
        if let Some(way) = who.power {
            let dx = if way == Way::Reel { 0.0 } else if start > (l + r) / 2.0 { -60.0 } else { 60.0 };
            return Some(vec![first, Leg { to: ((start + dx).clamp(l, r), fb), way }]);
        }
        if below && taste < 0.45 {
            return Some(vec![first, Leg { to: (start, fb), way: Way::Drop }]);
        }
        if who.climbs && taste > 0.8 {
            return Some(vec![first, Leg { to: (start, fb), way: Way::Ladder }]);
        }
        let mut legs = vec![first];
        legs.extend(flights((start, fa), fb, l, r));
        return Some(legs);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(l: i32, t: i32, w: i32, h: i32, bar: i32) -> Screen {
        let full = RECT { left: l, top: t, right: l + w, bottom: t + h };
        Screen { full, work: RECT { bottom: t + h - bar, ..full } }
    }

    const CAT: Traveller = Traveller { half: 48.0, tall: 110.0, flies: false, climbs: true, power: None };
    const CAR: Traveller = Traveller { climbs: false, ..CAT };
    const GHOST: Traveller = Traveller { flies: true, ..CAT };
    const LEAPER: Traveller = Traveller { power: Some(Way::Leap), ..CAT };
    const REELER: Traveller = Traveller { power: Some(Way::Reel), ..CAT };
    const WIZARD: Traveller = Traveller { power: Some(Way::Warp), ..CAT };

    /// Every way there is, for every taste, from everywhere, must stay where it can be seen.
    fn check(screens: &[Screen], from: usize, to: usize) -> Vec<Way> {
        let mut ways = Vec::new();
        for who in [CAT, CAR, GHOST, LEAPER, REELER, WIZARD] {
            for taste in [0.0, 0.2, 0.45, 0.5, 0.7, 0.9, 0.99] {
                let f = &screens[from];
                for k in 1..10 {
                    let x = f.full.left as f32 + (f.full.right - f.full.left) as f32 * k as f32 / 10.0;
                    let legs = plan(screens, from, x, to, &who, taste).expect("no way there");
                    let mut at = (x, f.work.bottom as f32);
                    for l in &legs {
                        assert!(who.climbs || l.way != Way::Ladder, "a car on a ladder");
                        if who.flies || who.power.is_some() {
                            assert!(!matches!(l.way, Way::Ladder | Way::Stairs | Way::Drop | Way::Hop), "{:?} for somebody with powers", l.way);
                        }
                        if l.way == Way::Walk {
                            assert!((l.to.1 - at.1).abs() <= STEP_OVER as f32, "walks up a wall: {at:?} -> {:?}", l.to);
                        }
                        if !matches!(l.way, Way::Hop | Way::Drop | Way::Leap | Way::Warp) {
                            // sample the way: the feet are always on (or at the edge of) some monitor
                            for i in 0..=20 {
                                let k = i as f32 / 20.0;
                                let p = (at.0 + (l.to.0 - at.0) * k, at.1 + (l.to.1 - at.1) * k);
                                assert!(standing_on(screens, p.0, p.1).is_some(), "{:?} leaves the screens at {p:?} from {at:?}", l);
                            }
                        }
                        at = l.to;
                        ways.push(l.way);
                    }
                    assert_eq!(standing_on(screens, at.0, at.1), Some(to), "ends on the wrong monitor: {at:?}");
                    assert_eq!(at.1, screens[to].work.bottom as f32, "does not end on the floor");
                }
            }
        }
        ways
    }

    #[test]
    fn level_monitors_side_by_side_are_simply_walked_across() {
        let s = [screen(0, 0, 1920, 1080, 32), screen(-1920, 6, 1920, 1080, 40)];
        assert!(check(&s, 0, 1).iter().all(|w| *w == Way::Walk));
        assert!(check(&s, 1, 0).iter().all(|w| *w == Way::Walk));
    }

    #[test]
    fn level_ground_runs_on_across_monitors_but_stops_at_a_step() {
        let s = [screen(0, 0, 1920, 1080, 32), screen(-1920, 6, 1920, 1080, 40), screen(1920, 400, 1920, 1080, 40)];
        assert_eq!(level_span(&s, 0), (-1920, 1920));
        assert_eq!(level_span(&s, 2), (1920, 3840));
        assert_eq!(level_floor(&s, -5.0, 1048.0), Some(1046));
        assert_eq!(level_floor(&s, 1930.0, 1048.0), None);
    }

    #[test]
    fn a_lower_neighbour_is_reached_by_jumping_or_stairs_and_left_by_climbing() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(1920, 400, 1920, 1080, 40)];
        let down = check(&s, 0, 1);
        assert!(down.contains(&Way::Hop) && down.contains(&Way::Stairs));
        let up = check(&s, 1, 0);
        assert!(up.contains(&Way::Ladder) && up.contains(&Way::Stairs) && !up.contains(&Way::Hop));
    }

    #[test]
    fn stacked_monitors_use_umbrella_stairs_or_ladder() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(200, 1080, 1366, 768, 40)];
        let down = check(&s, 0, 1);
        assert!(down.contains(&Way::Drop) && down.contains(&Way::Stairs) && down.contains(&Way::Ladder));
        let up = check(&s, 1, 0);
        assert!(up.contains(&Way::Stairs) && up.contains(&Way::Ladder) && !up.contains(&Way::Drop));
    }

    #[test]
    fn portrait_monitors_work_too() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(1920, -420, 1080, 1920, 40)];
        check(&s, 0, 1);
        check(&s, 1, 0);
        // a narrow portrait screen above a landscape one needs several flights of stairs
        let t = [screen(0, 0, 1920, 1080, 40), screen(700, -1920, 500, 1920, 40)];
        check(&t, 0, 1);
        check(&t, 1, 0);
    }

    #[test]
    fn monitors_that_do_not_touch_cannot_be_reached() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(1920, 1080, 1920, 1080, 40), screen(4000, 0, 800, 600, 0)];
        assert!(plan(&s, 0, 500.0, 1, &CAT, 0.3).is_none());
        assert!(plan(&s, 0, 500.0, 2, &CAT, 0.3).is_none());
        assert!(plan(&s, 0, 500.0, 0, &CAT, 0.3).is_none());
    }

    #[test]
    fn a_thrown_pet_can_cross_to_a_neighbour_at_that_height_but_not_beyond_the_world() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(1920, 400, 1920, 1080, 40)];
        assert_eq!(span(&s, 500.0, 600.0), Some((0, 3840)));
        assert_eq!(span(&s, 500.0, 200.0), Some((0, 1920)), "the neighbour does not reach that high");
        assert_eq!(span(&s, 2500.0, 1300.0), Some((1920, 3840)));
    }

    #[test]
    fn floors_and_ceilings_of_stacked_monitors() {
        let s = [screen(0, 0, 1920, 1080, 40), screen(0, 1080, 1920, 1080, 40)];
        assert_eq!(floor_below(&s, 500.0, 300.0, None), Some(1040));
        assert_eq!(floor_below(&s, 500.0, 1040.0, Some(1080.0)), Some(2120), "falling through to the lower one");
        assert_eq!(floor_below(&s, 500.0, 1500.0, None), Some(2120));
        assert_eq!(ceiling(&s, 500.0, 1500.0), Some(0));
        assert_eq!(floor_below(&s, 9000.0, 300.0, None), None);
    }
}
