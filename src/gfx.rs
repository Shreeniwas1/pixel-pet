//! Small GDI drawing helpers shared by every window.

use crate::prelude::*;

// ======================================================================
// drawing
// ======================================================================

pub(crate) unsafe fn fill(dc: HDC, x: i32, y: i32, w: i32, h: i32, c: u32) {
    SetDCBrushColor(dc, c);
    let r = RECT { left: x, top: y, right: x + w, bottom: y + h };
    FillRect(dc, &r, GetStockObject(DC_BRUSH));
}

pub(crate) unsafe fn sprite(dc: HDC, rows: &[&[u8]], x: i32, y: i32, px: i32, pal: &dyn Fn(u8) -> Option<u32>) {
    for (j, row) in rows.iter().enumerate() {
        for (i, &ch) in row.iter().enumerate() {
            if let Some(c) = pal(ch) {
                fill(dc, x + i as i32 * px, y + j as i32 * px, px, px, c);
            }
        }
    }
}

/// A small pixel button with a centred label (menu title bar).
pub(crate) unsafe fn pixel_btn(dc: HDC, r: RECT, label: &str, bg: u32) {
    fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, theme().panel_edge);
    fill(dc, r.left + 2, r.top + 2, r.right - r.left - 4, r.bottom - r.top - 4, bg);
    SetTextColor(dc, CREAM);
    let mut rr = r;
    text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
}

pub(crate) fn rc(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

pub(crate) unsafe fn text_at(dc: HDC, s: &str, r: &mut RECT, flags: u32) {
    let t = wide(s);
    DrawTextW(dc, t.as_ptr(), -1, r, flags | DT_NOPREFIX);
}

pub(crate) unsafe fn pixel_button(dc: HDC, r: &RECT, label: &str, bg: u32, hot: bool) {
    let e = 2;
    let (fill_c, text_c) = if hot { (CREAM, theme().ink) } else { (bg, CREAM) };
    fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, if hot { theme().accent } else { theme().panel_edge });
    fill(dc, r.left + e, r.top + e, r.right - r.left - 2 * e, r.bottom - r.top - 2 * e, fill_c);
    SetTextColor(dc, text_c);
    let mut rr = *r;
    text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
}
