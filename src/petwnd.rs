//! The pet window: clicks, hub actions and the window procedure.

use crate::prelude::*;

// ======================================================================
// pet window: mouse, menu, messages
// ======================================================================

pub(crate) unsafe fn handle_click(hwnd: HWND, x: i32, y: i32) {
    enum Act {
        Focus(FAct),
        Alert(u8),
        Menu(MAct),
        Pet,
        Fact,
        None,
    }
    let act = {
        let s = ST.lock().unwrap();
        if let Some((_, a)) = s.frects.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Focus(*a)
        } else if let Some((_, a)) = s.btns.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Alert(*a)
        } else if let Some((_, a)) = s.mrects.iter().find(|(r, _)| contains(r, x, y)).filter(|_| s.menu) {
            Act::Menu(*a)
        } else if contains(&s.pet_rect, x, y) {
            Act::Pet
        } else if s.fact_rect.map_or(false, |r| contains(&r, x, y)) {
            Act::Fact
        } else {
            Act::None
        }
    };
    match act {
        Act::Focus(a) => match a {
            FAct::Toggle => pomo_toggle(&mut ST.lock().unwrap()),
            FAct::Reset => pomo_reset(&mut ST.lock().unwrap()),
            FAct::Skip => pomo_advance(&mut ST.lock().unwrap(), false),
            FAct::Exit => reanchor(hwnd, |s| s.focus = false),
            FAct::Task(id) => {
                {
                    let mut s = ST.lock().unwrap();
                    let (now, ..) = local_now();
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        todo::complete(t, now);
                    }
                    if s.alert == Some(id) {
                        s.alert = None;
                    }
                    let ev = s.mood.on_task_done();
                    say(&mut s, ev);
                    save_todos(&s);
                }
                refresh_list();
            }
        },
        Act::Alert(a) => {
            let (now, ..) = local_now();
            {
                let mut s = ST.lock().unwrap();
                if let Some(id) = s.alert.take() {
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        match a {
                            0 => {
                                todo::complete(t, now);
                                let ev = s.mood.on_task_done();
                                say(&mut s, ev);
                            }
                            1 => {
                                t.due = Some(now + 10);
                                t.fired = false;
                            }
                            _ => {} // later: stays in the list, marked overdue
                        }
                    }
                    save_todos(&s);
                }
            }
            refresh_list();
        }
        Act::Menu(a) => menu_action(hwnd, a),
        Act::Pet => reanchor(hwnd, |s| {
            s.menu = !s.menu;
            s.page = 0;
            s.menu_ticks = 0;
        }),
        Act::Fact => {
            let mut s = ST.lock().unwrap();
            s.fact_left = 0;
            s.msg_left = 0;
        }
        Act::None => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) fn close_menu(s: &mut State) {
    s.menu = false;
    s.page = 0;
}

