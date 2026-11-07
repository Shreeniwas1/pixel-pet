//! The stats window.

use crate::prelude::*;

// ======================================================================
// stats window (custom pixel-drawn, like the todo window)
// ======================================================================

pub(crate) const STATS_W: i32 = 480;
pub(crate) const STATS_H: i32 = 530;
pub(crate) const RANGES: [(u32, &str); 3] = [(7, "7D"), (14, "14D"), (30, "30D")];

pub(crate) struct SLay {
    pub close: RECT,
    pub ranges: [RECT; 3],
    pub title_h: i32,
}

pub(crate) fn slay(k: &dyn Fn(i32) -> i32) -> SLay {
    let mut ranges = [NO_RECT; 3];
    for (i, r) in ranges.iter_mut().enumerate() {
        *r = rc(k(STATS_W - 14 - 3 * 44 - 2 * 6 + i as i32 * 50), k(184), k(44), k(24));
    }
    SLay { close: rc(k(STATS_W - 3 - 28), k(3), k(24), k(24)), ranges, title_h: k(30) }
}

/// What the mouse is over: 10 = close, 20..=22 = range buttons, -1 = nothing.
pub(crate) fn stats_hit(k: &dyn Fn(i32) -> i32, x: i32, y: i32) -> i32 {
    let l = slay(k);
    if contains(&l.close, x, y) {
        return 10;
    }
    for (i, r) in l.ranges.iter().enumerate() {
        if contains(r, x, y) {
            return 20 + i as i32;
        }
    }
    -1
}

pub(crate) unsafe fn seg_bar(dc: HDC, x: i32, y: i32, w: i32, h: i32, pct: f32, col: u32, gap: i32) {
    let segs = 24i32;
    let sw = (w - (segs - 1) * gap) / segs;
    let on = ((pct.clamp(0.0, 100.0) / 100.0) * segs as f32).round() as i32;
    for i in 0..segs {
        fill(dc, x + i * (sw + gap), y, sw, h, if i < on { col } else { PANEL_LIGHT });
    }
}

