//! The todo window (pixel-drawn panel plus one real edit box).

use crate::prelude::*;

// ======================================================================
// todo window (custom pixel-drawn panel + one real edit box)
// ======================================================================

pub(crate) const TODO_W: i32 = 440;
pub(crate) const TODO_H: i32 = 430;
/// Minimum row height; rows with wrapped text grow taller.
pub(crate) const ROW_H: i32 = 32;

pub(crate) static HOVER: AtomicI32 = AtomicI32::new(-1);
pub(crate) static EDIT_FONT: AtomicIsize = AtomicIsize::new(0);
/// Rows drawn by the last paint: (todo id, top, bottom, checkbox right edge). Rows wrap, so
/// their heights vary and clicks are hit-tested against what was actually drawn.
pub(crate) static ROWS: Mutex<Vec<(u32, i32, i32, i32)>> = Mutex::new(Vec::new());
/// Whether the last paint ran out of room before the end of the list (so the wheel can scroll on).
pub(crate) static LIST_MORE: AtomicI32 = AtomicI32::new(0);

pub(crate) struct Lay {
    pub close: RECT,
    pub input: RECT,
    pub prev: RECT,
    pub list: RECT,
    pub btns: [RECT; 6],
    pub hint: RECT,
    pub title_h: i32,
}

pub(crate) fn layout(k: &dyn Fn(i32) -> i32) -> Lay {
    let bw = (412 - 30) / 6;
    let mut btns = [NO_RECT; 6];
    for (i, b) in btns.iter_mut().enumerate() {
        *b = rc(k(14 + i as i32 * (bw + 6)), k(340), k(bw), k(32));
    }
    Lay {
        close: rc(k(TODO_W - 3 - 28), k(3), k(24), k(24)),
        input: rc(k(14), k(42), k(412), k(30)),
        prev: rc(k(14), k(78), k(412), k(18)),
        list: rc(k(14), k(102), k(412), k(228)),
        btns,
        hint: rc(k(14), k(382), k(412), k(40)),
        title_h: k(30),
    }
}

pub(crate) unsafe fn get_text(h: HWND) -> String {
    let n = GetWindowTextLengthW(h);
    if n <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; n as usize + 1];
    let got = GetWindowTextW(h, buf.as_mut_ptr(), n + 1);
    String::from_utf16_lossy(&buf[..got.max(0) as usize])
}

pub(crate) unsafe fn create_todo_window() -> HWND {
    let wa = primary_work();
    let dpi = scales(MonitorFromPoint(POINT { x: wa.left, y: wa.top }, MONITOR_DEFAULTTOPRIMARY)).0;
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let (ww, wh) = (k(TODO_W), k(TODO_H));
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        wide("PixelTodoWnd").as_ptr(),
        wide("Pixel Pet - Todo").as_ptr(),
        WS_POPUP,
        wa.left + (wa.right - wa.left - ww) / 2,
        wa.top + (wa.bottom - wa.top - wh) / 2,
        ww, wh,
        null_mut(), null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    TODO_WND.store(hwnd as isize, Relaxed);
    let l = layout(&k);
    let edit = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | 0x80, /* ES_AUTOHSCROLL */
        l.input.left + k(8), l.input.top + k(5), k(412 - 16), k(20),
        hwnd, null_mut(), GetModuleHandleW(null()), null(),
    );
    let f = CreateFontW(-k(15), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    EDIT_FONT.store(f as isize, Relaxed);
    SendMessageW(edit, WM_SETFONT, f as WPARAM, 1);
    TODO_EDIT.store(edit as isize, Relaxed);
    hwnd
}

