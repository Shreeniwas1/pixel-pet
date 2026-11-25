//! Where the pet window lives: monitors, corners, dragging, resizing.

use crate::prelude::*;

// ======================================================================
// window placement, monitors
// ======================================================================

pub(crate) unsafe fn monitor_work(mon: HMONITOR) -> RECT {
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(mon, &mut mi);
    mi.rcWork
}

pub(crate) unsafe fn work_at(x: i32, y: i32) -> RECT {
    monitor_work(MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST))
}

pub(crate) unsafe fn primary_work() -> RECT {
    monitor_work(MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY))
}

pub(crate) unsafe extern "system" fn enum_mon(m: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    (*(lp as *mut Vec<RECT>)).push(monitor_work(m));
    1
}

pub(crate) unsafe fn all_work_areas() -> Vec<RECT> {
    let mut v: Vec<RECT> = Vec::new();
    EnumDisplayMonitors(null_mut(), null(), Some(enum_mon), &mut v as *mut _ as LPARAM);
    v
}

pub(crate) fn clamp_in(wa: &RECT, x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    (x.clamp(wa.left, (wa.right - w).max(wa.left)), y.clamp(wa.top, (wa.bottom - h).max(wa.top)))
}

/// Which corner of its window the pet sits in: (left, top).
pub(crate) fn flips() -> (bool, bool) {
    ST.lock().unwrap().flip
}

/// Recompute the size cap from the monitor the pet is on (call after display changes).
pub(crate) unsafe fn refit(hwnd: HWND) {
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    ST.lock().unwrap().max_scale = rules::max_scale(wa.bottom - wa.top, FOCUS_H.max(MENU_H));
}

pub(crate) unsafe fn place(hwnd: HWND) {
    refit(hwnd);
    let (mut g, pos, placed) = {
        let s = ST.lock().unwrap();
        (geo(&s), s.pos, s.placed)
    };
    let (w, h) = (g.w, g.h);
    let (x, y) = match pos {
        Some((x, y)) => {
            let wa = work_at(x + w / 2, y + h / 2);
            if !placed {
                g.flip = (2 * x + w < wa.left + wa.right, 2 * y + h < wa.top + wa.bottom);
            }
            let b = g.bounds(&wa);
            let (x, y) = clamp_in(&b, x, y, w, h);
            // close to the floor means on the floor
            let gap = wa.bottom - (y + g.feet().1);
            (x, if gap > 0 && gap < g.pad_top { y + gap } else { y })
        }
        None => {
            let wa = primary_work();
            (wa.right - w + g.pad_x - g.edge, wa.bottom - h + g.pad_bottom)
        }
    };
    {
        let mut s = ST.lock().unwrap();
        s.flip = g.flip;
        s.placed = true;
        s.pos = Some((x, y));
    }
    SetWindowPos(hwnd, null_mut(), x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER); // z-order: presence::sync
}

pub(crate) unsafe fn put_in_corner(hwnd: HWND, right: bool, bottom: bool, wa: RECT) {
    {
        let mut s = ST.lock().unwrap();
        s.flip = (!right, !bottom);
        s.gravity &= bottom; // it cannot stay up there otherwise
        s.placed = true;
        let g = geo(&s);
        let b = g.bounds(&wa);
        s.pos = Some((if right { b.right - g.w } else { b.left }, if bottom { b.bottom - g.h } else { b.top }));
        s.body.stop();
        save_cfg(&s);
    }
    place(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe fn next_monitor(hwnd: HWND) {
    let areas = all_work_areas();
    if areas.len() < 2 {
        return;
    }
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let cur = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    let idx = areas.iter().position(|a| a.left == cur.left && a.top == cur.top).unwrap_or(0);
    let next = areas[(idx + 1) % areas.len()];
    let (fx, fy) = flips();
    put_in_corner(hwnd, !fx, !fy, next);
}

/// Change a setting that alters the window size, keeping the pet's corner where it is.
pub(crate) unsafe fn reanchor(hwnd: HWND, change: impl FnOnce(&mut State)) {
    let (fx, fy) = flips();
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    {
        let mut s = ST.lock().unwrap();
        change(&mut s);
        let (w, h) = win_dims_s(&s);
        let x = if fx { wr.left } else { wr.right - w };
        let y = if fy { wr.top } else { wr.bottom - h };
        s.pos = Some((x, y));
        save_cfg(&s);
    }
    place(hwnd);
    presence::sync(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe fn set_size(hwnd: HWND, idx: usize) {
    reanchor(hwnd, |s| s.size = idx);
}
