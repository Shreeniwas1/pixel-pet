//! Painting the pet window: pet, bubbles, focus cards and the pixel hub menu.

use crate::prelude::*;

pub(crate) unsafe fn paint(hwnd: HWND) {
    let (scale, tick, speed, up, show_up, bits, fact, alert, pet, pose) = {
        let s = ST.lock().unwrap();
        let alert = s.alert.and_then(|id| s.todos.iter().find(|t| t.id == id)).map(|t| t.text.clone());
        let fact = if alert.is_some() {
            None
        } else if s.msg_left > 0 {
            Some(s.msg.clone())
        } else if s.fact_left > 0 && !s.focus {
            Some(fact_text(s.fact))
        } else {
            None
        };
        (eff_scale(&s), s.tick, s.speed, s.up, s.show_up, s.bits, fact, alert, &PETS[s.pet], s.mood.act())
    };
    struct FocusSnap {
        focus: bool,
        tasks: Vec<(u32, String, Option<i64>)>,
        more: usize,
        phase: u8,
        remain: u64,
        total: u64,
        running: bool,
        done: u32,
        goal: u32,
    }
    let fs = {
        let s = ST.lock().unwrap();
        let open: Vec<&Todo> = s.todos.iter().filter(|t| !t.done).collect();
        FocusSnap {
            focus: s.focus,
            tasks: open.iter().take(5).map(|t| (t.id, t.text.clone(), t.due)).collect(),
            more: open.len().saturating_sub(5),
            phase: s.pomo_phase,
            remain: pomo_remaining(&s),
            total: pomo_len(&s),
            running: s.pomo_running,
            done: s.stats.today((local_now().0 / 1440) as u32).0,
            goal: s.stats.goal,
        }
    };
    let ms = hub_view();
    let fact = if ms.open { None } else { fact }; // the hub has the stage
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let (w, h) = win_dims(scale, fs.focus, ms.open);
    let (flipx, flipy) = flips(hwnd);

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);
    fill(dc, 0, 0, w, h, KEY);

    let font = CreateFontW(
        -u(13), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3 /* NONANTIALIASED */, 49, wide("Consolas").as_ptr(),
    );
    let old_font = SelectObject(dc, font);

    // --- pet (bobs every tick, blinks now and then); sits in whichever corner the window is in ---
    let px = u(PX);
    let (gh, gw) = (pet.grid.len(), pet.grid[0].len());
    let (cw, ch) = (gw as i32 * px, gh as i32 * px);
    let cx = if flipx { u(8) } else { w - cw - u(8) };
    let base_y = if flipy { u(8) } else { h - ch - u(8) };
    // body language: how the pet moves depends on what it is doing
    let bob = match pose {
        Pose::Idle if fs.focus => 0, // holds still while you work
        Pose::Idle => if tick % 2 == 0 { 0 } else { -px },
        Pose::Sleep => if (tick / 3) % 2 == 0 { 0 } else { -px / 3 }, // slow breathing
        Pose::Cheer => if tick % 2 == 0 { -2 * px } else { 0 },       // jumping for joy
        Pose::Love => if tick % 2 == 0 { -px / 2 } else { 0 },
        Pose::Stretch => -px,
        Pose::Tired | Pose::Sad => px / 3, // slumped
    };
    let cy = base_y + bob;
    let face_c = pet_color(pet.face).unwrap_or(CREAM);
    let closed = matches!(pose, Pose::Sleep | Pose::Cheer | Pose::Love | Pose::Stretch);
    let droopy = matches!(pose, Pose::Tired | Pose::Sad);
    let blink = tick % 16 == 0 && !fs.focus && pose == Pose::Idle;
    let mut grid: Vec<Vec<u8>> = pet.grid.iter().map(|r| r.to_vec()).collect();
    // eyes follow the mouse while the pet is awake
    if pose == Pose::Idle || droopy {
        let mut cp: POINT = zeroed();
        GetCursorPos(&mut cp);
        let mut wr: RECT = zeroed();
        GetWindowRect(hwnd, &mut wr);
        let dx = cp.x - (wr.left + cx + cw / 2);
        let dir: i32 = if dx > u(120) { 1 } else if dx < -u(120) { -1 } else { 0 };
        if dir != 0 {
            let cols: Vec<usize> = if dir > 0 { (0..gw).rev().collect() } else { (0..gw).collect() };
            for j in 0..gh {
                for &i in &cols {
                    let t = i as i32 + dir;
                    if grid[j][i] == b'E' && t >= 0 && (t as usize) < gw && grid[j][t as usize] == pet.face {
                        grid[j][i] = pet.face;
                        grid[j][t as usize] = b'E';
                    }
                }
            }
        }
    }
    if blink {
        // eyes drop one row (or close) while blinking
        for j in 0..gh {
            for i in 0..gw {
                if pet.grid[j][i] == b'E' {
                    grid[j][i] = pet.face;
                    if j + 1 < gh && pet.grid[j + 1][i] == pet.face {
                        grid[j + 1][i] = b'E';
                    }
                }
            }
        }
    }
    let rows: Vec<&[u8]> = grid.iter().map(|r| &r[..]).collect();
    sprite(dc, &rows, cx, cy, px, &pet_color);
    // eyelids: closed eyes are a thin line, tired eyes get a heavy upper lid
    for j in 0..gh {
        for i in 0..gw {
            if grid[j][i] != b'E' {
                continue;
            }
            let (ex, ey) = (cx + i as i32 * px, cy + j as i32 * px);
            if closed {
                fill(dc, ex, ey, px, px, face_c);
                fill(dc, ex, ey + px * 2 / 3, px, (px / 3).max(1), INK);
            } else if droopy {
                fill(dc, ex, ey, px, px / 2, face_c);
            }
        }
    }
    // little extras that show the mood: z's, hearts, sparkles, a tear
    {
        let ap = u(3).max(2);
        let off = u(2) + if tick % 4 < 2 { 0 } else { u(4) };
        let ox = cx + cw / 2;
        let oy = if flipy { cy + ch + u(4) + off } else { cy - u(4) - off };
        let up_s = |h: i32| if flipy { oy } else { oy - h };
        match pose {
            Pose::Sleep => {
                SetTextColor(dc, UP_BLUE);
                for (k, z) in ["z", "Z"].iter().enumerate() {
                    let zy = if flipy { cy + ch + u(4) + off / 2 + k as i32 * u(10) } else { cy - u(18) - off / 2 - k as i32 * u(10) };
                    let mut zr = RECT { left: ox + u(14) + k as i32 * u(12), top: zy, right: ox + u(40) + k as i32 * u(12), bottom: zy + u(16) };
                    text_at(dc, z, &mut zr, DT_SINGLELINE);
                }
            }
            Pose::Love => {
                let h = 5 * ap;
                for k in 0..2 {
                    let hx = ox - u(14) + k * u(22);
                    sprite(dc, &HEART, hx, up_s(h) - k * u(6), ap, &|c| (c == b'X').then_some(PINK));
                }
            }
            Pose::Cheer => {
                let col = rgb(0xff, 0xe0, 0x66);
                for k in 0..3 {
                    let sx = cx + k * (cw / 2) - u(2);
                    let sy = if tick % 2 == 0 { up_s(3 * ap) } else { up_s(3 * ap) - u(5) };
                    let sy = sy + if k == 1 { -u(6) } else { 0 };
                    sprite(dc, &SPARK, sx, sy, ap, &|c| (c == b'X').then_some(col));
                }
            }
            Pose::Sad | Pose::Tired => {
                let drop_c = if pose == Pose::Sad { UP_BLUE } else { rgb(0xb8, 0xe6, 0xff) };
                let dy = (tick % 4) as i32 * u(3);
                let dxp = if pose == Pose::Sad { cx + cw / 4 } else { cx + cw - u(10) };
                let base = if pose == Pose::Sad { cy + 5 * px } else { cy + px };
                sprite(dc, &DROP, dxp, base + dy, ap, &|c| (c == b'X').then_some(drop_c));
            }
            _ => {}
        }
    }
    let pet_rect = RECT { left: cx, top: cy, right: cx + cw, bottom: cy + ch };

    // --- speed bubble beside the pet (download, plus upload if enabled); hidden in focus mode ---
    if !fs.focus && !ms.open {
    let ap = u(3);
    let (row_h, bd) = (u(26), u(3));
    let nrows = if show_up { 2 } else { 1 };
    let (bw, bh) = (u(142), row_h * nrows + 2 * bd);
    let bx = if flipx { cx + cw + u(12) } else { cx - bw - u(12) };
    let by = if flipy { u(10) } else { h - bh - u(10) };
    fill(dc, bx, by, bw, bh, GREEN);
    fill(dc, bx + bd, by + bd, bw - 2 * bd, bh - 2 * bd, INK);
    let mut lines: Vec<(f64, u32, bool)> = vec![(speed, GREEN, true)];
    if show_up {
        lines.push((up, UP_BLUE, false));
    }
    for (n, (bps, col, down)) in lines.into_iter().enumerate() {
        let ry = by + bd + row_h * n as i32;
        let arrow: Vec<&[u8]> = if down { ARROW.to_vec() } else { ARROW.iter().rev().copied().collect() };
        sprite(dc, &arrow, bx + bd + u(10), ry + (row_h - 7 * ap) / 2, ap, &|c| (c == b'X').then_some(col));
        SetTextColor(dc, col);
        let txt = wide(&format_rate(bps, bits));
        let mut tr = RECT { left: bx + bd + u(40), top: ry, right: bx + bw - bd, bottom: ry + row_h };
        DrawTextW(dc, txt.as_ptr(), -1, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    }
    }

    // --- focus mode: pomodoro timer card + open tasks card, stacked next to the pet ---
    let mut frects: Vec<(RECT, FAct)> = Vec::new();
    let (pad, bdr, sh) = (u(10), u(3), u(4));
    let (fx, fw) = (u(2), w - u(2) - sh - u(2));
    let measure = |dc: HDC, t: &[u16]| -> i32 {
        let mut m = RECT { left: 0, top: 0, right: fw - 2 * (pad + bdr), bottom: 0 };
        DrawTextW(dc, t.as_ptr(), -1, &mut m, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
        m.bottom
    };
    // what the speech bubble will be (shown text, text height, total height), if there is one
    let alert_shown = alert.as_ref().map(|text| {
        let mut shown: String = text.chars().take(90).collect();
        if shown.len() < text.len() {
            shown.push_str("...");
        }
        shown
    });
    let (alert_head_h, alert_btn_h, alert_gap) = (u(18), u(24), u(8));
    let bubble_h = if let Some(shown) = &alert_shown {
        2 * (bdr + pad) + alert_head_h + measure(dc, &wide(shown)) + alert_gap + alert_btn_h
    } else if let Some(f) = &fact {
        measure(dc, &wide(f)) + 2 * (pad + bdr)
    } else {
        0
    };
    // in focus mode the bubble takes the tasks card's slot (never covers the timer)
    let mut focus_bubble_y: Option<i32> = None;
    if fs.focus && !ms.open {
        let (fpad, fb) = (u(8), u(3));
        let (x0, pw) = (u(2), w - u(2) - u(6));
        let gap = u(8);
        let tc_h = u(118);
        let n_rows = fs.tasks.len().max(1) as i32;
        let tk_h = 2 * (fb + fpad) + u(22) + n_rows * u(26) + if fs.more > 0 { u(18) } else { 0 };
        let geom = rules::focus_geom(base_y, ch, flipy, u(12), tc_h, tk_h, bubble_h, gap);
        let (tc_y, tk_y) = (geom.timer_y, geom.tasks_y);
        if bubble_h > 0 {
            focus_bubble_y = Some(geom.bubble_y);
        }
        let iw = pw - 2 * (fb + fpad);
        let ix = x0 + fb + fpad;
        let accent = if fs.phase == 0 { TOMATO } else { GREEN };

        // timer card
        fill(dc, x0, tc_y, pw, tc_h, accent);
        fill(dc, x0 + fb, tc_y + fb, pw - 2 * fb, tc_h - 2 * fb, INK);
        let y0 = tc_y + fb + fpad;
        let name = ["FOCUS", "SHORT BREAK", "LONG BREAK"][fs.phase as usize];
        SetTextColor(dc, accent);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(16) };
        text_at(dc, name, &mut r, DT_SINGLELINE);
        SetTextColor(dc, HINT);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(16) };
        text_at(dc, &format!("{}/{} TODAY", fs.done, fs.goal), &mut r, DT_SINGLELINE | DT_RIGHT);
        let big = CreateFontW(-u(30), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
        SelectObject(dc, big);
        SetTextColor(dc, CREAM);
        let mut r = RECT { left: ix, top: y0 + u(16), right: ix + iw, bottom: y0 + u(52) };
        text_at(dc, &format!("{:02}:{:02}", fs.remain / 60, fs.remain % 60), &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
        SelectObject(dc, font);
        DeleteObject(big);
        let (segs, sg) = (20i32, u(2));
        let sw = (iw - (segs - 1) * sg) / segs;
        let filled = if fs.total == 0 { 0 } else { (fs.total.saturating_sub(fs.remain) * segs as u64 / fs.total) as i32 };
        for i in 0..segs {
            fill(dc, ix + i * (sw + sg), y0 + u(56), sw, u(10), if i < filled { accent } else { PANEL_LIGHT });
        }
        let (by, bh, g) = (y0 + u(70), u(24), u(6));
        let bw = (iw - 3 * g) / 4;
        let buttons = [
            (if fs.running { "PAUSE" } else { "START" }, FAct::Toggle, BTN_GREEN),
            ("RESET", FAct::Reset, PANEL_LIGHT),
            ("SKIP", FAct::Skip, PANEL_LIGHT),
            ("EXIT", FAct::Exit, BTN_RED),
        ];
        for (i, (label, act, bg)) in buttons.into_iter().enumerate() {
            let bx = ix + i as i32 * (bw + g);
            fill(dc, bx, by, bw, bh, PANEL_EDGE);
            fill(dc, bx + u(2), by + u(2), bw - u(4), bh - u(4), bg);
            SetTextColor(dc, CREAM);
            let mut rr = RECT { left: bx, top: by, right: bx + bw, bottom: by + bh };
            text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            frects.push((RECT { left: bx, top: by, right: bx + bw, bottom: by + bh }, act));
        }

        // tasks card (hidden while the pet is talking: the bubble uses its slot)
        if bubble_h == 0 {
        fill(dc, x0, tk_y, pw, tk_h, UP_BLUE);
        fill(dc, x0 + fb, tk_y + fb, pw - 2 * fb, tk_h - 2 * fb, INK);
        let y0 = tk_y + fb + fpad;
        SetTextColor(dc, UP_BLUE);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(20) };
        text_at(dc, &format!("TASKS ({})", fs.tasks.len() + fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        let (now, ..) = local_now();
        if fs.tasks.is_empty() {
            SetTextColor(dc, DIM);
            let mut r = RECT { left: ix, top: y0 + u(22), right: ix + iw, bottom: y0 + u(48) };
            text_at(dc, "NO OPEN TASKS. ADD ONE!", &mut r, DT_SINGLELINE | DT_VCENTER);
        }
        for (i, (id, text, due)) in fs.tasks.iter().enumerate() {
            let ry = y0 + u(22) + i as i32 * u(26);
            let bs = u(16);
            let cby = ry + (u(26) - bs) / 2;
            fill(dc, ix, cby, bs, bs, CREAM);
            fill(dc, ix + u(2), cby + u(2), bs - u(4), bs - u(4), PANEL);
            frects.push((RECT { left: ix - u(2), top: ry, right: ix + bs + u(6), bottom: ry + u(26) }, FAct::Task(*id)));
            let overdue = due.map_or(false, |d| d <= now);
            let due_w = if due.is_some() { u(118) } else { 0 };
            SetTextColor(dc, if overdue { RED } else { CREAM });
            let mut tr = RECT { left: ix + bs + u(8), top: ry, right: ix + iw - due_w, bottom: ry + u(26) };
            text_at(dc, text, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
            if let Some(d) = due {
                SetTextColor(dc, if overdue { RED } else { GREEN });
                let mut dr = RECT { left: ix + iw - due_w, top: ry, right: ix + iw, bottom: ry + u(26) };
                text_at(dc, &todo::fmt_due(*d, now), &mut dr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            }
        }
        if fs.more > 0 {
            SetTextColor(dc, DIM);
            let ry = y0 + u(22) + n_rows * u(26);
            let mut r = RECT { left: ix, top: ry, right: ix + iw, bottom: ry + u(18) };
            text_at(dc, &format!("+{} MORE (PET MENU > TASKS)", fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        }
        }
    }

    // --- pixel hub menu: one place to reach everything ---
    let (mrects, menu_top) = draw_hub(dc, scale, w, base_y, ch, flipy, &ms);

    // --- message bubble (reminder or fact), above the pet (below it when the pet is at the top) ---
    let mut btns: Vec<(RECT, u8)> = Vec::new();
    let mut fact_rect: Option<RECT> = None;
    let anchor_y = |bh: i32| -> i32 {
        if flipy { base_y + ch + u(12) } else { (base_y - u(12) - bh).max(u(2)) }
    };
    if let Some(shown) = alert_shown {
        let t = wide(&shown);
        let th = measure(dc, &t);
        let (head_h, btn_h, gap) = (alert_head_h, alert_btn_h, alert_gap);
        let fh = 2 * (bdr + pad) + head_h + th + gap + btn_h;
        // focus mode: the tasks card's slot; hub open: cover the top of the hub; else beside the pet
        let fy = focus_bubble_y.or(menu_top).unwrap_or_else(|| anchor_y(fh));
        fill(dc, fx + sh, fy + sh, fw, fh, INK);
        fill(dc, fx, fy, fw, fh, ALERT_ORANGE);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, INK);
        let head = wide("REMINDER");
        let mut hr = RECT { left: fx + bdr + pad, top: fy + bdr + pad, right: fx + fw, bottom: fy + bdr + pad + head_h };
        DrawTextW(dc, head.as_ptr(), -1, &mut hr, DT_SINGLELINE | DT_NOPREFIX);
        let mut tr = RECT {
            left: fx + bdr + pad,
            top: fy + bdr + pad + head_h,
            right: fx + fw - bdr - pad,
            bottom: fy + bdr + pad + head_h + th,
        };
        DrawTextW(dc, t.as_ptr(), -1, &mut tr, DT_WORDBREAK | DT_NOPREFIX);
        // click options
        let inner = fw - 2 * (bdr + pad);
        let g2 = u(6);
        let bw3 = (inner - 2 * g2) / 3;
        let by0 = fy + fh - bdr - pad - btn_h;
        for (i, (label, bg, fg)) in [("DONE", BTN_GREEN, CREAM), ("+10 MIN", INK, CREAM), ("LATER", BTN_GRAY, INK)]
            .into_iter()
            .enumerate()
        {
            let x0 = fx + bdr + pad + i as i32 * (bw3 + g2);
            fill(dc, x0, by0, bw3, btn_h, bg);
            SetTextColor(dc, fg);
            let l = wide(label);
            let mut r = RECT { left: x0, top: by0, right: x0 + bw3, bottom: by0 + btn_h };
            DrawTextW(dc, l.as_ptr(), -1, &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_NOPREFIX);
            btns.push((RECT { left: x0, top: by0, right: x0 + bw3, bottom: by0 + btn_h }, i as u8));
        }
    } else if let Some(f) = fact {
        let ft = wide(&f);
        let fh = measure(dc, &ft) + 2 * (pad + bdr);
        let fy = focus_bubble_y.unwrap_or_else(|| anchor_y(fh));
        fill(dc, fx + sh, fy + sh, fw, fh, INK);
        fill(dc, fx, fy, fw, fh, INK);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, INK);
        let mut r = RECT { left: fx + bdr + pad, top: fy + bdr + pad, right: fx + fw - bdr - pad, bottom: fy + fh };
        DrawTextW(dc, ft.as_ptr(), -1, &mut r, DT_WORDBREAK | DT_NOPREFIX);
        fact_rect = Some(RECT { left: fx, top: fy, right: fx + fw, bottom: fy + fh });
    }

    {
        let mut s = ST.lock().unwrap();
        s.btns = btns;
        s.pet_rect = pet_rect;
        s.fact_rect = fact_rect;
        s.mrects = mrects;
        s.frects = frects;
    }

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(font);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}