pub(crate) unsafe fn todo_paint(hwnd: HWND) {
    let dpi = win_scale(hwnd);
    let (dpi, items, sel, scroll, prev) = {
        let s = ST.lock().unwrap();
        let items: Vec<(u32, String, Option<i64>, bool, bool, Option<(usize, usize)>)> = s
            .todos
            .iter()
            .map(|t| (t.id, t.text.clone(), t.due, t.done, t.parent.is_some(), todo::progress(&s.todos, t.id)))
            .collect();
        let sel = s.todo_sel.and_then(|id| s.todos.iter().position(|t| t.id == id));
        (dpi, items, sel, s.todo_scroll, s.todo_prev.clone())
    };
    let (now, ..) = local_now();
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let (w, h) = (k(TODO_W), k(TODO_H));
    let lay = layout(&k);
    let hover = HOVER.load(Relaxed);

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);

    // frame, title bar
    let b = k(3);
    fill(dc, 0, 0, w, h, theme().accent);
    fill(dc, b, b, w - 2 * b, h - 2 * b, theme().ink);
    fill(dc, b, b, w - 2 * b, lay.title_h - b, theme().accent);

    let f14 = CreateFontW(-k(14), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    let f12 = CreateFontW(-k(12), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    let old_font = SelectObject(dc, f14);

    SetTextColor(dc, theme().ink);
    let open = items.iter().filter(|i| !i.3).count();
    let mut tr = rc(k(14), 0, k(300), lay.title_h);
    text_at(dc, &format!("PIXEL TODO  -  {} OPEN", open), &mut tr, DT_SINGLELINE | DT_VCENTER);
    let c = &lay.close;
    fill(dc, c.left, c.top, c.right - c.left, c.bottom - c.top, if hover == 10 { CREAM } else { theme().ink });
    let cp = k(2);
    let cross: Vec<&[u8]> = CROSS.to_vec();
    sprite(dc, &cross, c.left + (c.right - c.left - 7 * cp) / 2, c.top + (c.bottom - c.top - 7 * cp) / 2, cp,
        &|ch| (ch == b'X').then_some(if hover == 10 { theme().ink } else { theme().accent }));

    // input frame (the real EDIT control sits inside it)
    let i = &lay.input;
    fill(dc, i.left, i.top, i.right - i.left, i.bottom - i.top, GREEN);
    fill(dc, i.left + k(2), i.top + k(2), i.right - i.left - k(4), i.bottom - i.top - k(4), theme().panel);

    // preview of what the smart parser understood
    SelectObject(dc, f12);
    SetTextColor(dc, UP_BLUE);
    let mut pr = lay.prev;
    text_at(dc, &prev, &mut pr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);

    // list
    let l = &lay.list;
    fill(dc, l.left, l.top, l.right - l.left, l.bottom - l.top, theme().panel_edge);
    fill(dc, l.left + k(2), l.top + k(2), l.right - l.left - k(4), l.bottom - l.top - k(4), theme().panel);
    SelectObject(dc, f14);
    if items.is_empty() {
        SetTextColor(dc, theme().dim);
        let mut er = *l;
        text_at(dc, "NOTHING TO DO. ADD SOMETHING ABOVE!", &mut er, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
    }
    // Rows word-wrap so long tasks are readable in full; each row grows to fit its text.
    let wrap = DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX;
    let (inner_top, inner_bot) = (l.top + k(2), l.bottom - k(2));
    let mut drawn: Vec<(u32, i32, i32, i32)> = Vec::new();
    let mut more = false;
    let mut y = inner_top;
    // one line's height, measured with DrawText (the Linux/macOS port has no GetTextMetricsW)
    let mut lm = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    text_at(dc, "Ag", &mut lm, DT_SINGLELINE | DT_CALCRECT);
    let line_h = (lm.bottom - lm.top).max(1);
    for idx in scroll.min(items.len())..items.len() {
        let (id, text, due, done, sub, prog) = &items[idx];
        // subtasks: indented, with a smaller box
        let bs = if *sub { k(14) } else { k(18) };
        let bx = l.left + if *sub { k(36) } else { k(12) };
        let tx = bx + bs + k(10);
        let text = &match prog {
            Some((d, n)) => format!("{text}  [{d}/{n}]"),
            None => text.clone(),
        };
        let due_w = if due.is_some() { k(128) } else { 0 };
        let right = l.right - k(16);
        let tw = (right - due_w - tx).max(k(40));
        let t = wide(text);
        let mut m = RECT { left: 0, top: 0, right: tw, bottom: 0 };
        DrawTextW(dc, t.as_ptr(), -1, &mut m, wrap | DT_CALCRECT);
        let text_h = m.bottom - m.top;
        let mut rh = (text_h + k(12)).max(k(ROW_H));
        if y + rh > inner_bot {
            if idx > scroll {
                more = true;
                break;
            }
            rh = inner_bot - y; // a single giant task: show what fits
        }
        if sel == Some(idx) {
            fill(dc, l.left + k(2), y, l.right - l.left - k(4), rh, theme().panel_light);
        }
        let overdue = !*done && due.map_or(false, |d| d <= now);
        // pixel checkbox, level with the first line of text
        let ty = y + ((rh - text_h) / 2).max(k(6));
        let by = (ty + (line_h - bs) / 2).max(y + k(2));
        if *sub {
            // a little elbow joining the subtask to its parent
            let ex = l.left + k(22);
            fill(dc, ex, y, k(2), by + bs / 2 - y, theme().dim);
            fill(dc, ex, by + bs / 2, bx - ex - k(4), k(2), theme().dim);
        }
        fill(dc, bx, by, bs, bs, if *done { GREEN } else { CREAM });
        fill(dc, bx + k(2), by + k(2), bs - k(4), bs - k(4), theme().panel);
        if *done {
            let cp = k(2);
            let ck: Vec<&[u8]> = CHECK.to_vec();
            sprite(dc, &ck, bx + (bs - 6 * cp) / 2, by + (bs - 5 * cp) / 2, cp, &|ch| (ch == b'X').then_some(GREEN));
        }
        let col = if *done { theme().dim } else if overdue { RED } else { CREAM };
        SetTextColor(dc, col);
        let mut r = RECT { left: tx, top: ty, right: tx + tw, bottom: y + rh };
        DrawTextW(dc, t.as_ptr(), -1, &mut r, wrap);
        if *done {
            // strike through every wrapped line
            let mut ly = ty;
            while ly + line_h / 2 < (ty + text_h).min(y + rh) {
                fill(dc, tx, ly + line_h / 2, m.right.min(tw), k(2), theme().dim);
                ly += line_h;
            }
        }
        if let Some(d) = due {
            SetTextColor(dc, if *done { theme().dim } else if overdue { RED } else { GREEN });
            let mut dr = RECT { left: right - due_w, top: ty, right, bottom: ty + line_h };
            text_at(dc, &todo::fmt_due(*d, now), &mut dr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
        }
        drawn.push((*id, y, y + rh, bx + bs + k(8)));
        y += rh;
    }
    let shown_n = drawn.len().max(1);
    *ROWS.lock().unwrap() = drawn;
    LIST_MORE.store(more as i32, Relaxed);
    if scroll > 0 || more {
        let (tx, tw) = (l.right - k(10), k(6));
        let (ty, th) = (l.top + k(4), l.bottom - l.top - k(8));
        fill(dc, tx, ty, tw, th, theme().panel_light);
        let n = items.len().max(1) as i32;
        let thumb = (th * shown_n as i32 / n).clamp(k(16), th);
        let off = ((th - thumb) * scroll as i32 / (n - shown_n as i32).max(1)).min(th - thumb);
        fill(dc, tx, ty + off, tw, thumb, theme().accent);
    }

    // buttons
    SelectObject(dc, f12);
    let labels = [("ADD", BTN_GREEN), ("DONE", theme().panel_light), ("+1 HR", theme().panel_light), ("DELETE", BTN_RED), ("CLEAR", theme().panel_light), ("+ SUB", BTN_GREEN)];
    for (n, (label, bg)) in labels.iter().enumerate() {
        pixel_button(dc, &lay.btns[n], label, *bg, hover == n as i32);
    }

    SetTextColor(dc, theme().hint);
    let mut hr = lay.hint;
    text_at(dc, "TRY: call mom tomorrow 5pm | gym fri 6:30pm | stretch in 45m.  Enter adds, Shift+Enter or +SUB adds a subtask to the selected task, Esc closes.", &mut hr, DT_WORDBREAK);

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(f14);
    DeleteObject(f12);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

pub(crate) unsafe fn redraw_todo() {
    let w = hw(&TODO_WND);
    if !w.is_null() {
        InvalidateRect(w, null(), 0);
    }
}

/// Re-sort, remember list order for selection, and repaint.
pub(crate) unsafe fn refresh_list() {
    {
        let mut s = ST.lock().unwrap();
        todo::sort(&mut s.todos);
        s.view = s.todos.iter().map(|t| t.id).collect();
        let n = s.todos.len();
        s.todo_scroll = s.todo_scroll.min(n.saturating_sub(1));
        if let Some(id) = s.todo_sel {
            if !s.todos.iter().any(|t| t.id == id) {
                s.todo_sel = None;
            }
        }
    }
    redraw_todo();
}

pub(crate) unsafe fn update_preview() {
    let text = get_text(hw(&TODO_EDIT));
    let msg = if text.trim().is_empty() {
        String::new()
    } else {
        let (now, ..) = local_now();
        match todo::parse_full(&text, now) {
            (title, Some(d), _) => format!("> {} | {}", todo::fmt_due(d, now), title),
            (title, None, _) => format!("> {} (no time set: try 'tomorrow 5pm', 'in 30m' or 'every monday 9am')", title),
        }
    };
    ST.lock().unwrap().todo_prev = msg;
    redraw_todo();
}

/// Add the typed task. With `sub`, it becomes a subtask of the selected task (or of that
/// task's parent, since lists are only two tiers); with nothing selected it is a normal task.
pub(crate) unsafe fn add_from_edit(sub: bool) {
    let edit = hw(&TODO_EDIT);
    let text = get_text(edit);
    if text.trim().is_empty() {
        return;
    }
    let (now, ..) = local_now();
    let (title, due, repeat) = todo::parse_full(&text, now);
    {
        let mut s = ST.lock().unwrap();
        let parent = if sub { s.todo_sel.and_then(|sel| todo::parent_for(&s.todos, sel)) } else { None };
        let id = s.next_id;
        s.next_id += 1;
        s.todos.push(Todo { id, text: title, due, done: false, fired: false, repeat, parent });
        if let Some(p) = parent {
            // a new open subtask reopens a finished parent
            if let Some(t) = s.todos.iter_mut().find(|t| t.id == p) {
                t.done = false;
            }
        }
        // keep the parent selected so several subtasks can be added in a row
        s.todo_sel = Some(parent.unwrap_or(id));
        save_todos(&s);
    }
    SetWindowTextW(edit, wide("").as_ptr());
    refresh_list();
    update_preview();
    InvalidateRect(hw(&PET_WND), null(), 0);
}

pub(crate) unsafe fn selected_id() -> Option<u32> {
    ST.lock().unwrap().todo_sel
}

/// op: 0 toggle done, 1 snooze +1h, 2 delete, 3 clear done
pub(crate) unsafe fn todo_op_on(op: u32, id: Option<u32>) {
    let (now, ..) = local_now();
    if op != 3 && id.is_none() {
        return;
    }
    {
        let mut s = ST.lock().unwrap();
        match (op, id) {
            (0, Some(id)) => {
                let was_done = s.todos.iter().any(|t| t.id == id && t.done);
                let completed = if was_done {
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        t.done = false; // un-complete
                    }
                    false
                } else {
                    todo::finish(&mut s.todos, id, now)
                };
                if completed {
                    let ev = s.mood.on_task_done();
                    body::gain(&mut s, wardrobe::XP_TASK);
                    say(&mut s, ev);
                }
            }
            (1, Some(id)) => {
                if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                    t.due = Some(now + 60);
                    t.fired = false;
                    t.done = false;
                }
            }
            (2, Some(id)) => {
                // deleting a task deletes its subtasks with it
                let gone = todo::family(&s.todos, id);
                s.todos.retain(|t| !gone.contains(&t.id));
            }
            _ => {
                let gone: Vec<u32> = s.todos.iter().filter(|t| t.done && t.parent.is_none()).map(|t| t.id).collect();
                s.todos.retain(|t| !t.done && !t.parent.map_or(false, |p| gone.contains(&p)));
            }
        }
        if let Some(a) = s.alert {
            if !s.todos.iter().any(|t| t.id == a && !t.done) {
                s.alert = None;
            }
        }
        save_todos(&s);
    }
    refresh_list();
    InvalidateRect(hw(&PET_WND), null(), 0);
}

pub(crate) unsafe fn todo_op(op: u32) {
    todo_op_on(op, selected_id());
}

pub(crate) unsafe fn show_todo() {
    let mut w = hw(&TODO_WND);
    if w.is_null() {
        w = create_todo_window();
    }
    refresh_list();
    update_preview();
    ShowWindow(w, SW_SHOW);
    SetForegroundWindow(w);
    SetFocus(hw(&TODO_EDIT));
}

pub(crate) unsafe fn todo_hit(x: i32, y: i32) -> i32 {
    let dpi = win_scale(hw(&TODO_WND));
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let lay = layout(&k);
    if contains(&lay.close, x, y) {
        return 10;
    }
    lay.btns.iter().position(|b| contains(b, x, y)).map_or(-1, |i| i as i32)
}

pub(crate) unsafe extern "system" fn todo_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => {
            todo_paint(hwnd);
            0
        }
        WM_DPICHANGED => {
            resize_for_dpi(hwnd, lp);
            let dpi = win_scale(hwnd);
            let k = |v: i32| (v as f32 * dpi).round() as i32;
            let l = layout(&k);
            let edit = hw(&TODO_EDIT);
            SetWindowPos(edit, null_mut(), l.input.left + k(8), l.input.top + k(5), k(412 - 16), k(20), SWP_NOZORDER | SWP_NOACTIVATE);
            let f = CreateFontW(-k(15), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
            SendMessageW(edit, WM_SETFONT, f as WPARAM, 1);
            DeleteObject(EDIT_FONT.swap(f as isize, Relaxed) as HGDIOBJ);
            0
        }
        WM_ERASEBKGND => 1,
        0x0133 /* WM_CTLCOLOREDIT */ => {
            let dc = wp as HDC;
            SetTextColor(dc, CREAM);
            SetBkColor(dc, theme().panel);
            SetDCBrushColor(dc, theme().panel);
            GetStockObject(DC_BRUSH) as LRESULT
        }
        WM_COMMAND => {
            if (wp >> 16) & 0xFFFF == 0x0300 {
                update_preview(); // EN_CHANGE
            }
            0
        }
        WM_NCHITTEST => {
            let mut p = POINT { x: (lp & 0xFFFF) as i16 as i32, y: ((lp >> 16) & 0xFFFF) as i16 as i32 };
            ScreenToClient(hwnd, &mut p);
            let dpi = win_scale(hw(&TODO_WND));
            let k = |v: i32| (v as f32 * dpi).round() as i32;
            let lay = layout(&k);
            if p.y < lay.title_h && !contains(&lay.close, p.x, p.y) {
                2 // HTCAPTION: drag by the title bar
            } else {
                1 // HTCLIENT
            }
        }
        WM_MOUSEMOVE => {
            let h = todo_hit(mx, my);
            if HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp >> 16) & 0xFFFF) as i16;
            {
                let mut s = ST.lock().unwrap();
                if delta > 0 {
                    s.todo_scroll = s.todo_scroll.saturating_sub(1);
                } else if LIST_MORE.load(Relaxed) != 0 {
                    s.todo_scroll = (s.todo_scroll + 1).min(s.todos.len().saturating_sub(1));
                }
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            SetFocus(hw(&TODO_EDIT));
            let hit = todo_hit(mx, my);
            match hit {
                10 => {
                    ShowWindow(hwnd, SW_HIDE);
                }
                0 => add_from_edit(false),
                1..=4 => todo_op(hit as u32 - 1),
                5 => add_from_edit(true),
                _ => {
                    // a click inside the list selects the row; the checkbox ticks it
                    let dpi = win_scale(hw(&TODO_WND));
                    let k = |v: i32| (v as f32 * dpi).round() as i32;
                    let l = layout(&k).list;
                    if contains(&l, mx, my) {
                        let hit = ROWS.lock().unwrap().iter().find(|r| my >= r.1 && my < r.2).copied();
                        if let Some((id, _, _, box_r)) = hit {
                            ST.lock().unwrap().todo_sel = Some(id);
                            if mx < box_r {
                                todo_op_on(0, Some(id));
                            }
                        }
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
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
