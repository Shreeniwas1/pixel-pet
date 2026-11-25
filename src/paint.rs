//! Painting the pet window: pet, bubbles, focus cards and the pixel hub menu.

use crate::prelude::*;

/// The picture the pet is drawn on before it is shown, kept between paints.
struct Easel {
    dc: HDC,
    bmp: HBITMAP,
    dots: *mut u32,
    size: (i32, i32),
    font: HFONT,
    font_px: i32,
}

thread_local! {
    static EASEL: std::cell::RefCell<Easel> = const {
        std::cell::RefCell::new(Easel { dc: null_mut(), bmp: null_mut(), dots: null_mut(), size: (0, 0), font: null_mut(), font_px: 0 })
    };
}

/// A drawing surface of this size with the pet's font selected, or None if Windows is out of them.
unsafe fn easel(screen: HDC, w: i32, h: i32, font_px: i32) -> Option<(HDC, *mut u32, HFONT)> {
    EASEL.with(|e| {
        let mut e = e.borrow_mut();
        if e.dc.is_null() {
            e.dc = CreateCompatibleDC(screen);
            SetBkMode(e.dc, TRANSPARENT as i32);
        }
        if e.size != (w, h) {
            let mut bi: BITMAPINFO = zeroed();
            bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top row first
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            let mut dots: *mut std::ffi::c_void = null_mut();
            let bmp = CreateDIBSection(screen, &bi, DIB_RGB_COLORS, &mut dots, null_mut(), 0);
            if bmp.is_null() || dots.is_null() {
                return None;
            }
            SelectObject(e.dc, bmp);
            if !e.bmp.is_null() {
                DeleteObject(e.bmp);
            }
            (e.bmp, e.dots, e.size) = (bmp, dots as *mut u32, (w, h));
        }
        if e.font_px != font_px {
            let font = CreateFontW(-font_px, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3 /* NONANTIALIASED */, 49, wide("Consolas").as_ptr());
            SelectObject(e.dc, font);
            if !e.font.is_null() {
                DeleteObject(e.font);
            }
            (e.font, e.font_px) = (font, font_px);
        }
        Some((e.dc, e.dots, e.font))
    })
}

