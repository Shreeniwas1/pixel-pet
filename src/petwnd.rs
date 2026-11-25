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
        Act::Pet => {
            let hits = {
                let s = ST.lock().unwrap();
                s.click_hits && !s.menu
            };
            if hits {
                let mut wr: RECT = zeroed();
                GetWindowRect(hwnd, &mut wr);
                let mut s = ST.lock().unwrap();
                let mid = (s.pet_rect.left + s.pet_rect.right) / 2;
                let from = if x < mid { -1.0 } else { 1.0 };
                body::hit(&mut s, from, ((wr.left + x) as f32, (wr.top + y) as f32));
            } else {
                toggle_hub(hwnd);
            }
        }
        Act::Fact => {
            let mut s = ST.lock().unwrap();
            s.fact_left = 0;
            s.msg_left = 0;
        }
        Act::None => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

/// Give the pet something; `None` is its favourite.
pub(crate) unsafe fn feed_pet(hwnd: HWND, food: Option<usize>) {
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let mut s = ST.lock().unwrap();
    let food = food.unwrap_or(PETS[s.pet].foods[0]);
    let (fx, fy) = geo(&s).feet();
    let feet = (wr.left + fx, wr.top + fy);
    let floor = if s.gravity { work_at(feet.0, feet.1 - 10).bottom } else { feet.1 };
    body::feed(&mut s, food, (feet.0 as f32, feet.1 as f32), floor as f32);
}

