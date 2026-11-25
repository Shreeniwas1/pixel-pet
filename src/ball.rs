//! A ball to throw: it lives in a little window of its own, and the pet runs after it and brings it
//! back.

use crate::prelude::*;

pub(crate) struct Ball {
    pub wnd: isize,
    pub out: bool,     // in play
    pub held: bool,    // by the mouse
    pub carried: bool, // by the pet
    pub x: f32,        // the middle of its underside, on the screen
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub turn: f32,
    pub still: bool,
    pub interest: f32, // seconds the pet still cares
    pub home: f32,     // where it should be brought back to
    grab: (f32, f32),
    cursor: (f32, f32),
    size: i32,
}

pub(crate) static BALL: Mutex<Ball> = Mutex::new(Ball {
    wnd: 0,
    out: false,
    held: false,
    carried: false,
    x: 0.0,
    y: 0.0,
    vx: 0.0,
    vy: 0.0,
    turn: 0.0,
    still: true,
    interest: 0.0,
    home: 0.0,
    grab: (0.0, 0.0),
    cursor: (0.0, 0.0),
    size: 0,
});

/// How long a thrown ball is worth running after.
const INTEREST: f32 = 25.0;

pub(crate) const ART: &[&[u8]] = &[b"..KKKK..", b".KRRiiK.", b"KRRRiiiK", b"KRRRiibK", b"KiiiRbbK", b"KiiRRbbK", b".KiRRbK.", b"..KKKK.."];

/// Rolling: the picture in quarter turns.
fn rolled(turn: f32) -> Vec<Vec<u8>> {
    let n = ART.len();
    let mut g: Vec<Vec<u8>> = ART.iter().map(|r| r.to_vec()).collect();
    for _ in 0..(turn / 90.0).rem_euclid(4.0) as usize {
        g = (0..n).map(|j| (0..n).map(|i| g[n - 1 - i][j]).collect()).collect();
    }
    g
}

pub(crate) unsafe fn register(hinst: HINSTANCE) {
    let class = wide("PixelBallWnd");
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), IDC_HAND),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    RegisterClassW(&wc);
}

/// Get the ball out beside the pet, or put it away.
pub(crate) unsafe fn toggle() {
    // towards where there is more room
    let (feet, scale, dir) = {
        let s = ST.lock().unwrap();
        (s.body.feet, pet_scale(&s), if s.flip.0 { 1.0 } else { -1.0 })
    };
    // Windows may call `proc` back while it hides or creates the window, and that takes the lock,
    // so the lock is never held while talking to Windows
    let (out, known) = {
        let mut b = BALL.lock().unwrap();
        let was = b.out;
        if was {
            (b.out, b.held, b.carried) = (false, false, false);
        }
        (was, b.wnd)
    };
    if out {
        ShowWindow(known as HWND, SW_HIDE);
        return;
    }
    if known == 0 {
        let w = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            wide("PixelBallWnd").as_ptr(),
            wide("Pixel Pet - Ball").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), GetModuleHandleW(null()), null(),
        );
        SetLayeredWindowAttributes(w, KEY, 0, LWA_COLORKEY);
        BALL.lock().unwrap().wnd = w as isize;
    }
    let mut b = BALL.lock().unwrap();
    b.out = true;
    (b.held, b.carried, b.still) = (false, false, false);
    (b.x, b.y) = (feet.0, feet.1 - 130.0 * scale);
    (b.vx, b.vy) = (dir * 640.0 * scale, -420.0 * scale);
    b.home = feet.0;
    b.interest = INTEREST;
}

/// Bring it to the pet's feet again (it was carried back).
pub(crate) fn put_down(x: f32, y: f32, dir: f32) {
    let mut b = BALL.lock().unwrap();
    b.carried = false;
    b.still = false;
    b.interest = 0.0;
    (b.x, b.y, b.vx, b.vy) = (x, y - 12.0, dir * 60.0, -150.0);
}

