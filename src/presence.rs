//! How much of the screen the pet takes up: minimal mode, hiding, z-order, click-through, hover.

use crate::prelude::*;

/// One sprite pixel in minimal mode (half the usual size).
pub(crate) const MINI_PX: i32 = 3;

// ---- rules (pure, so they can be tested) ----

/// May the pet speak without being asked (facts, chatter, coach, system alerts)?
pub(crate) fn chatty(minimal: bool, hidden: bool) -> bool {
    !minimal && !hidden
}

/// Is this mood line worth saying? A finished focus round was asked for, so it always is.
pub(crate) fn says(minimal: bool, hidden: bool, ev: Ev) -> bool {
    chatty(minimal, hidden) || ev == Ev::Focus
}

/// Is the pet window on screen? A hidden pet still comes out for a reminder, the hub or a message.
pub(crate) fn shown(hidden: bool, fullscreen: bool, busy: bool) -> bool {
    !fullscreen && (!hidden || busy)
}

/// Do mouse clicks fall through the pet? Never while there is something to click.
pub(crate) fn passes_clicks(pref: bool, interactive: bool) -> bool {
    pref && !interactive
}

/// The smallest rectangle holding both.
pub(crate) fn union(a: RECT, b: RECT) -> RECT {
    RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) }
}

// ---- applying them to the window ----

/// Bring visibility, z-order and click-through in line with the current state.
pub(crate) unsafe fn sync(hwnd: HWND) {
    let (show, top, through) = {
        let s = ST.lock().unwrap();
        let needs_you = s.menu || s.alert.is_some();
        let talking = s.msg_left > 0 || s.fact_left > 0;
        (
            s.menu || shown(s.hidden, s.quiet, needs_you || talking), // the hub was asked for
            s.on_top || needs_you,
            passes_clicks(s.through, needs_you || s.focus),
        )
    };
    if (IsWindowVisible(hwnd) != 0) != show {
        ShowWindow(hwnd, if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let want = if through { ex | WS_EX_TRANSPARENT } else { ex & !WS_EX_TRANSPARENT };
    if want != ex {
        SetWindowLongW(hwnd, GWL_EXSTYLE, want as i32);
    }
    // other topmost windows (the taskbar!) can push us down, so topmost is reasserted every time;
    // dropping out of it happens once, or the pet would keep jumping over normal windows
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    if show && top {
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags);
    } else if !top && ex & WS_EX_TOPMOST != 0 {
        SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
    }
}

/// 50 ms timer: in minimal mode the details only appear while the mouse is over the pet.
pub(crate) unsafe fn track_hover(hwnd: HWND) {
    let (minimal, was, r) = {
        let s = ST.lock().unwrap();
        (s.minimal, s.hover, s.hover_rect)
    };
    if !minimal && !was {
        return;
    }
    let mut p: POINT = zeroed();
    GetCursorPos(&mut p);
    ScreenToClient(hwnd, &mut p);
    let over = minimal && contains(&r, p.x, p.y);
    if over != was {
        ST.lock().unwrap().hover = over;
        InvalidateRect(hwnd, null(), 0);
    }
}

pub(crate) unsafe fn toggle_minimal(hwnd: HWND) {
    toggle_minimal_here(hwnd);
    friend::reload();
}

unsafe fn toggle_minimal_here(hwnd: HWND) {
    reanchor(hwnd, |s| {
        s.minimal = !s.minimal;
        if s.minimal {
            // stop whatever it was doing on its own
            s.body.stop();
            s.msg_left = 0;
            s.fact_left = 0;
            if s.intro != intro::DONE {
                s.intro = intro::DONE;
                s.intro_wait = 0;
            }
        }
    });
}

pub(crate) unsafe fn toggle_hidden(hwnd: HWND) {
    let close = {
        let mut s = ST.lock().unwrap();
        s.hidden = !s.hidden;
        if s.hidden {
            s.msg_left = 0;
            s.fact_left = 0;
            s.body.stop();
        }
        s.hidden && s.menu
    };
    if close {
        reanchor(hwnd, close_menu);
    }
    sync(hwnd);
}

pub(crate) unsafe fn toggle_hub(hwnd: HWND) {
    reanchor(hwnd, |s| {
        s.menu = !s.menu;
        s.page = 0;
        s.menu_ticks = 0;
        if s.menu {
            s.msg_left = 0; // a speech bubble would sit on top of the hub
            s.fact_left = 0;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_and_hidden_pets_do_not_speak_unasked() {
        assert!(chatty(false, false));
        for (minimal, hidden) in [(true, false), (false, true), (true, true)] {
            assert!(!chatty(minimal, hidden));
            for ev in [Ev::Wake, Ev::Morning, Ev::Task, Ev::Pet, Ev::Late, Ev::Tired, Ev::Sad, Ev::Back] {
                assert!(!says(minimal, hidden, ev), "{ev:?} is chatter");
            }
            assert!(says(minimal, hidden, Ev::Focus), "a finished focus round must be announced");
        }
    }

    #[test]
    fn a_hidden_pet_comes_out_only_when_it_is_needed_and_never_over_fullscreen() {
        assert!(shown(false, false, false));
        assert!(!shown(true, false, false));
        assert!(shown(true, false, true));
        assert!(!shown(true, true, true));
        assert!(!shown(false, true, true));
    }

    #[test]
    fn click_through_lets_go_while_there_is_something_to_click() {
        assert!(passes_clicks(true, false));
        assert!(!passes_clicks(true, true));
        assert!(!passes_clicks(false, false));
    }

    #[test]
    fn union_covers_both() {
        let u = union(rc(10, 10, 5, 5), rc(0, 12, 4, 20));
        assert_eq!((u.left, u.top, u.right, u.bottom), (0, 10, 15, 32));
    }
}