/// Something in the pixel hub was clicked.
pub(crate) unsafe fn menu_action(hwnd: HWND, a: MAct) {
    ST.lock().unwrap().menu_ticks = 0;
    match a {
        MAct::Close => reanchor(hwnd, close_menu),
        MAct::Back => ST.lock().unwrap().page = 0,
        MAct::Pets => ST.lock().unwrap().page = 1,
        MAct::Options => ST.lock().unwrap().page = 2,
        MAct::Tasks => {
            reanchor(hwnd, close_menu);
            show_todo();
        }
        MAct::Focus => reanchor(hwnd, |s| {
            close_menu(s);
            s.focus = !s.focus;
        }),
        MAct::Stats => {
            reanchor(hwnd, close_menu);
            show_stats();
        }
        MAct::Corner(n) => {
            reanchor(hwnd, close_menu);
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
            put_in_corner(hwnd, n % 2 == 0, n < 2, wa);
        }
        MAct::NextMonitor => {
            reanchor(hwnd, close_menu);
            next_monitor(hwnd);
        }
        MAct::Quit => {
            DestroyWindow(hwnd);
        }
        MAct::SetPet(i) => {
            let mut s = ST.lock().unwrap();
            s.pet = i.min(PETS.len() - 1);
            save_cfg(&s);
        }
        MAct::Opt(i) => {
            if i == 6 {
                let next = (ST.lock().unwrap().size + 1) % SIZES.len();
                set_size(hwnd, next);
                return;
            }
            if i == 9 {
                ST.lock().unwrap().page = 3;
                return;
            }
            if i == 11 {
                reanchor(hwnd, close_menu);
                let mut s = ST.lock().unwrap();
                s.intro = 0;
                s.intro_wait = 2;
                s.msg_left = 0;
                return;
            }
            if i == 10 {
                reanchor(hwnd, close_menu);
                let mut s = ST.lock().unwrap();
                next_fact(&mut s);
                s.msg_left = 0;
                s.fact_left = FACT_FOR;
                return;
            }
            let mut s = ST.lock().unwrap();
            match i {
                0 => {
                    s.facts_on = !s.facts_on;
                    if !s.facts_on {
                        s.fact_left = 0;
                    }
                }
                1 => s.show_up = !s.show_up,
                2 => s.bits = !s.bits,
                3 => s.care_on = !s.care_on,
                4 => s.sys_on = !s.sys_on,
                5 => s.dnd_on = !s.dnd_on,
                7 => {
                    let goals = [2u32, 4, 6, 8];
                    let at = goals.iter().position(|g| *g == s.stats.goal).unwrap_or(1);
                    s.stats.goal = goals[(at + 1) % goals.len()];
                }
                _ => {
                    s.pomo_preset = (s.pomo_preset + 1) % POMO.len();
                    pomo_reset(&mut s);
                }
            }
            save_cfg(&s);
        }
    }
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_CREATE => {
            SetTimer(hwnd, 1, TICK_MS, None);
            SetTimer(hwnd, 2, 50, None);
            0
        }
        WM_TIMER => {
            if wp == 2 {
                on_walk(hwnd);
                return 0;
            }
            on_tick(hwnd);
            maybe_start(hwnd);
            0
        }
        WM_HOTKEY => {
            match wp {
                1 => show_todo(),
                2 => reanchor(hwnd, |s| s.focus = !s.focus),
                3 => show_stats(),
                _ => {}
            }
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            place(hwnd);
            0
        }
        WM_LBUTTONDOWN => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            SetCapture(hwnd);
            let mut s = ST.lock().unwrap();
            s.down = true;
            s.dragging = false;
            s.cur0 = (p.x, p.y);
            s.win0 = (wr.left, wr.top);
            0
        }
        WM_MOUSEMOVE => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let target = {
                let mut s = ST.lock().unwrap();
                if !s.down {
                    // stroking the pet with the mouse pets it
                    if contains(&s.pet_rect, mx, my) {
                        if let Some((lx, ly)) = s.last_mouse {
                            s.stroke += (mx - lx).abs() + (my - ly).abs();
                        }
                        s.last_mouse = Some((mx, my));
                        if s.stroke > (220.0 * s.scale) as i32 {
                            s.stroke = 0;
                            if let Some(ev) = s.mood.on_petted() {
                                say(&mut s, ev);
                            }
                        }
                    } else {
                        s.last_mouse = None;
                        s.stroke = 0;
                    }
                    None
                } else {
                    let (dx, dy) = (p.x - s.cur0.0, p.y - s.cur0.1);
                    if !s.dragging && (dx.abs() > 4 || dy.abs() > 4) {
                        s.dragging = true;
                    }
                    if s.dragging {
                        Some((s.win0.0 + dx, s.win0.1 + dy, win_dims_s(&s)))
                    } else {
                        None
                    }
                }
            };
            if let Some((x, y, (w, h))) = target {
                let wa = work_at(p.x, p.y);
                let (x, y) = clamp_in(&wa, x, y, w, h);
                SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let (was_down, dragged) = {
                let mut s = ST.lock().unwrap();
                let r = (s.down, s.dragging);
                s.down = false;
                s.dragging = false;
                r
            };
            if was_down {
                if dragged {
                    let mut wr: RECT = zeroed();
                    GetWindowRect(hwnd, &mut wr);
                    let mut s = ST.lock().unwrap();
                    s.pos = Some((wr.left, wr.top));
                    save_cfg(&s);
                } else {
                    handle_click(hwnd, mx, my);
                }
            }
            0
        }
        WM_RBUTTONUP => {
            // right click opens the same pixel hub as left click (no native menus)
            reanchor(hwnd, |s| {
                s.menu = !s.menu;
                s.page = 0;
                s.menu_ticks = 0;
            });
            0
        }
        WM_DESTROY => {
            {
                let s = ST.lock().unwrap();
                save_cfg(&s);
                save_usage(&s);
                save_todos(&s);
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