/// 30 times a second, from the pet's timer.
pub(crate) unsafe fn step(pet: HWND, dt: f32) {
    let (screens, scale, gravity, sound_at) = {
        let s = ST.lock().unwrap();
        if !BALL.lock().unwrap().out {
            return;
        }
        (s.body.screens.clone(), pet_scale(&s), 1900.0 * pet_scale(&s).max(0.8), 420.0)
    };
    let mut p: POINT = zeroed();
    GetCursorPos(&mut p);
    let (w, carried) = {
        let b = BALL.lock().unwrap();
        (b.wnd as HWND, b.carried)
    };
    let show = IsWindowVisible(pet) != 0 && !carried;
    if (IsWindowVisible(w) != 0) != show {
        ShowWindow(w, if show { SW_SHOWNOACTIVATE } else { SW_HIDE }); // see `toggle` about the lock
    }
    let mut b = BALL.lock().unwrap();
    let size = ((ART.len() as i32 * 4) as f32 * scale).round() as i32;
    let (half, tall) = (size as f32 / 2.0, size as f32);
    b.interest = (b.interest - dt).max(0.0);
    if b.carried || !show {
        return;
    }
    let before = (b.x, b.y, b.turn);
    if b.held {
        let cursor = (p.x as f32, p.y as f32);
        if dt > 0.0 {
            let v = ((cursor.0 - b.cursor.0) / dt, (cursor.1 - b.cursor.1) / dt);
            (b.vx, b.vy) = (b.vx * 0.5 + v.0 * 0.5, b.vy * 0.5 + v.1 * 0.5);
        }
        b.cursor = cursor;
        (b.x, b.y) = (cursor.0 + b.grab.0, cursor.1 + b.grab.1);
    } else if !b.still {
        let mut f = body::Flight { x: b.x, y: b.y, vx: b.vx, vy: b.vy };
        let wa = work_at(f.x as i32, f.y as i32 - 4);
        let mut area = wa;
        let mid = f.y - half;
        if let Some((l, r)) = world::span(&screens, f.x, mid) {
            (area.left, area.right) = (l, r);
        }
        if let Some(y) = world::floor_below(&screens, f.x, f.y, None) {
            area.bottom = y;
        }
        if let Some(y) = world::ceiling(&screens, f.x, mid) {
            area.top = y;
        }
        let hit_x = f.vx;
        let end = body::bounce(&mut f, dt, gravity, &area, half, tall);
        if (f.vy < 0.0 && b.vy > sound_at) || (hit_x * f.vx < 0.0 && hit_x.abs() > sound_at) {
            sound::play(Sound::Ball);
        }
        b.turn += f.vx * dt * 1.6;
        (b.x, b.y, b.vx, b.vy) = (f.x, f.y, f.vx, f.vy);
        b.still = end;
    }
    let moved = before != (b.x, b.y, b.turn);
    let resized = b.size != size;
    b.size = size;
    let (x, y) = ((b.x - half) as i32, (b.y - tall) as i32);
    let rolled = (before.2 / 90.0).floor() != (b.turn / 90.0).floor();
    drop(b);
    if moved || resized {
        SetWindowPos(w, HWND_TOPMOST, x, y, size, size, SWP_NOACTIVATE);
    }
    if rolled || resized {
        InvalidateRect(w, null(), 0);
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let (turn, size) = {
                let b = BALL.lock().unwrap();
                (b.turn, b.size)
            };
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            fill(dc, 0, 0, size, size, KEY);
            let g = rolled(turn);
            let rows: Vec<&[u8]> = g.iter().map(|r| &r[..]).collect();
            sprite(dc, &rows, 0, 0, (size / ART.len() as i32).max(1), &pet_color);
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DPICHANGED => 0,
        WM_LBUTTONDOWN => {
            SetCapture(hwnd);
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let mut b = BALL.lock().unwrap();
            b.held = true;
            b.cursor = (p.x as f32, p.y as f32);
            b.grab = (b.x - p.x as f32, b.y - p.y as f32);
            (b.vx, b.vy) = (0.0, 0.0);
            0
        }
        WM_LBUTTONUP | WM_CAPTURECHANGED => {
            let was = {
                let mut b = BALL.lock().unwrap();
                let was = b.held;
                if was {
                    b.held = false;
                    b.still = false;
                    b.home = b.x;
                    b.interest = INTEREST;
                    let max = 2600.0;
                    (b.vx, b.vy) = (b.vx.clamp(-max, max), b.vy.clamp(-max, max));
                }
                was
            };
            if msg == WM_LBUTTONUP {
                ReleaseCapture();
                if was {
                    sound::play(Sound::Whoosh);
                    let mut s = ST.lock().unwrap();
                    s.body.ignored = 0.0;
                    s.mood.wake();
                }
            }
            0
        }
        WM_RBUTTONUP => {
            toggle(); // put it away
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_turns_the_picture_and_four_turns_bring_it_back() {
        assert_eq!(rolled(0.0), rolled(360.0));
        assert_ne!(rolled(0.0), rolled(90.0));
        for t in [0.0, 90.0, 180.0, 270.0, -90.0] {
            let g = rolled(t);
            let (mut a, mut b): (Vec<u8>, Vec<u8>) = (g.concat(), ART.concat());
            a.sort();
            b.sort();
            assert_eq!(a, b);
        }
        assert!(ART.iter().all(|r| r.len() == ART.len() && r.iter().all(|c| *c == b'.' || pet_color(*c).is_some())));
    }
}
