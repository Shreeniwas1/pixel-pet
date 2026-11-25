//! Webs that reach across the screen: a see-through window that only shows one strand, and the
//! swinging and climbing done on it.

use crate::prelude::*;

/// What it is made of.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Stuff {
    Web,
    Cable,
    Limb(u32, f32), // an arm: its colour and how thick it is
}

/// A strand from a hand (or foot) to where it sticks.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Strand {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub stuck: bool, // it has arrived, so it has spread out there
    pub scale: f32,
    pub stuff: Stuff,
}

static STRAND: Mutex<Option<Strand>> = Mutex::new(None);
static WND: AtomicIsize = AtomicIsize::new(0);

/// Room around the strand for its thickness and the patch at its end.
fn pad(s: f32) -> i32 {
    (44.0 * s) as i32 + 4
}

pub(crate) unsafe fn register(hinst: HINSTANCE) {
    let class = wide("PixelWebWnd");
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    RegisterClassW(&wc);
}

fn area(s: &Strand) -> RECT {
    let p = pad(s.scale);
    RECT {
        left: s.from.0.min(s.to.0) as i32 - p,
        top: s.from.1.min(s.to.1) as i32 - p,
        right: s.from.0.max(s.to.0) as i32 + p,
        bottom: s.from.1.max(s.to.1) as i32 + p,
    }
}

/// Show this strand, or none. Called after the state lock is let go.
pub(crate) unsafe fn show(strand: Option<Strand>) {
    let was = {
        let mut s = STRAND.lock().unwrap();
        std::mem::replace(&mut *s, strand)
    };
    if was == strand {
        return;
    }
    let w = WND.load(Relaxed) as HWND;
    let Some(s) = strand else {
        if !w.is_null() {
            ShowWindow(w, SW_HIDE);
        }
        return;
    };
    let w = if w.is_null() {
        let w = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            wide("PixelWebWnd").as_ptr(),
            wide("Pixel Pet - Web").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), GetModuleHandleW(null()), null(),
        );
        SetLayeredWindowAttributes(w, KEY, 0, LWA_COLORKEY);
        WND.store(w as isize, Relaxed);
        w
    } else {
        w
    };
    let r = area(&s);
    // just below the pet, so the hand is in front of it
    SetWindowPos(w, hw(&PET_WND), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    InvalidateRect(w, null(), 0);
    UpdateWindow(w);
}

unsafe fn line(dc: HDC, a: (f32, f32), b: (f32, f32), width: i32, c: u32) {
    let pen = CreatePen(PS_SOLID, width.max(1), c);
    let old = SelectObject(dc, pen);
    MoveToEx(dc, a.0 as i32, a.1 as i32, null_mut());
    LineTo(dc, b.0 as i32, b.1 as i32);
    SelectObject(dc, old);
    DeleteObject(pen);
}

