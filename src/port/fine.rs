//! A Mac measures in points, and a Retina screen has two dots to a point. Everything here is drawn in
//! dots, or the words would be drawn at half the size and stretched.
//!
//! It is two everywhere, so that nothing changes when a window goes to a screen that is not Retina.

use windows_sys::Win32::Foundation::RECT;

pub(super) const DOTS: f64 = 2.0;

pub(super) fn dot(x: f64, y: f64) -> (i32, i32) {
    ((x * DOTS).floor() as i32, (y * DOTS).floor() as i32)
}

/// Points, from the top of the screen down, as dots.
pub(super) fn dots(x: f64, top: f64, w: f64, h: f64) -> RECT {
    let n = |v: f64| (v * DOTS).round() as i32;
    RECT { left: n(x), top: n(top), right: n(x + w), bottom: n(top + h) }
}

/// Left, top, width and height of the window that shows these dots. A window starts at a whole point
/// and has room for all of them, so that no dot falls between two of the screen.
pub(super) fn points(r: RECT) -> (f64, f64, f64, f64) {
    let (x, top) = ((r.left as f64 / DOTS).floor(), (r.top as f64 / DOTS).floor());
    let size = |n: i32| (n.max(1) as f64 / DOTS).ceil();
    (x, top, size(r.right - r.left), size(r.bottom - r.top))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT { left, top, right, bottom }
    }

    #[test]
    fn a_screen_has_twice_as_many_dots_as_points() {
        let s = dots(0.0, 25.0, 1440.0, 875.0);
        assert_eq!((s.left, s.top, s.right, s.bottom), (0, 50, 2880, 1800));
        assert_eq!(dot(-10.5, 20.25), (-21, 40));
    }

    #[test]
    fn a_window_is_at_whole_points_and_holds_every_dot() {
        for left in -5..5 {
            for w in 1..6 {
                let (x, top, pw, ph) = points(r(left, 7, left + w, 7 + w));
                for v in [x, top, pw, ph] {
                    assert_eq!(v, v.trunc());
                }
                assert!(pw * DOTS >= w as f64 && ph * DOTS >= w as f64);
                assert!((x * DOTS - left as f64).abs() < DOTS, "it is hardly moved");
            }
        }
    }

    #[test]
    fn there_and_back() {
        let a = r(-200, 40, 560, 1368);
        let (x, top, w, h) = points(a);
        let b = dots(x, top, w, h);
        assert_eq!((a.left, a.top, a.right, a.bottom), (b.left, b.top, b.right, b.bottom));
    }
}
