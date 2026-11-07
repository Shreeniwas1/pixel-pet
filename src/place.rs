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

/// Which corner of its monitor's work area the pet window sits in.
pub(crate) unsafe fn flips(hwnd: HWND) -> (bool, bool) {
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    ((wr.left + wr.right) < (wa.left + wa.right), (wr.top + wr.bottom) < (wa.top + wa.bottom))
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
    let (scale, pos, focus, menu) = {
        let s = ST.lock().unwrap();
        (eff_scale(&s), s.pos, s.focus, s.menu)
    };
    let (w, h) = win_dims(scale, focus, menu);
    let (x, y) = match pos {
        Some((x, y)) => {
            let wa = work_at(x + w / 2, y + h / 2);
            clamp_in(&wa, x, y, w, h)
        }
        None => {
            let wa = primary_work();
            let m = (MARGIN as f32 * scale).round() as i32;
            (wa.right - w - m, wa.bottom - h - m)
        }
    };
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
}

pub(crate) unsafe fn put_in_corner(hwnd: HWND, right: bool, bottom: bool, wa: RECT) {
    let (w, h, m) = {
        let s = ST.lock().unwrap();
        let (w, h) = win_dims_s(&s);
        (w, h, (MARGIN as f32 * eff_scale(&s)).round() as i32)
    };
    let x = if right { wa.right - w - m } else { wa.left + m };
    let y = if bottom { wa.bottom - h - m } else { wa.top + m };
    {
        let mut s = ST.lock().unwrap();
        s.pos = Some((x, y));
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
    let (fx, fy) = flips(hwnd);
    put_in_corner(hwnd, !fx, !fy, next);
}

/// Change a setting that alters the window size, keeping the pet's corner where it is.
pub(crate) unsafe fn reanchor(hwnd: HWND, change: impl FnOnce(&mut State)) {
    let (fx, fy) = flips(hwnd);
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
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe fn set_size(hwnd: HWND, idx: usize) {
    reanchor(hwnd, |s| s.size = idx);
}