unsafe fn draw(dc: HDC, s: &Strand) {
    let r = area(s);
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    fill(dc, 0, 0, w, h, KEY);
    let at = |p: (f32, f32)| (p.0 - r.left as f32, p.1 - r.top as f32);
    let (a, b) = (at(s.from), at(s.to));
    let k = s.scale.max(0.75);
    let (white, grey) = (rgb(0xfb, 0xfb, 0xff), rgb(0xb9, 0xc0, 0xd0));
    match s.stuff {
        Stuff::Web => {}
        Stuff::Cable => {
            line(dc, a, b, (6.0 * k) as i32, PET_INK);
            line(dc, a, b, (3.0 * k) as i32, rgb(0xb4, 0xbe, 0xd2));
            if s.stuck {
                // a hook
                for (dx, dy) in [(-9.0, 9.0), (9.0, 9.0), (0.0, 12.0)] {
                    line(dc, b, (b.0 + dx * k, b.1 + dy * k), (5.0 * k) as i32, PET_INK);
                    line(dc, b, (b.0 + dx * k, b.1 + dy * k), (2.6 * k) as i32, rgb(0xd0, 0xd6, 0xe2));
                }
            }
            return;
        }
        Stuff::Limb(c, wide) => {
            let w = wide.max(3.0);
            line(dc, a, b, (w + 4.0 * k) as i32, PET_INK);
            line(dc, a, b, w as i32, c);
            if s.stuck {
                fill(dc, (b.0 - w) as i32, b.1 as i32 - 2, (2.0 * w) as i32, w as i32 + 2, PET_INK);
                fill(dc, (b.0 - w) as i32 + 2, b.1 as i32, (2.0 * w) as i32 - 4, w as i32 - 2, c);
            }
            return;
        }
    }
    line(dc, a, b, (6.0 * k) as i32, grey);
    line(dc, a, b, (3.0 * k) as i32, white);
    // it is twisted: little rungs all along
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy).max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let mut d = 18.0 * k;
    while d < len - 10.0 * k {
        let (x, y) = (a.0 + dx / len * d, a.1 + dy / len * d);
        let q = 5.0 * k;
        line(dc, (x - nx * q, y - ny * q), (x + nx * q, y + ny * q), (1.6 * k) as i32, white);
        d += 22.0 * k;
    }
    if s.stuck {
        // where it sticks it spreads into a little web
        let spoke = |i: i32, rad: f32| {
            let t = i as f32 * std::f32::consts::TAU / 10.0;
            (b.0 + t.cos() * rad, b.1 + t.sin() * rad)
        };
        for i in 0..10 {
            line(dc, b, spoke(i, 38.0 * k), (2.0 * k) as i32, white);
            for rad in [13.0, 25.0, 35.0] {
                line(dc, spoke(i, rad * k), spoke(i + 1, rad * k), (1.4 * k) as i32, white);
            }
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let s = *STRAND.lock().unwrap();
            let mut ps: PAINTSTRUCT = zeroed();
            let screen = BeginPaint(hwnd, &mut ps);
            if let Some(s) = s {
                let r = area(&s);
                let (w, h) = (r.right - r.left, r.bottom - r.top);
                // drawn off screen first, or it would flicker
                let dc = CreateCompatibleDC(screen);
                let bmp = CreateCompatibleBitmap(screen, w, h);
                let old = SelectObject(dc, bmp);
                draw(dc, &s);
                BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
                SelectObject(dc, old);
                DeleteObject(bmp);
                DeleteDC(dc);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DPICHANGED => 0,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---- swinging and climbing ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stage {
    // swinging
    Shoot,
    Swing,
    Leap,
    // to the ceiling
    Aim,
    Zip,
    Top,
    Lower,
    Dangle,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ride {
    pub stage: Stage,
    pub t: f32,            // seconds in this stage
    pub anchor: (f32, f32), // where the web sticks
    pub len: f32,
    pub ang: f32, // of the rope, 0 = straight down, right is positive
    pub vel: f32, // radians per second, or (while leaping) unused
    pub left: u32, // swings still to come
    pub dir: f32,
}

/// One step of a pendulum.
pub(crate) fn pendulum(ang: f32, vel: f32, len: f32, g: f32, dt: f32) -> (f32, f32) {
    let vel = vel - g / len.max(40.0) * ang.sin() * dt;
    (ang + vel * dt, vel)
}

/// Where to stick the next web: ahead, on the ceiling, inside the world. The direction turns round
/// when there is no room left.
pub(crate) fn next_anchor(x: f32, dir: f32, tall: f32, span: (f32, f32), top: f32) -> ((f32, f32), f32) {
    let ahead = tall * 0.46;
    let ok = |d: f32| x + d * ahead * 1.75 > span.0 && x + d * ahead * 1.75 < span.1;
    let dir = if ok(dir) || !ok(-dir) { dir } else { -dir };
    (((x + dir * ahead).clamp(span.0, span.1), top), dir)
}

/// What the body does on the web right now.
pub(crate) fn clip_of(s: Stage) -> Clip {
    match s {
        Stage::Shoot | Stage::Swing | Stage::Leap => Clip::Swinging,
        Stage::Aim | Stage::Zip => Clip::Zipping,
        Stage::Top | Stage::Lower | Stage::Dangle => Clip::Hanging,
    }
}

/// Begin: `up` climbs to the ceiling, otherwise it swings away.
pub(crate) fn begin(up: bool, feet: (f32, f32), tall_body: f32, top: f32, floor: f32, span: (f32, f32), dir: f32) -> Ride {
    let hand = (feet.0, feet.1 - tall_body);
    let (anchor, dir) = if up { ((feet.0, top), dir) } else { next_anchor(hand.0, dir, floor - top, span, top) };
    Ride { stage: if up { Stage::Aim } else { Stage::Shoot }, t: 0.0, anchor, len: 0.0, ang: 0.0, vel: 0.0, left: 5, dir }
}

/// What `step` decided.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Out {
    On,
    Done((f32, f32)), // let go, with this speed
}

/// The room there is for a ride.
#[derive(Clone, Copy)]
pub(crate) struct Room {
    pub top: f32,
    pub floor: f32,
    pub span: (f32, f32), // for the middle of the body
    pub body: f32,        // how tall the pet is
    pub scale: f32,
}

/// Advance the ride; moves `feet`, the speed of a leap and the angle the body hangs at (degrees).
pub(crate) fn step(r: &mut Ride, feet: &mut (f32, f32), vel: &mut (f32, f32), swing: &mut f32, dt: f32, w: &Room) -> Out {
    r.t += dt;
    let high = w.floor - w.top;
    let g = 2300.0 * w.scale.max(0.8);
    // the body hangs from the top of its head
    let head = |f: (f32, f32), ang: f32| (f.0 - ang.sin() * w.body, f.1 - ang.cos() * w.body);
    let feet_of = |h: (f32, f32), ang: f32| (h.0 + ang.sin() * w.body, h.1 + ang.cos() * w.body);
    match r.stage {
        Stage::Shoot => {
            // standing, or still flying from the last swing
            if vel.1 != 0.0 || feet.1 < w.floor - 4.0 {
                vel.1 += g * 0.55 * dt;
                feet.0 = (feet.0 + vel.0 * dt).clamp(w.span.0, w.span.1);
                feet.1 = (feet.1 + vel.1 * dt).min(w.floor);
            }
            *swing *= 0.8;
            if r.t >= 0.22 {
                let h = head(*feet, 0.0);
                let d = (h.0 - r.anchor.0, h.1 - r.anchor.1);
                r.len = d.0.hypot(d.1);
                r.ang = d.0.atan2(d.1);
                // whatever speed there was carries on along the rope, plus a push
                let along = vel.0 * r.ang.cos() - vel.1 * r.ang.sin();
                r.vel = along / r.len.max(60.0) + r.dir * 0.9;
                *vel = (0.0, 0.0);
                (r.stage, r.t) = (Stage::Swing, 0.0);
            }
        }
        Stage::Swing => {
            // pulls itself up while swinging, so it clears the ground
            let want = high * 0.6;
            if (feet.0 <= w.span.0 + 1.0 && r.vel < 0.0) || (feet.0 >= w.span.1 - 1.0 && r.vel > 0.0) {
                r.vel *= 0.8; // scraping along the side
            }
            let was = r.len;
            r.len += (want - r.len).clamp(-1500.0 * w.scale * dt, 500.0 * w.scale * dt);
            r.vel *= (was / r.len).clamp(1.0, 1.02);
            (r.ang, r.vel) = pendulum(r.ang, r.vel, r.len, g, dt);
            r.ang = r.ang.clamp(-1.15, 1.15);
            let mut h = (r.anchor.0 + r.ang.sin() * r.len, r.anchor.1 + r.ang.cos() * r.len);
            h.0 = h.0.clamp(w.span.0, w.span.1);
            *feet = feet_of(h, r.ang);
            // neither head nor feet leave the screen
            // (only a wall it is moving towards counts, or it could never swing away from one)
            let wall = (feet.0 < w.span.0 && r.vel < 0.0) || (feet.0 > w.span.1 && r.vel > 0.0);
            feet.0 = feet.0.clamp(w.span.0, w.span.1);
            feet.1 = feet.1.min(w.floor);
            *swing = -r.ang.to_degrees();
            let far = r.dir * r.ang > 0.68 || (r.dir * r.vel <= 0.0 && r.dir * r.ang > 0.1);
            if far || wall || r.t > 3.0 {
                let v = r.vel * r.len;
                *vel = (v * r.ang.cos() * 1.05, -(v * r.ang.sin()).abs() - 240.0 * w.scale);
                if wall {
                    // kicks off the wall
                    r.dir = if feet.0 <= w.span.0 + 1.0 { 1.0 } else { -1.0 };
                    vel.0 = r.dir * (380.0 * w.scale + vel.0.abs() * 0.3);
                }
                (r.stage, r.t) = (Stage::Leap, 0.0);
            }
        }
        Stage::Leap => {
            vel.1 += g * 0.6 * dt;
            feet.0 = (feet.0 + vel.0 * dt).clamp(w.span.0, w.span.1);
            feet.1 = (feet.1 + vel.1 * dt).clamp(w.top + w.body, w.floor);
            if (feet.0 <= w.span.0 && vel.0 < 0.0) || (feet.0 >= w.span.1 && vel.0 > 0.0) {
                // feet against the wall, and off again
                r.dir = if feet.0 <= w.span.0 { 1.0 } else { -1.0 };
                vel.0 = r.dir * (420.0 * w.scale + vel.0.abs() * 0.3);
            }
            // a somersault
            *swing += r.dir * 620.0 * dt;
            if r.t > 0.42 && (vel.1 > 60.0 * w.scale || r.t > 0.8) {
                r.left = r.left.saturating_sub(1);
                if r.left == 0 || feet.1 >= w.floor {
                    return Out::Done((vel.0 * 0.5, vel.1));
                }
                *swing = 0.0;
                (r.anchor, r.dir) = next_anchor(feet.0, r.dir, high, w.span, w.top);
                (r.stage, r.t) = (Stage::Shoot, 0.0);
            }
        }
        Stage::Aim => {
            if r.t >= 0.35 {
                (r.stage, r.t) = (Stage::Zip, 0.0);
            }
        }
        Stage::Zip => {
            let goal = w.top + w.body + 16.0 * w.scale;
            let speed = (500.0 + 2600.0 * r.t) * w.scale;
            feet.1 = (feet.1 - speed * dt).max(goal);
            if feet.1 <= goal {
                (r.stage, r.t) = (Stage::Top, 0.0);
            }
        }
        Stage::Top => {
            *swing = 4.0 * (r.t * 2.4).sin();
            if r.t > 1.6 {
                (r.stage, r.t) = (Stage::Lower, 0.0);
            }
        }
        Stage::Lower | Stage::Dangle => {
            let goal = w.top + w.body + high * 0.42;
            if r.stage == Stage::Lower {
                feet.1 = (feet.1 + 240.0 * w.scale * dt).min(goal);
                if feet.1 >= goal {
                    (r.stage, r.t) = (Stage::Dangle, 0.0);
                }
            } else if r.t > 3.2 {
                return Out::Done((0.0, 40.0));
            }
            // swaying at the end of a long thread
            let a = 0.07 * (r.t * 1.5).sin();
            feet.0 = (r.anchor.0 + a * (feet.1 - w.top)).clamp(w.span.0, w.span.1);
            *swing = -a.to_degrees() * 2.0;
        }
    }
    Out::On
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: Room = Room { top: 0.0, floor: 1048.0, span: (48.0, 1872.0), body: 120.0, scale: 1.0 };

    fn ride(up: bool, x: f32, dir: f32) -> (Vec<(Stage, (f32, f32))>, Ride) {
        let mut feet = (x, ROOM.floor);
        let (mut vel, mut swing) = ((0.0, 0.0), 0.0);
        let mut r = begin(up, feet, ROOM.body, ROOM.top, ROOM.floor, ROOM.span, dir);
        let mut seen = Vec::new();
        for n in 0..3000 {
            let out = step(&mut r, &mut feet, &mut vel, &mut swing, 0.033, &ROOM);
            assert!(feet.0.is_finite() && feet.1.is_finite() && swing.is_finite());
            assert!(feet.1 <= ROOM.floor + 0.5, "through the floor at {feet:?}");
            assert!(feet.0 >= ROOM.span.0 - 130.0 && feet.0 <= ROOM.span.1 + 130.0, "left the screen at {feet:?}");
            seen.push((r.stage, feet));
            if out != Out::On {
                return (seen, r);
            }
            assert!(n < 2500, "never lets go");
        }
        panic!("never lets go")
    }

    #[test]
    fn swinging_covers_ground_high_above_the_floor_and_ends() {
        for (x, dir) in [(960.0, 1.0), (960.0, -1.0), (120.0, -1.0), (1800.0, 1.0)] {
            let (seen, _) = ride(false, x, dir);
            let swings = seen.windows(2).filter(|w| w[0].0 == Stage::Shoot && w[1].0 == Stage::Swing).count();
            assert!(swings >= 3, "only {swings} swings from {x}");
            // never stuck against the side of the screen
            let mut still = 0;
            for p in seen.windows(2) {
                still = if (p[0].1 .0 - p[1].1 .0).abs() < 0.5 { still + 1 } else { 0 };
                assert!(still < 22, "stuck at {:?} from {x}", p[1].1);
            }
            let (lo, hi) = seen.iter().fold((f32::MAX, f32::MIN), |m, s| (m.0.min(s.1 .0), m.1.max(s.1 .0)));
            assert!(hi - lo > 600.0, "only went {} from {x}", hi - lo);
            let highest = seen.iter().map(|s| s.1 .1).fold(f32::MAX, f32::min);
            assert!(highest < 600.0, "stayed down at {highest}");
        }
    }

    #[test]
    fn climbing_reaches_the_top_of_the_screen_and_comes_down_again() {
        let (seen, _) = ride(true, 700.0, 1.0);
        let highest = seen.iter().map(|s| s.1 .1).fold(f32::MAX, f32::min);
        assert!(highest <= ROOM.top + ROOM.body + 20.0, "only got to {highest}");
        for s in [Stage::Zip, Stage::Top, Stage::Lower, Stage::Dangle] {
            assert!(seen.iter().any(|x| x.0 == s), "{s:?} is missing");
        }
        assert!(seen.len() < 500, "takes {} steps", seen.len());
    }

    #[test]
    fn webs_stick_inside_the_world_and_turn_round_at_its_end() {
        let (a, d) = next_anchor(1800.0, 1.0, 1000.0, (48.0, 1872.0), 0.0);
        assert!(d < 0.0 && a.0 < 1800.0 && a.1 == 0.0);
        let (a, d) = next_anchor(500.0, 1.0, 1000.0, (48.0, 1872.0), 0.0);
        assert!(d > 0.0 && a.0 > 500.0);
    }

    #[test]
    fn a_pendulum_keeps_swinging_about_as_high() {
        let (mut a, mut v, mut top) = (0.6f32, 0.0f32, 0.0f32);
        for _ in 0..400 {
            (a, v) = pendulum(a, v, 600.0, 2300.0, 0.016);
            top = top.max(a.abs());
        }
        assert!((0.5..0.75).contains(&top), "{top}");
    }
}