pub(crate) unsafe fn stats_paint(hwnd: HWND) {
    struct Snap {
        dpi: f32,
        happy: f32,
        energy: f32,
        mood: &'static str,
        today: (u32, u32),
        goal: u32,
        streak: u32,
        best_streak: u32,
        week: (u32, u32),
        range: usize,
        days: Vec<u32>,
        day: u32,
        totals: (u32, u32, u32, Option<(u32, u32)>),
        usage: [u64; 4],
    }
    let sn = {
        let s = ST.lock().unwrap();
        let day = (local_now().0 / 1440) as u32;
        let w = s.stats.week(day);
        Snap {
            dpi: s.scale,
            happy: s.mood.happy,
            energy: s.mood.energy,
            mood: s.mood.label(),
            today: s.stats.today(day),
            goal: s.stats.goal,
            streak: s.stats.streak(day),
            best_streak: s.stats.best_streak(),
            week: (w.sessions, w.minutes),
            range: s.stats_range,
            days: s.stats.last_n(day, RANGES[s.stats_range].0),
            day,
            totals: s.stats.totals(),
            usage: [s.day_rx, s.day_tx, s.mon_rx, s.mon_tx],
        }
    };
    let k = |v: i32| (v as f32 * sn.dpi).round() as i32;
    let (w, h) = (k(STATS_W), k(STATS_H));
    let lay = slay(&k);
    let hover = STATS_HOVER.load(Relaxed);

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);

    let b = k(3);
    fill(dc, 0, 0, w, h, ORANGE);
    fill(dc, b, b, w - 2 * b, h - 2 * b, INK);
    fill(dc, b, b, w - 2 * b, lay.title_h - b, ORANGE);

    let mkfont = |px: i32| CreateFontW(-k(px), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    let (f14, f12, f26) = (mkfont(14), mkfont(12), mkfont(26));
    let old_font = SelectObject(dc, f14);

    // title bar + close button
    SetTextColor(dc, INK);
    let mut tr = rc(k(14), 0, k(300), lay.title_h);
    text_at(dc, "PIXEL STATS", &mut tr, DT_SINGLELINE | DT_VCENTER);
    let c = &lay.close;
    fill(dc, c.left, c.top, c.right - c.left, c.bottom - c.top, if hover == 10 { CREAM } else { INK });
    let cp = k(2);
    let cross: Vec<&[u8]> = CROSS.to_vec();
    sprite(dc, &cross, c.left + (c.right - c.left - 7 * cp) / 2, c.top + (c.bottom - c.top - 7 * cp) / 2, cp,
        &|ch| (ch == b'X').then_some(if hover == 10 { INK } else { ORANGE }));

    // three headline cards
    let gap = k(8);
    let cw = (w - 2 * k(14) - 2 * gap) / 3;
    let cards = [
        ("TODAY", format!("{}/{}", sn.today.0, sn.goal), format!("{} MIN FOCUSED", sn.today.1), TOMATO),
        ("STREAK", format!("{}D", sn.streak), format!("BEST EVER {}D", sn.best_streak), GREEN),
        ("THIS WEEK", format!("{}", sn.week.0), format!("{}H {:02}M FOCUSED", sn.week.1 / 60, sn.week.1 % 60), UP_BLUE),
    ];
    for (i, (label, value, sub, accent)) in cards.into_iter().enumerate() {
        let x = k(14) + i as i32 * (cw + gap);
        let y = k(42);
        fill(dc, x, y, cw, k(76), accent);
        fill(dc, x + k(2), y + k(2), cw - k(4), k(72), PANEL);
        SelectObject(dc, f12);
        SetTextColor(dc, DIM);
        let mut r = rc(x + k(10), y + k(6), cw - k(14), k(16));
        text_at(dc, label, &mut r, DT_SINGLELINE | DT_VCENTER);
        SelectObject(dc, f26);
        SetTextColor(dc, accent);
        let mut r = rc(x + k(10), y + k(22), cw - k(14), k(32));
        text_at(dc, &value, &mut r, DT_SINGLELINE | DT_VCENTER);
        SelectObject(dc, f12);
        SetTextColor(dc, HINT);
        let mut r = rc(x + k(10), y + k(54), cw - k(14), k(16));
        text_at(dc, &sub, &mut r, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
    }

    // mood and energy
    SelectObject(dc, f14);
    for (i, (label, pct, col, val)) in [
        ("MOOD", sn.happy, PINK, sn.mood.to_uppercase()),
        ("ENERGY", sn.energy, UP_BLUE, format!("{:.0}%", sn.energy)),
    ]
    .into_iter()
    .enumerate()
    {
        let y = k(126) + i as i32 * k(26);
        SetTextColor(dc, CREAM);
        let mut r = rc(k(14), y, k(80), k(22));
        text_at(dc, label, &mut r, DT_SINGLELINE | DT_VCENTER);
        seg_bar(dc, k(96), y + k(5), w - k(96) - k(110), k(12), pct, col, k(2));
        SetTextColor(dc, col);
        let mut r = rc(w - k(104), y, k(90), k(22));
        text_at(dc, &val, &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
    }

    // chart header with the range switch
    SetTextColor(dc, ORANGE);
    let mut r = rc(k(14), k(184), k(250), k(24));
    text_at(dc, "FOCUS SESSIONS PER DAY", &mut r, DT_SINGLELINE | DT_VCENTER);
    for (i, rect) in lay.ranges.iter().enumerate() {
        let active = i == sn.range;
        let hot = hover == 20 + i as i32;
        fill(dc, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, if active { GREEN } else { PANEL_EDGE });
        fill(dc, rect.left + k(2), rect.top + k(2), rect.right - rect.left - k(4), rect.bottom - rect.top - k(4),
            if active { INK } else if hot { PANEL_LIGHT } else { PANEL });
        SetTextColor(dc, if active { GREEN } else { CREAM });
        let mut rr = *rect;
        text_at(dc, RANGES[i].1, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
    }

    // the bar chart, with a dashed line at the daily goal
    let (cx0, cy0, cw2, ch2) = (k(14), k(214), w - k(28), k(136));
    fill(dc, cx0, cy0, cw2, ch2, PANEL_EDGE);
    fill(dc, cx0 + k(2), cy0 + k(2), cw2 - k(4), ch2 - k(4), PANEL);
    let n = sn.days.len() as i32;
    let pad = k(8);
    let bgap = if n <= 7 { k(12) } else if n <= 14 { k(6) } else { k(2) };
    let bw = ((cw2 - 2 * pad - (n - 1) * bgap) / n).max(2);
    let top_pad = k(18); // room for the value labels
    let plot_h = ch2 - top_pad - k(8);
    let base_y = cy0 + ch2 - k(8);
    let peak = sn.days.iter().copied().max().unwrap_or(0).max(sn.goal).max(1);
    let goal_y = base_y - (plot_h as f32 * sn.goal as f32 / peak as f32).round() as i32;
    let mut dx = cx0 + k(2) + k(4);
    while dx < cx0 + cw2 - k(6) {
        fill(dc, dx, goal_y, k(4), 1.max(k(1)), ALERT_ORANGE);
        dx += k(8);
    }
    SelectObject(dc, f12);
    for (i, v) in sn.days.iter().enumerate() {
        let bx = cx0 + pad + i as i32 * (bw + bgap);
        let bh = (plot_h as f32 * *v as f32 / peak as f32).round() as i32;
        let today = i as i32 == n - 1;
        let col = if today { TOMATO } else if *v >= sn.goal { GREEN } else { UP_BLUE };
        if bh > 0 {
            fill(dc, bx, base_y - bh, bw, bh, col);
            if n <= 14 {
                SetTextColor(dc, col);
                let mut r = rc(bx - k(6), base_y - bh - k(15), bw + k(12), k(14));
                text_at(dc, &v.to_string(), &mut r, DT_SINGLELINE | DT_CENTER);
            }
        } else {
            fill(dc, bx, base_y - 1.max(k(1)), bw, 1.max(k(1)), PANEL_LIGHT);
        }
        // weekday initial under the bar
        let wd = (sn.day + i as u32 + 4 + 7 * 5 - (n as u32 - 1)) % 7;
        SetTextColor(dc, if today { TOMATO } else { DIM });
        let mut r = rc(bx - k(3), cy0 + ch2 + k(2), bw + k(6), k(14));
        text_at(dc, ["S", "M", "T", "W", "T", "F", "S"][wd as usize], &mut r, DT_SINGLELINE | DT_CENTER);
    }
    SetTextColor(dc, DIM);
    let mut r = rc(k(14), cy0 + ch2 + k(18), w - k(28), k(14));
    text_at(dc, &format!("GREEN = GOAL MET ({} PER DAY)   RED = TODAY   DASHED = GOAL", sn.goal), &mut r, DT_SINGLELINE | DT_VCENTER);

    // all-time totals
    let (sessions, minutes, active, best) = sn.totals;
    let best_txt = best.map_or("-".to_string(), |(d, n)| {
        format!("{} ({} SESSIONS)", ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"][((d + 4) % 7) as usize], n)
    });
    let avg = if active > 0 { sessions as f32 / active as f32 } else { 0.0 };
    SelectObject(dc, f14);
    let ty = k(386);
    for (i, (label, val)) in [
        ("ALL TIME", format!("{} SESSIONS, {}H {:02}M", sessions, minutes / 60, minutes % 60)),
        ("ACTIVE DAYS", format!("{}   ({:.1} SESSIONS EACH)", active, avg)),
        ("BEST DAY", best_txt),
    ]
    .into_iter()
    .enumerate()
    {
        let y = ty + i as i32 * k(20);
        SetTextColor(dc, DIM);
        let mut r = rc(k(14), y, k(120), k(20));
        text_at(dc, label, &mut r, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, CREAM);
        let mut r = rc(k(136), y, w - k(150), k(20));
        text_at(dc, &val, &mut r, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
    }

    // data usage
    let uw = (w - 2 * k(14) - gap) / 2;
    for (i, (label, rx, tx)) in [("DATA TODAY", sn.usage[0], sn.usage[1]), ("DATA THIS MONTH", sn.usage[2], sn.usage[3])]
        .into_iter()
        .enumerate()
    {
        let x = k(14) + i as i32 * (uw + gap);
        let y = k(456);
        fill(dc, x, y, uw, k(60), UP_BLUE);
        fill(dc, x + k(2), y + k(2), uw - k(4), k(56), PANEL);
        SelectObject(dc, f12);
        SetTextColor(dc, DIM);
        let mut r = rc(x + k(10), y + k(5), uw - k(14), k(16));
        text_at(dc, label, &mut r, DT_SINGLELINE | DT_VCENTER);
        SelectObject(dc, f14);
        SetTextColor(dc, GREEN);
        let mut r = rc(x + k(10), y + k(21), uw - k(14), k(18));
        text_at(dc, &format!("DOWN  {}", format_bytes(rx)), &mut r, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, UP_BLUE);
        let mut r = rc(x + k(10), y + k(38), uw - k(14), k(18));
        text_at(dc, &format!("UP    {}", format_bytes(tx)), &mut r, DT_SINGLELINE | DT_VCENTER);
    }

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(f14);
    DeleteObject(f12);
    DeleteObject(f26);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

pub(crate) unsafe extern "system" fn stats_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    let k_of = |dpi: f32| move |v: i32| (v as f32 * dpi).round() as i32;
    match msg {
        WM_PAINT => {
            stats_paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_TIMER => {
            if IsWindowVisible(hwnd) != 0 {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_NCHITTEST => {
            let mut p = POINT { x: (lp & 0xFFFF) as i16 as i32, y: ((lp >> 16) & 0xFFFF) as i16 as i32 };
            ScreenToClient(hwnd, &mut p);
            let k = k_of(ST.lock().unwrap().scale);
            let lay = slay(&k);
            if p.y < lay.title_h && !contains(&lay.close, p.x, p.y) {
                2 // HTCAPTION: drag by the title bar
            } else {
                1 // HTCLIENT
            }
        }
        WM_MOUSEMOVE => {
            let k = k_of(ST.lock().unwrap().scale);
            let h = stats_hit(&k, mx, my);
            if STATS_HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let k = k_of(ST.lock().unwrap().scale);
            match stats_hit(&k, mx, my) {
                10 => {
                    ShowWindow(hwnd, SW_HIDE);
                }
                h @ 20..=22 => {
                    ST.lock().unwrap().stats_range = (h - 20) as usize;
                    InvalidateRect(hwnd, null(), 0);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub(crate) unsafe fn show_stats() {
    let mut w = hw(&STATS_WND);
    if w.is_null() {
        let dpi = ST.lock().unwrap().scale;
        let k = |v: i32| (v as f32 * dpi).round() as i32;
        let wa = primary_work();
        let (ww, wh) = (k(STATS_W), k(STATS_H));
        w = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            wide("PixelStatsWnd").as_ptr(),
            wide("Pixel Pet - Stats").as_ptr(),
            WS_POPUP,
            wa.left + (wa.right - wa.left - ww) / 2,
            wa.top + (wa.bottom - wa.top - wh).max(0) / 2,
            ww,
            wh,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        STATS_WND.store(w as isize, Relaxed);
        SetTimer(w, 1, 1000, None); // keep mood and data usage fresh while it is open
    }
    InvalidateRect(w, null(), 0);
    ShowWindow(w, SW_SHOW);
    SetForegroundWindow(w);
}