pub(crate) unsafe fn paint(hwnd: HWND) {
    let (scale, tick, speed, up, (show_speed, show_up), bits, fact, alert, pet, pose) = {
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
        (eff_scale(&s), s.tick, s.speed, s.up, (s.show_speed, s.show_up), s.bits, fact, alert, &PETS[s.pet], s.mood.act())
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
    let (minimal, hover, pc) = {
        let s = ST.lock().unwrap();
        (s.minimal, s.hover, s.show_pc.then_some((s.cpu, s.ram)))
    };
    let ms = hub_view();
    let fact = if ms.open { None } else { fact }; // the hub has the stage
    let u = |v: i32| (v as f32 * scale).round() as i32;
    // the pet: what its body is doing, and the little things flying around it
    let (g, clip, t, facing, push, parts, meal, hungry_for, resting) = {
        let s = ST.lock().unwrap();
        let b = &s.body;
        (geo(&s), b.clip(pose), b.t, b.facing, b.push(), b.parts.clone(), b.meal, b.food, b.mode == Mode::Rest)
    };
    let (w, h) = (g.w, g.h);
    let (flipx, flipy) = g.flip;

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let Some((dc, dots, font)) = easel(screen, w, h, u(13)) else {
        EndPaint(hwnd, &ps);
        return;
    };
    fill(dc, 0, 0, w, h, KEY);

    // --- pet ---
    let px = g.px;
    let (cw, ch) = (g.cw, g.ch);
    let (cx, base_y) = g.origin();
    let cy = base_y;
    let (feet_x, feet_y) = g.feet();
    let mut rig = sample(pet, clip, t, &push);
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    if matches!(clip, Clip::Stand | Clip::Tired) && resting {
        // eyes follow the mouse while the pet is awake
        let mut cp: POINT = zeroed();
        GetCursorPos(&mut cp);
        let dx = cp.x - (wr.left + feet_x);
        rig.look = (if dx > u(90) { 1.0 } else if dx < -u(90) { -1.0 } else { 0.0 }) * facing as f32;
    }
    let mut art = Surface { px: std::slice::from_raw_parts_mut(dots, (w * h) as usize), w, h };
    GdiFlush();
    blit(&mut art, &compose(pet, &rig), pet, &rig, (feet_x as f32, feet_y as f32), px as f32, facing);

    let ap = if minimal { u(2) } else { u(3) }.max(1);
    if clip == Clip::Dizzy {
        // stars circling the head
        let (hx, hy) = (feet_x as f32, (feet_y - ch) as f32 - 1.5 * ap as f32);
        for k in 0..3 {
            let a = t * 5.0 + k as f32 * 2.094;
            let (sx, sy) = (hx + a.cos() * cw as f32 * 0.5, hy + a.sin() * 2.5 * ap as f32);
            sprite(dc, &SPARK, sx as i32 - ap, sy as i32 - ap, ap, &|c| (c == b'X').then_some(rgb(0xff, 0xe0, 0x66)));
        }
    }
    if clip == Clip::Hungry {
        // a thought bubble with what it would like
        let food = FOODS[hungry_for].grid;
        let (fw, fh) = (food[0].len() as i32 * ap, food.len() as i32 * ap);
        let (bw, bh) = (fw + 6 * ap, fh + 6 * ap);
        let bx = (feet_x - bw / 2).clamp(0, w - bw);
        let by = if flipy { feet_y + 3 * ap } else { (feet_y - ch - bh - 5 * ap).max(0) };
        for (x, y, w, h) in [(bx + ap, by, bw - 2 * ap, bh), (bx, by + ap, bw, bh - 2 * ap)] {
            fill(dc, x, y, w, h, CREAM);
        }
        if !flipy {
            fill(dc, feet_x - ap, by + bh + ap, 2 * ap, 2 * ap, CREAM);
        }
        sprite(dc, food, bx + 3 * ap, by + 3 * ap, ap, &pet_color);
    }
    if let Some(m) = meal {
        let food = FOODS[m.food].grid;
        let fp = (px * 2 / 3).max(2);
        let (fw, fh) = (food[0].len() as i32 * fp, food.len() as i32 * fp);
        sprite(dc, food, m.x as i32 - wr.left - fw / 2, m.y as i32 - wr.top - fh, fp, &pet_color);
    }
    for q in &parts {
        let (x, y) = (q.x as i32 - wr.left, q.y as i32 - wr.top);
        let k = q.age / q.life;
        let one = |rows: &[&[u8]], p: i32| sprite(dc, rows, x - rows[0].len() as i32 * p / 2, y - rows.len() as i32 * p / 2, p, &|c| (c == b'X').then_some(q.c));
        match q.bit {
            Bit::Heart => one(&HEART, ap),
            Bit::Spark => one(&SPARK, if k < 0.6 { ap } else { (ap * 2 / 3).max(1) }),
            Bit::Drop => one(&DROP, ap),
            Bit::Note => one(&NOTE, ap),
            Bit::Alarm => one(&ALARM, ap),
            Bit::Zed => one(&ZED, if k < 0.35 { (ap * 2 / 3).max(1) } else { ap }),
            Bit::Burst => one(&BURST, (ap as f32 * (1.0 + k)) as i32),
            Bit::Crumb => fill(dc, x, y, ap, ap, q.c),
            Bit::Dust => {
                let d = ((2.4 * (1.0 - k)) * ap as f32) as i32 + 1;
                fill(dc, x - d / 2, y - d, d, d, q.c);
            }
            Bit::Smoke => {
                let d = ((1.4 + 2.4 * k) * ap as f32) as i32;
                if k < 0.75 || (tick + x as u32) % 2 == 0 {
                    fill(dc, x - d / 2, y - d / 2, d, d, q.c);
                }
            }
        }
    }
    let pet_rect = RECT { left: cx - px, top: cy - px, right: cx + cw + px, bottom: cy + ch };
    let mut hover_rect = pet_rect; // grows with whatever is drawn beside the pet

    // --- info bubble beside the pet; hidden in focus mode, while it is moving about, and until hovered in minimal mode ---
    if !fs.focus && !ms.open && resting && (!minimal || hover) {
    let ap = u(3);
    let (row_h, bd, frame) = if minimal { (u(26), u(1).max(1), theme().panel_edge) } else { (u(26), u(3), GREEN) };
    let ai_tokens = { let (on, cache) = { let s = ST.lock().unwrap(); (s.tokens_on, s.cache_on) }; let t = TOK.lock().unwrap(); let n = shown(&t.sum(1), cache); (on && t.found && n > 0).then(|| n) };
    let nrows = show_speed as i32 + show_up as i32 + pc.is_some() as i32 + ai_tokens.is_some() as i32;
    if nrows > 0 {
    let (bw, bh) = (u(142), row_h * nrows + 2 * bd);
    let bx = if flipx { cx + cw + u(22) } else { cx - bw - u(22) };
    let by = if flipy { u(10) } else { h - bh - u(10) };
    fill(dc, bx, by, bw, bh, frame);
    hover_rect = presence::union(hover_rect, rc(bx, by, bw, bh));
    fill(dc, bx + bd, by + bd, bw - 2 * bd, bh - 2 * bd, theme().ink);
    let mut lines: Vec<(f64, u32, bool)> = Vec::new();
    if show_speed {
        lines.push((speed, GREEN, true));
    }
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
    if let Some((cpu, ram)) = pc {
        let ry = by + bd + row_h * (show_speed as i32 + show_up as i32);
        SetTextColor(dc, theme().hint);
        let mut lr = rc(bx + bd + u(8), ry, bw, row_h);
        text_at(dc, &format!("CPU {cpu}%"), &mut lr, DT_SINGLELINE | DT_VCENTER);
        let mut rr = RECT { left: bx, top: ry, right: bx + bw - bd - u(4), bottom: ry + row_h };
        text_at(dc, &format!("RAM {ram}%"), &mut rr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
    }
    if let Some(t) = ai_tokens {
        let ry = by + bd + row_h * (nrows - 1);
        SetTextColor(dc, PINK);
        let mut lr = RECT { left: bx + bd + u(8), top: ry, right: bx + bd + u(40), bottom: ry + row_h };
        DrawTextW(dc, wide("AI").as_ptr(), -1, &mut lr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
        let txt = wide(&format!("{} TOK", fmt_tokens(t)));
        let mut tr = RECT { left: bx + bd + u(40), top: ry, right: bx + bw - bd, bottom: ry + row_h };
        DrawTextW(dc, txt.as_ptr(), -1, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    }
    }
    }

    // --- focus mode: pomodoro timer card + open tasks card, stacked next to the pet ---
    let mut frects: Vec<(RECT, FAct)> = Vec::new();
    let (pad, bdr, sh) = (u(10), u(3), u(4));
    let ox = if flipx { w - u(PANEL_W) } else { 0 }; // the panels leave the pet its margin
    let (fx, fw) = (ox + u(2), u(PANEL_W) - u(2) - sh - u(2));
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
    // minimal mode folds the cards into a small clock until hovered; a bubble keeps them folded
    // so its buttons do not jump away from the mouse
    let cards = fs.focus && !ms.open && (!minimal || (hover && bubble_h == 0));
    if fs.focus && !ms.open && !cards {
        let (pw, ph) = (u(58), u(24));
        let x = if flipx { cx + cw + u(8) } else { cx - u(8) - pw };
        let y = cy + (ch - ph) / 2;
        let b = u(1).max(1);
        fill(dc, x, y, pw, ph, theme().panel_edge);
        fill(dc, x + b, y + b, pw - 2 * b, ph - 2 * b, theme().ink);
        SetTextColor(dc, if !fs.running { theme().dim } else if fs.phase == 0 { TOMATO } else { GREEN });
        let mut r = rc(x, y, pw, ph);
        text_at(dc, &format!("{:02}:{:02}", fs.remain / 60, fs.remain % 60), &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
        hover_rect = presence::union(hover_rect, rc(x, y, pw, ph));
    }
    if cards {
        let (fpad, fb) = (u(8), u(3));
        let (x0, pw) = (ox + u(2), u(PANEL_W) - u(2) - u(6));
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

        hover_rect = presence::union(hover_rect, rc(x0, tc_y, pw, tc_h));
        if bubble_h == 0 {
            hover_rect = presence::union(hover_rect, rc(x0, tk_y, pw, tk_h));
        }

        // timer card
        fill(dc, x0, tc_y, pw, tc_h, accent);
        fill(dc, x0 + fb, tc_y + fb, pw - 2 * fb, tc_h - 2 * fb, theme().ink);
        let y0 = tc_y + fb + fpad;
        let name = ["FOCUS", "SHORT BREAK", "LONG BREAK"][fs.phase as usize];
        SetTextColor(dc, accent);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(16) };
        text_at(dc, name, &mut r, DT_SINGLELINE);
        SetTextColor(dc, theme().hint);
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
            fill(dc, ix + i * (sw + sg), y0 + u(56), sw, u(10), if i < filled { accent } else { theme().panel_light });
        }
        let (by, bh, g) = (y0 + u(70), u(24), u(6));
        let bw = (iw - 3 * g) / 4;
        let buttons = [
            (if fs.running { "PAUSE" } else { "START" }, FAct::Toggle, BTN_GREEN),
            ("RESET", FAct::Reset, theme().panel_light),
            ("SKIP", FAct::Skip, theme().panel_light),
            ("EXIT", FAct::Exit, BTN_RED),
        ];
        for (i, (label, act, bg)) in buttons.into_iter().enumerate() {
            let bx = ix + i as i32 * (bw + g);
            fill(dc, bx, by, bw, bh, theme().panel_edge);
            fill(dc, bx + u(2), by + u(2), bw - u(4), bh - u(4), bg);
            SetTextColor(dc, CREAM);
            let mut rr = RECT { left: bx, top: by, right: bx + bw, bottom: by + bh };
            text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            frects.push((RECT { left: bx, top: by, right: bx + bw, bottom: by + bh }, act));
        }

        // tasks card (hidden while the pet is talking: the bubble uses its slot)
        if bubble_h == 0 {
        fill(dc, x0, tk_y, pw, tk_h, UP_BLUE);
        fill(dc, x0 + fb, tk_y + fb, pw - 2 * fb, tk_h - 2 * fb, theme().ink);
        let y0 = tk_y + fb + fpad;
        SetTextColor(dc, UP_BLUE);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(20) };
        text_at(dc, &format!("TASKS ({})", fs.tasks.len() + fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        let (now, ..) = local_now();
        if fs.tasks.is_empty() {
            SetTextColor(dc, theme().dim);
            let mut r = RECT { left: ix, top: y0 + u(22), right: ix + iw, bottom: y0 + u(48) };
            text_at(dc, "NO OPEN TASKS. ADD ONE!", &mut r, DT_SINGLELINE | DT_VCENTER);
        }
        for (i, (id, text, due)) in fs.tasks.iter().enumerate() {
            let ry = y0 + u(22) + i as i32 * u(26);
            let bs = u(16);
            let cby = ry + (u(26) - bs) / 2;
            fill(dc, ix, cby, bs, bs, CREAM);
            fill(dc, ix + u(2), cby + u(2), bs - u(4), bs - u(4), theme().panel);
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
            SetTextColor(dc, theme().dim);
            let ry = y0 + u(22) + n_rows * u(26);
            let mut r = RECT { left: ix, top: ry, right: ix + iw, bottom: ry + u(18) };
            text_at(dc, &format!("+{} MORE (PET MENU > TASKS)", fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        }
        }
    }

    // --- pixel hub menu: one place to reach everything ---
    let (mrects, menu_top) = draw_hub(dc, &mut art, scale, ox, base_y, ch, flipy, &ms);

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
        fill(dc, fx + sh, fy + sh, fw, fh, theme().ink);
        fill(dc, fx, fy, fw, fh, ALERT_ORANGE);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, theme().ink);
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
        for (i, (label, bg, fg)) in [("DONE", BTN_GREEN, CREAM), ("+10 MIN", theme().ink, CREAM), ("LATER", BTN_GRAY, theme().ink)]
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
        fill(dc, fx + sh, fy + sh, fw, fh, theme().ink);
        fill(dc, fx, fy, fw, fh, theme().ink);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, theme().ink);
        let mut r = RECT { left: fx + bdr + pad, top: fy + bdr + pad, right: fx + fw - bdr - pad, bottom: fy + fh };
        DrawTextW(dc, ft.as_ptr(), -1, &mut r, DT_WORDBREAK | DT_NOPREFIX);
        fact_rect = Some(RECT { left: fx, top: fy, right: fx + fw, bottom: fy + fh });
    }

    {
        let mut s = ST.lock().unwrap();
        s.btns = btns;
        s.pet_rect = pet_rect;
        s.hover_rect = hover_rect;
        s.fact_rect = fact_rect;
        s.mrects = mrects;
        s.frects = frects;
    }

    GdiFlush();
    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    EndPaint(hwnd, &ps);
}
