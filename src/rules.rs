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
