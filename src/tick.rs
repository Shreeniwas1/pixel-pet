//! The half-second heartbeat: network, mood, reminders, break coach, tour, do-not-disturb.

use crate::prelude::*;

// ======================================================================
// periodic tick: network, usage, facts, reminders, fullscreen check
// ======================================================================

pub(crate) unsafe fn on_tick(hwnd: HWND) {
    let (now_in, now_out) = totals();
    let (now, dkey, mkey) = local_now();
    let idle = idle_secs();
    let (ram, bat) = (ram_percent(), battery());
    let mut cpu_prev = ST.lock().unwrap().cpu_prev;
    let cpu_now = cpu_percent(&mut cpu_prev);
    let mut beep = false;
    let keep_top;
    let dnd_on = ST.lock().unwrap().dnd_on;
    let full = dnd_on && fullscreen_active(hwnd); // checked every tick: it is cheap
    let was_quiet;
    let mut close_hub = false;
    {
        let mut s = ST.lock().unwrap();
        was_quiet = s.quiet;
        s.quiet = full;
        s.tick += 1;
        let d_in = now_in.wrapping_sub(s.last_in) as f64;
        let d_out = now_out.wrapping_sub(s.last_out) as f64;
        s.last_in = now_in;
        s.last_out = now_out;
        // ignore bogus jumps (adapter appeared/disappeared)
        if d_in < 1.5e9 && d_out < 1.5e9 {
            let k = 1000.0 / TICK_MS as f64;
            s.speed = s.speed * 0.4 + d_in * k * 0.6;
            s.up = s.up * 0.4 + d_out * k * 0.6;
            if s.day_key != dkey {
                s.day_key = dkey;
                s.day_rx = 0;
                s.day_tx = 0;
            }
            if s.mon_key != mkey {
                s.mon_key = mkey;
                s.mon_rx = 0;
                s.mon_tx = 0;
            }
            s.day_rx += d_in as u64;
            s.day_tx += d_out as u64;
            s.mon_rx += d_in as u64;
            s.mon_tx += d_out as u64;
        }
        if s.speed < 500.0 {
            s.speed = 0.0;
        }
        if s.up < 500.0 {
            s.up = 0.0;
        }

        // the pet's inner life: mood, sleep, and speech
        s.msg_left = s.msg_left.saturating_sub(1);
        let focusing = s.focus && s.pomo_running;
        if let Some(ev) = s.mood.tick(idle, ((now / 60) % 24) as u32, focusing) {
            if !full {
                say(&mut s, ev);
            }
        }
        s.mood.last_run = now;
        s.cpu_prev = cpu_prev;
        if let Some(c) = cpu_now {
            s.cpu = c;
        }

        // break coach (never interrupts a running focus round) and system alerts
        let deep_focus = s.focus && s.pomo_running && s.pomo_phase == 0;
        let care = s.coach.tick(idle, deep_focus);
        let cpu_now_pct = s.cpu;
        let alert_sys = s.watch.tick(cpu_now_pct, ram, bat);
        if s.alert.is_none() && !full {
            if let (Some(c), true) = (care, s.care_on) {
                say_text(&mut s, care::care_text(c), 24);
            } else if let (Some(a), true) = (alert_sys, s.sys_on) {
                say_text(&mut s, &care::sys_text(a), 24);
            }
        }

        // first-run tour: a few friendly bubbles, one after another
        if s.intro != intro::DONE && s.alert.is_none() && !full && s.msg_left == 0 && !s.menu {
            if s.intro_wait > 0 {
                s.intro_wait -= 1;
            } else {
                let i = s.intro as usize;
                say_text(&mut s, intro::STEPS[i], 34);
                s.intro += 1;
                s.intro_wait = 3;
                if s.intro as usize >= intro::STEPS.len() {
                    s.intro = intro::DONE;
                    save_cfg(&s);
                }
            }
        }

        // facts
        if s.fact_left > 0 {
            s.fact_left -= 1;
        } else if s.facts_on && !full && !s.focus && s.alert.is_none() && s.msg_left == 0 && s.tick % FACT_EVERY == 10 {
            next_fact(&mut s);
            s.fact_left = FACT_FOR;
        }

        // pomodoro finished?
        if s.pomo_running && pomo_remaining(&s) == 0 {
            let was_focus = s.pomo_phase == 0;
            pomo_advance(&mut s, true);
            if was_focus {
                let mins = (pomo_len(&s) / 60) as u32;
                let ev = s.mood.on_focus_done();
                say(&mut s, ev);
                if s.stats.record((now / 1440) as u32, mins) {
                    let g = s.stats.goal;
                    say_text(&mut s, &format!("Daily goal reached: {g} focus sessions! Amazing."), 24);
                }
                save_stats(&s);
            }
            beep = true;
        }

        // reminders: first overdue, not-yet-shown item
        if s.alert.is_none() && !full {
            if let Some(t) = s.todos.iter_mut().find(|t| !t.done && !t.fired && t.due.map_or(false, |d| d <= now)) {
                t.fired = true;
                let id = t.id;
                s.alert = Some(id);
                s.fact_left = 0;
                beep = true;
            }
        }

        if s.menu {
            s.menu_ticks += 1;
            close_hub = s.menu_ticks > 80 && s.alert.is_none();
        }
        keep_top = true; // other topmost windows (the taskbar!) can push us down; reassert every tick
        if s.tick % 120 == 0 {
            save_cfg(&s);
            save_usage(&s);
        }
    }
    if close_hub {
        reanchor(hwnd, close_menu);
    }
    if full != was_quiet {
        // slip away while a game or video is fullscreen, come back when it ends
        ShowWindow(hwnd, if full { SW_HIDE } else { SW_SHOWNOACTIVATE });
    }
    if beep && !full {
        MessageBeep(MB_ICONASTERISK);
        refresh_list();
    }
    if keep_top && !full {
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
    InvalidateRect(hwnd, null(), 0);
}
