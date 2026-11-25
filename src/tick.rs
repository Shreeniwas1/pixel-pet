//! The half-second heartbeat: network, mood, reminders, break coach, tour, do-not-disturb.

use crate::prelude::*;

// ======================================================================
// periodic tick: network, usage, facts, reminders, fullscreen check
// ======================================================================

/// Half-seconds left of showing the chosen opacity even with the hub open (so changing it is visible).
pub(crate) static OPACITY_PREVIEW: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

static WAS_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Half-seconds until the pet comes back out of hiding.
static COAST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

static LAST_ALPHA: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// Dim the idle pet to the chosen opacity; menus, messages and focus cards stay fully readable.
unsafe fn apply_opacity(hwnd: HWND) {
    let want = {
        let s = ST.lock().unwrap();
        let previewing = OPACITY_PREVIEW.load(Relaxed) > 0;
        if previewing {
            OPACITY_PREVIEW.fetch_sub(1, Relaxed);
        }
        if !previewing && (s.menu || s.alert.is_some() || s.msg_left > 0 || s.fact_left > 0 || s.focus) {
            255
        } else {
            (OPACITIES[s.opacity.min(OPACITIES.len() - 1)] * 255 / 100) as i32
        }
    };
    if LAST_ALPHA.swap(want, Relaxed) != want {
        SetLayeredWindowAttributes(hwnd, KEY, want as u8, LWA_COLORKEY | LWA_ALPHA);
    }
}

pub(crate) unsafe fn on_tick(hwnd: HWND) {
    apply_opacity(hwnd);
    // the interface counters cost a few ms to read, so sample them every other tick (1 s)
    let poll_net = ST.lock().unwrap().tick % 2 == 0;
    let (now_in, now_out) = if poll_net { totals() } else { (0, 0) };
    let (now, dkey, mkey) = local_now();
    let idle = idle_secs();
    let (ram, bat) = (ram_percent(), battery());
    let mut cpu_prev = ST.lock().unwrap().cpu_prev;
    let cpu_now = cpu_percent(&mut cpu_prev);
    let mut beep = false;
    let (dnd_on, list) = {
        let s = ST.lock().unwrap();
        (s.dnd_on, s.hide_for.clone())
    };
    // what is in front changes slowly and costs a few system calls, so look once a second
    let look = ST.lock().unwrap().tick % 2 == 1;
    let (busy, app) = if !look { (WAS_BUSY.load(Relaxed), String::new()) } else if std::env::var_os("PIXEL_PET_TEST").is_some_and(|v| v != "busy") { (false, String::new()) } else { busy_with(&list) };
    WAS_BUSY.store(busy, Relaxed);
    // hide at once, but only come back when the coast has been clear for a moment
    let wait = if dnd_on && busy { 4 } else { COAST.load(Relaxed).saturating_sub(1) };
    COAST.store(wait, Relaxed);
    let full = wait > 0;
    let mut close_hub = false;
    {
        let mut s = ST.lock().unwrap();
        s.quiet = full;
        if !app.is_empty() && !app.eq_ignore_ascii_case("explorer.exe") {
            s.last_app = app;
        }
        let chatty = presence::chatty(s.minimal, s.hidden);
        s.tick += 1;
        let (d_in, d_out) = if poll_net {
            let d = (now_in.wrapping_sub(s.last_in) as f64, now_out.wrapping_sub(s.last_out) as f64);
            s.last_in = now_in;
            s.last_out = now_out;
            d
        } else {
            (f64::MAX, f64::MAX) // no sample this tick
        };
        // ignore bogus jumps (adapter appeared/disappeared)
        if poll_net && d_in < 1.5e9 && d_out < 1.5e9 {
            let k = 1000.0 / (2 * TICK_MS) as f64;
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
        // left alone for long enough, it dozes off until it is rested or someone touches it
        let patience = NAP_AFTER[s.nap_after] as f32;
        let lonely = patience > 0.0 && s.body.ignored > patience;
        if lonely && s.mood.energy >= 100.0 {
            s.body.ignored = 0.0;
        }
        let idle_felt = if lonely { idle.max(300) } else { idle };
        if let Some(ev) = s.mood.tick(idle_felt, ((now / 60) % 24) as u32, focusing) {
            if !full {
                say(&mut s, ev);
            }
        }
        s.mood.last_run = now;
        s.cpu_prev = cpu_prev;
        if let Some(c) = cpu_now {
            s.cpu = c;
        }
        s.ram = ram;

        // break coach (never interrupts a running focus round) and system alerts
        let deep_focus = s.focus && s.pomo_running && s.pomo_phase == 0;
        let care = s.coach.tick(idle, deep_focus);
        let cpu_now_pct = s.cpu;
        let alert_sys = s.watch.tick(cpu_now_pct, ram, bat);
        if s.alert.is_none() && !full && chatty {
            if let (Some(c), true) = (care, s.care_on) {
                say_text(&mut s, care::care_text(c), 24);
            } else if let (Some(a), true) = (alert_sys, s.sys_on) {
                say_text(&mut s, &care::sys_text(a), 24);
            }
        }

        // the pet notices big AI-token days (never at startup, never over another message)
        if s.tokens_on && s.intro == intro::DONE {
            let total = {
                let t = TOK.lock().unwrap();
                if t.found { Some(t.sum(1).fresh()) } else { None }
            };
            if let Some(total) = total {
                let lvl = tokens::milestone(total);
                if s.tok_day != dkey {
                    s.tok_day = dkey;
                    s.tok_mile = lvl; // a new day starts from what is already there
                }
                if !s.tok_seen {
                    s.tok_seen = true;
                    s.tok_mile = lvl;
                } else if lvl > s.tok_mile && chatty && s.alert.is_none() && !full && s.msg_left == 0 && !s.menu {
                    s.tok_mile = lvl;
                    say_text(&mut s, &tokens::milestone_text(lvl), 24);
                }
            }
        }

        // first-run tour: a few friendly bubbles, one after another
        let can_speak = s.alert.is_none() && !full && !s.hidden && s.msg_left == 0 && !s.menu;
        let (mut step, mut wait) = (s.intro, s.intro_wait);
        let said = intro::advance(&mut step, &mut wait, can_speak);
        s.intro_wait = wait;
        if let Some(i) = said {
            say_text(&mut s, intro::STEPS[i], intro::STEP_TICKS);
        }
        if step != s.intro {
            s.intro = step;
            if step == intro::DONE {
                save_cfg(&s); // remember the tour is over, even if the app is closed right now
            }
        }

        // facts
        if s.fact_left > 0 {
            s.fact_left -= 1;
        } else if s.facts_on && chatty && intro::facts_allowed(s.intro, s.intro_wait) && !full && !s.focus && s.alert.is_none() && s.msg_left == 0 && s.tick % FACT_EVERY == 10 {
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
        if s.tick % 120 == 0 {
            save_cfg(&s);
            save_usage(&s);
        }
    }
    if close_hub {
        reanchor(hwnd, close_menu);
    }
    presence::sync(hwnd); // slip away behind fullscreen games, come out for reminders, stay on top
    if beep && !full {
        MessageBeep(MB_ICONASTERISK);
        refresh_list();
    }
    InvalidateRect(hwnd, null(), 0);
}