unsafe fn open_code(hwnd: HWND) {
    if !tools::open_code() {
        say_text(&mut ST.lock().unwrap(), "I can't find VS Code on this PC. Is it installed?", 16);
        InvalidateRect(hwnd, null(), 0);
    }
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
        MAct::Back => {
            let mut s = ST.lock().unwrap();
            s.page = parent_page(s.page);
        }
        MAct::Pets => ST.lock().unwrap().page = 1,
        MAct::Options => ST.lock().unwrap().page = PAGE_OPTIONS,
        MAct::Group(g) => ST.lock().unwrap().page = PAGE_GROUP + g,
        MAct::Minimal => {
            reanchor(hwnd, close_menu);
            toggle_minimal(hwnd);
        }
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
        MAct::Nap => {
            reanchor(hwnd, close_menu);
            let mut s = ST.lock().unwrap();
            let focusing = s.focus && s.pomo_running;
            if !focusing {
                s.mood.start_nap();
            }
        }
        MAct::FeedPage => ST.lock().unwrap().page = PAGE_FEED,
        MAct::Feed(i) => {
            reanchor(hwnd, close_menu);
            feed_pet(hwnd, Some(i));
        }
        MAct::Calc => {
            reanchor(hwnd, close_menu);
            tools::show_calc();
        }
        MAct::Notes => {
            reanchor(hwnd, close_menu);
            tools::show_notes();
        }
        MAct::Code => {
            reanchor(hwnd, close_menu);
            open_code(hwnd);
        }
        MAct::Trick => {
            reanchor(hwnd, close_menu);
            body::perform(&mut ST.lock().unwrap());
        }
        MAct::Quit => {
            DestroyWindow(hwnd);
        }
        MAct::SetPet(i) => {
            reanchor(hwnd, |s| {
                s.body.stop();
                s.body.meal = None;
                s.pet = i.min(PETS.len() - 1);
            });
        }
        MAct::Opt(i) => {
            if i == 6 {
                let next = (ST.lock().unwrap().size + 1) % SIZES.len();
                set_size(hwnd, next);
                return;
            }
            if i == 9 {
                ST.lock().unwrap().page = PAGE_POSITION;
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
            if i == 20 {
                {
                    let mut s = ST.lock().unwrap();
                    s.on_top = !s.on_top;
                    save_cfg(&s);
                }
                presence::sync(hwnd);
                InvalidateRect(hwnd, null(), 0);
                return;
            }
            if i == 21 {
                let on = {
                    let mut s = ST.lock().unwrap();
                    s.through = !s.through;
                    save_cfg(&s);
                    s.through
                };
                if on {
                    // the pet can no longer be clicked, so say how to get back here
                    reanchor(hwnd, close_menu);
                    say_text(&mut ST.lock().unwrap(), "Clicks now pass through me. Ctrl+Alt+P or my tray icon opens the hub.", 24);
                }
                InvalidateRect(hwnd, null(), 0);
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
                12 => {
                    s.walk_on = !s.walk_on;
                    s.body.stop();
                }
                13 => s.tokens_on = !s.tokens_on,
                14 => s.cache_on = !s.cache_on,
                16 => {
                    s.theme = (s.theme + 1) % THEMES.len();
                    set_theme(s.theme);
                }
                17 => {
                    s.opacity = (s.opacity + 1) % OPACITIES.len();
                    crate::tick::OPACITY_PREVIEW.store(5, Relaxed); // show it for ~2 s even though the hub is open
                }
                18 => s.show_speed = !s.show_speed,
                19 => s.show_pc = !s.show_pc,
                22 => {
                    s.tricks_on = !s.tricks_on;
                    s.body.stop();
                }
                23 => s.gravity = !s.gravity,
                24 => s.click_hits = !s.click_hits,
                25 => s.nap_after = (s.nap_after + 1) % NAP_AFTER.len(),
                15 => {
                    let want = !s.autostart;
                    // only believe the toggle if Windows accepted the change
                    if autostart::set_enabled(want) {
                        s.autostart = want;
                    }
                }
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
            SetTimer(hwnd, 2, body::FRAME_MS, None);
            0
        }
        WM_TIMER => {
            if wp == 2 {
                track_hover(hwnd);
                body::step(hwnd);
                return 0;
            }
            on_tick(hwnd);
            0
        }
        WM_HOTKEY => {
            match wp {
                1 => show_todo(),
                2 => reanchor(hwnd, |s| s.focus = !s.focus),
                3 => show_stats(),
                4 => toggle_minimal(hwnd),
                5 => toggle_hidden(hwnd),
                6 => toggle_hub(hwnd),
                7 => feed_pet(hwnd, None),
                8 => {
                    DestroyWindow(hwnd);
                }
                9 => tools::show_calc(),
                10 => tools::show_notes(),
                11 => open_code(hwnd),
                99 if std::env::var_os("PIXEL_PET_TEST").is_some() => {
                    body::travel(&mut ST.lock().unwrap());
                }
                _ => {}
            }
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DPICHANGED => 0, // body::step follows the monitor by itself
        instance::WM_COME_OUT => {
            ST.lock().unwrap().hidden = false;
            presence::sync(hwnd);
            body::perform(&mut ST.lock().unwrap());
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            ST.lock().unwrap().body.screens.clear();
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
            s.body.on_pet = false;
            if !s.menu && contains(&s.pet_rect, mx, my) {
                let (fx, fy) = geo(&s).feet();
                body::grab(&mut s, (p.x, p.y), ((wr.left + fx) as f32, (wr.top + fy) as f32));
            }
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
                    if s.dragging && s.body.on_pet {
                        body::drag(&mut s, (dx, dy)); // the body follows in its own time
                        None
                    } else if s.dragging {
                        Some((s.win0.0 + dx, s.win0.1 + dy, geo(&s)))
                    } else {
                        None
                    }
                }
            };
            if let Some((x, y, g)) = target {
                let wa = work_at(p.x, p.y);
                let (x, y) = clamp_in(&g.bounds(&wa), x, y, g.w, g.h);
                SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            let (was_down, dragged) = {
                let mut s = ST.lock().unwrap();
                let r = (s.down, s.dragging);
                s.down = false;
                s.dragging = false;
                if r.1 && s.body.on_pet {
                    body::release(&mut s);
                }
                s.body.on_pet = false;
                r
            };
            ReleaseCapture();
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
        WM_CAPTURECHANGED => {
            // something else took the mouse away in the middle of a drag
            let mut s = ST.lock().unwrap();
            s.down = false;
            s.dragging = false;
            0
        }
        WM_RBUTTONUP => {
            // right click opens the same pixel hub as left click (no native menus)
            toggle_hub(hwnd);
            0
        }
        tray::WM_TRAY => {
            tray::on_message(hwnd, lp);
            0
        }
        WM_DESTROY => {
            tray::remove(hwnd);
            tools::save_note();
            {
                let s = ST.lock().unwrap();
                save_cfg(&s);
                save_usage(&s);
                save_todos(&s);
            }
            PostQuitMessage(0);
            0
        }
        m if tray::is_taskbar_created(m) => {
            tray::add(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
