//! The pet wanders along the bottom of the screen now and then when it has nothing to do.

use crate::prelude::*;

/// Pixels moved per 50 ms step at 100% size.
pub(crate) const STEP: i32 = 3;

/// One step along a stroll. Returns the new x and the (possibly flipped) direction, turning
/// around at the edges of the work area. Pure so the edge cases are tested.
pub(crate) fn step_x(x: i32, w: i32, left: i32, right: i32, dir: i32, step: i32) -> (i32, i32) {
    let max = (right - w).max(left);
    let nx = x + dir * step;
    if nx <= left {
        (left, 1)
    } else if nx >= max {
        (max, -1)
    } else {
        (nx, dir)
    }
}

/// A cheap pseudo-random number from the tick counter (no crate needed).
fn rnd(n: u32) -> u32 {
    let t = unsafe { GetTickCount() };
    (t ^ n.wrapping_mul(2654435761)) >> 4
}

/// Should the pet be walking right now? Never while anything else wants attention.
fn calm(s: &State) -> bool {
    !s.menu && !s.focus && !s.dragging && !s.down && !s.quiet && s.alert.is_none() && s.msg_left == 0 && s.fact_left == 0 && s.intro == intro::DONE && s.walk_on
}

/// Called every half second: occasionally begin a stroll.
pub(crate) unsafe fn maybe_start(_hwnd: HWND) {
    let mut s = ST.lock().unwrap();
    if s.stroll != 0 {
        return;
    }
    if s.stroll_cd > 0 {
        s.stroll_cd -= 1;
        return;
    }
    if !calm(&s) || s.mood.act() != Pose::Idle {
        return;
    }
    let r = rnd(s.tick);
    s.stroll = if r % 2 == 0 { 1 } else { -1 };
    s.stroll_left = 40 + (r % 60) as i32; // 2 to 5 seconds of walking
    s.stroll_cd = 120 + (r % 240); // next one in 1 to 3 minutes
}

/// 50 ms timer: move the window one step if a stroll is under way.
pub(crate) unsafe fn on_walk(hwnd: HWND) {
    let (dir, scale) = {
        let mut s = ST.lock().unwrap();
        if s.stroll == 0 {
            return;
        }
        if !calm(&s) || s.stroll_left <= 0 {
            s.stroll = 0;
            save_cfg(&s);
            return;
        }
        s.stroll_left -= 1;
        (s.stroll, eff_scale(&s))
    };
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let (w, h) = (wr.right - wr.left, wr.bottom - wr.top);
    let wa = work_at(wr.left + w / 2, wr.top + h / 2);
    let step = ((STEP as f32) * scale).round().max(1.0) as i32;
    let (nx, nd) = step_x(wr.left, w, wa.left, wa.right, dir, step);
    {
        let mut s = ST.lock().unwrap();
        s.stroll = nd;
        s.pos = Some((nx, wr.top));
    }
    SetWindowPos(hwnd, HWND_TOPMOST, nx, wr.top, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_in_direction() {
        assert_eq!(step_x(100, 50, 0, 1000, 1, 3), (103, 1));
        assert_eq!(step_x(100, 50, 0, 1000, -1, 3), (97, -1));
    }

    #[test]
    fn turns_at_edges() {
        assert_eq!(step_x(2, 50, 0, 1000, -1, 3), (0, 1));
        assert_eq!(step_x(949, 50, 0, 1000, 1, 3), (950, -1));
    }

    #[test]
    fn window_wider_than_area_stays_put() {
        let (x, _) = step_x(0, 2000, 0, 1000, 1, 3);
        assert_eq!(x, 0);
    }
}
