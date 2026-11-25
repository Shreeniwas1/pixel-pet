//! The tops of other programs' windows, for the pet to walk on.

use crate::prelude::*;
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};

/// A stretch of a window's top edge that can be seen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Perch {
    pub wnd: isize,
    pub l: i32,
    pub r: i32,
    pub y: i32,
}

/// Shorter ledges than this are not worth standing on.
const SHORTEST: i32 = 150;

// ---- rules (pure, so they can be tested) ----

/// The parts of a window's top edge that nothing in front of it covers. `over` are the windows in
/// front, `screens` where there is anything to see.
pub(crate) fn ledges(wnd: isize, top: RECT, over: &[RECT], screens: &[Screen], headroom: i32) -> Vec<Perch> {
    let y = top.top;
    let mut parts: Vec<(i32, i32)> = screens
        .iter()
        .filter(|s| y > s.work.top + headroom && y < s.work.bottom - headroom / 2)
        .map(|s| (top.left.max(s.work.left), top.right.min(s.work.right)))
        .collect();
    for o in over.iter().filter(|o| o.top <= y && o.bottom > y) {
        parts = parts.into_iter().flat_map(|(l, r)| [(l, r.min(o.left)), (l.max(o.right), r)]).collect();
    }
    parts.into_iter().filter(|(l, r)| r - l >= SHORTEST).map(|(l, r)| Perch { wnd, l, r, y }).collect()
}

/// The ledge right under these feet.
pub(crate) fn under(perches: &[Perch], x: f32, y: f32) -> Option<Perch> {
    perches.iter().copied().find(|p| x as i32 >= p.l && (x as i32) < p.r && (p.y as f32 - y).abs() <= 4.0)
}

/// The first ledge something falling from here would meet.
pub(crate) fn below(perches: &[Perch], x: f32, y: f32) -> Option<i32> {
    perches.iter().filter(|p| x as i32 >= p.l && (x as i32) < p.r && p.y as f32 >= y - 1.0).map(|p| p.y).min()
}

// ---- asking Windows ----

/// Where a window really is (without its invisible borders), if it can be seen.
pub(crate) unsafe fn place_of(w: HWND) -> Option<RECT> {
    if IsWindowVisible(w) == 0 || IsIconic(w) != 0 {
        return None;
    }
    let mut cloaked = 0u32;
    DwmGetWindowAttribute(w, DWMWA_CLOAKED as u32, &mut cloaked as *mut _ as *mut _, 4);
    let mut r: RECT = zeroed();
    let ok = DwmGetWindowAttribute(w, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut r as *mut _ as *mut _, size_of::<RECT>() as u32) >= 0;
    (cloaked == 0 && ok && r.right - r.left > 40 && r.bottom - r.top > 40).then_some(r)
}

struct Scan<'a> {
    screens: &'a [Screen],
    headroom: i32,
    over: Vec<RECT>,
    out: Vec<Perch>,
}

unsafe extern "system" fn visit(w: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Scan);
    if IsWindowVisible(w) == 0 {
        return 1;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(w, &mut pid);
    if pid == windows_sys::Win32::System::Threading::GetCurrentProcessId() {
        return 1;
    }
    let ex = GetWindowLongW(w, GWL_EXSTYLE) as u32;
    // click-through overlays cover nothing
    if ex & WS_EX_TRANSPARENT != 0 {
        return 1;
    }
    let mut cls = [0u16; 32];
    let n = GetClassNameW(w, cls.as_mut_ptr(), 32).max(0) as usize;
    let class = String::from_utf16_lossy(&cls[..n]);
    if class.starts_with("Pixel") || matches!(&class[..], "Progman" | "WorkerW" | "Windows.UI.Core.CoreWindow") {
        return 1;
    }
    let Some(r) = place_of(w) else { return 1 };
    let titled = GetWindowTextLengthW(w) > 0;
    if titled && ex & WS_EX_TOOLWINDOW == 0 && IsZoomed(w) == 0 && !class.starts_with("Shell_") {
        let found = ledges(w as isize, r, &s.over, s.screens, s.headroom);
        s.out.extend(found);
    }
    if s.over.len() < 40 {
        s.over.push(r);
    }
    1
}

/// All ledges there are right now, topmost window first.
pub(crate) unsafe fn scan(screens: &[Screen], headroom: i32) -> Vec<Perch> {
    let mut s = Scan { screens, headroom, over: Vec::new(), out: Vec::new() };
    EnumWindows(Some(visit), &mut s as *mut _ as LPARAM);
    s.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Vec<Screen> {
        let full = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
        vec![Screen { full, work: RECT { bottom: 1040, ..full } }]
    }

    #[test]
    fn a_window_in_the_open_is_one_long_ledge() {
        let l = ledges(7, rc(300, 400, 800, 500), &[], &world(), 120);
        assert_eq!(l, vec![Perch { wnd: 7, l: 300, r: 1100, y: 400 }]);
    }

    #[test]
    fn windows_in_front_cut_the_ledge_and_hide_short_ends() {
        let l = ledges(7, rc(300, 400, 800, 500), &[rc(600, 300, 250, 400)], &world(), 120);
        assert_eq!(l.iter().map(|p| (p.l, p.r)).collect::<Vec<_>>(), [(300, 600), (850, 1100)]);
        let l = ledges(7, rc(300, 400, 800, 500), &[rc(400, 300, 900, 400)], &world(), 120);
        assert!(l.is_empty(), "100 px are too short: {l:?}");
        let l = ledges(7, rc(300, 400, 800, 500), &[rc(600, 450, 250, 400)], &world(), 120);
        assert_eq!(l.len(), 1, "a window below the edge does not cover it");
    }

    #[test]
    fn edges_without_room_to_stand_do_not_count() {
        assert!(ledges(7, rc(300, 60, 800, 500), &[], &world(), 120).is_empty(), "too close to the top");
        assert!(ledges(7, rc(300, 1010, 800, 500), &[], &world(), 120).is_empty(), "practically the floor");
        assert!(ledges(7, rc(-900, 400, 800, 500), &[], &world(), 120).is_empty(), "off the screen");
    }

    #[test]
    fn falling_meets_the_nearest_ledge_below() {
        let p = [Perch { wnd: 1, l: 100, r: 500, y: 700 }, Perch { wnd: 2, l: 100, r: 500, y: 400 }, Perch { wnd: 3, l: 600, r: 900, y: 300 }];
        assert_eq!(below(&p, 200.0, 100.0), Some(400));
        assert_eq!(below(&p, 200.0, 500.0), Some(700));
        assert_eq!(below(&p, 200.0, 800.0), None);
        assert_eq!(below(&p, 550.0, 100.0), None);
        assert_eq!(under(&p, 200.0, 402.0).map(|p| p.wnd), Some(2));
        assert_eq!(under(&p, 200.0, 500.0), None);
    }
}
