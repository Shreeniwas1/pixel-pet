//! Window-placement rules kept free of Win32 calls so they can be unit tested.

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Shell windows that always span the screen but are not "a fullscreen app".
const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// Is this foreground window a genuine fullscreen app (game, video, slideshow, F11 browser)?
///
/// A *maximized* window can cover the whole monitor too (for example when the taskbar auto-hides),
/// and a normal window has a title bar, so neither counts. Getting this wrong hides the pet while
/// the user is just working in a maximized editor.
pub fn is_fullscreen_app(win: Rect, monitor: Rect, maximized: bool, has_caption: bool, class: &str) -> bool {
    if maximized || has_caption || SHELL_CLASSES.contains(&class) {
        return false;
    }
    win.left <= monitor.left && win.top <= monitor.top && win.right >= monitor.right && win.bottom >= monitor.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: Rect = Rect { left: 0, top: 0, right: 1536, bottom: 864 };

    #[test]
    fn maximized_editor_with_autohide_taskbar_is_not_fullscreen() {
        // The real case that hid the pet: Zed maximized, taskbar auto-hidden, so work area == monitor.
        let zed = Rect { left: -7, top: -7, right: 1543, bottom: 871 };
        assert!(!is_fullscreen_app(zed, MON, true, false, "Zed::Window"));
        // even if a maximized app reports no caption (custom title bar)
        assert!(!is_fullscreen_app(zed, MON, true, true, "Chrome_WidgetWin_1"));
    }

    #[test]
    fn a_normal_window_with_a_title_bar_is_not_fullscreen() {
        let r = Rect { left: 0, top: 0, right: 1536, bottom: 864 };
        assert!(!is_fullscreen_app(r, MON, false, true, "Notepad"));
    }

    #[test]
    fn borderless_game_covering_the_monitor_is_fullscreen() {
        assert!(is_fullscreen_app(MON, MON, false, false, "UnityWndClass"));
        let bigger = Rect { left: -1, top: -1, right: 1537, bottom: 865 };
        assert!(is_fullscreen_app(bigger, MON, false, false, "SDL_app"));
    }

    #[test]
    fn small_or_partial_windows_are_not_fullscreen() {
        let half = Rect { left: 0, top: 0, right: 768, bottom: 864 };
        assert!(!is_fullscreen_app(half, MON, false, false, "Popup"));
    }

    #[test]
    fn the_desktop_and_taskbar_never_count() {
        for c in ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"] {
            assert!(!is_fullscreen_app(MON, MON, false, false, c));
        }
    }
}

/// Vertical layout of the focus-mode stack that sits next to the pet.
///
/// Order, starting at the pet: timer card, then either the tasks card or (while the pet is
/// saying something) the speech bubble. The bubble takes the tasks card's place instead of
/// being drawn over the timer, so the timer buttons are never covered.
pub struct FocusGeom {
    pub timer_y: i32,
    pub tasks_y: i32,
    pub bubble_y: i32,
}

/// `base_y`/`pet_h` describe the pet sprite; `flipy` is true when the pet sits at the top.
pub fn focus_geom(base_y: i32, pet_h: i32, flipy: bool, pet_gap: i32, timer_h: i32, tasks_h: i32, bubble_h: i32, gap: i32) -> FocusGeom {
    if flipy {
        let timer_y = base_y + pet_h + pet_gap;
        let beyond = timer_y + timer_h + gap;
        FocusGeom { timer_y, tasks_y: beyond, bubble_y: beyond }
    } else {
        let timer_y = base_y - pet_gap - timer_h;
        FocusGeom { timer_y, tasks_y: timer_y - gap - tasks_h, bubble_y: timer_y - gap - bubble_h }
    }
}

#[cfg(test)]
mod focus_layout_tests {
    use super::*;

    /// Every scale the app can run at: DPI (100-200%) times the three pet sizes.
    fn scales() -> Vec<f32> {
        let mut v = Vec::new();
        for dpi in [1.0f32, 1.25, 1.5, 1.75, 2.0] {
            for size in [0.75f32, 1.0, 1.4] {
                v.push(dpi * size);
            }
        }
        v
    }

    fn overlap(a: (i32, i32), b: (i32, i32)) -> bool {
        a.0 < b.1 && b.0 < a.1
    }

    #[test]
    fn bubble_never_covers_the_timer_card_or_leaves_the_window() {
        for sc in scales() {
            let u = |v: i32| (v as f32 * sc).round() as i32;
            let win_h = u(crate::state::FOCUS_H);
            let (timer_h, gap) = (u(118), u(8));
            for pet_rows in [11, 15] {
                // the biggest pet sprite is 15 rows of 6 px
                let pet_h = pet_rows * u(6);
                for flipy in [false, true] {
                    let base_y = if flipy { u(8) } else { win_h - pet_h - u(8) };
                    // an alert with a 3-line message is the tallest bubble; also try a short one
                    for bubble_h in [u(30), u(56), 2 * (u(3) + u(10)) + u(18) + 3 * u(16) + u(8) + u(24)] {
                        let g = focus_geom(base_y, pet_h, flipy, u(12), timer_h, u(70), bubble_h, gap);
                        let timer = (g.timer_y, g.timer_y + timer_h);
                        let bubble = (g.bubble_y, g.bubble_y + bubble_h);
                        let pet = (base_y, base_y + pet_h);
                        assert!(!overlap(bubble, timer), "bubble covers the timer (scale {sc}, flipy {flipy})");
                        assert!(!overlap(bubble, pet), "bubble covers the pet (scale {sc}, flipy {flipy})");
                        assert!(bubble.0 >= 0 && bubble.1 <= win_h, "bubble leaves the window (scale {sc}, flipy {flipy}, h {bubble_h})");
                        assert!(timer.0 >= 0 && timer.1 <= win_h);
                    }
                }
            }
        }
    }

    #[test]
    fn tasks_card_stacks_beyond_the_timer_without_overlap() {
        for sc in scales() {
            let u = |v: i32| (v as f32 * sc).round() as i32;
            let win_h = u(crate::state::FOCUS_H);
            let (timer_h, gap) = (u(118), u(8));
            // tasks card heights: no tasks .. five tasks plus the "+N more" line
            for tasks_h in [u(70), u(122), 2 * (u(3) + u(8)) + u(22) + 5 * u(26) + u(18)] {
                for flipy in [false, true] {
                    let pet_h = 11 * u(6);
                    let base_y = if flipy { u(8) } else { win_h - pet_h - u(8) };
                    let g = focus_geom(base_y, pet_h, flipy, u(12), timer_h, tasks_h, 0, gap);
                    let timer = (g.timer_y, g.timer_y + timer_h);
                    let tasks = (g.tasks_y, g.tasks_y + tasks_h);
                    assert!(!overlap(timer, tasks), "tasks card overlaps the timer (scale {sc}, flipy {flipy})");
                    assert!(tasks.0 >= 0 && tasks.1 <= win_h, "tasks card leaves the window (scale {sc}, flipy {flipy})");
                }
            }
        }
    }
}
