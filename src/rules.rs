//! Window-placement rules kept free of Win32 calls so they can be unit tested.

/// The biggest scale at which the tallest pet window (`tallest` logical px) still fits a work
/// area `work_h` physical px high, leaving a little breathing room. Never below 0.5.
pub fn max_scale(work_h: i32, tallest: i32) -> f32 {
    ((work_h as f32 * 0.96) / tallest as f32).max(0.5)
}

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

/// Does this program live where game stores install games? (Launchers and tools do not count.)
pub fn is_game(path: &str) -> bool {
    const STORES: [&str; 9] = [
        "\\steamapps\\common\\", "\\epic games\\", "\\riot games\\", "\\xboxgames\\", "\\gog galaxy\\games\\", "\\ubisoft game launcher\\games\\", "\\ea games\\", "\\origin games\\",
        "\\rockstar games\\",
    ];
    const NOT: [&str; 6] = ["launcher", "wallpaper_engine", "riotclient", "\\unins", "crashhandler", "blender"];
    let p = path.to_lowercase();
    STORES.iter().any(|s| p.contains(s)) && !NOT.iter().any(|n| p.contains(n))
}

/// Is this a film player? (`exe` is the file name.)
pub fn is_player(exe: &str) -> bool {
    const PLAYERS: [&str; 16] = [
        "vlc", "mpv", "mpvnet", "mpc-hc", "mpc-hc64", "mpc-be", "mpc-be64", "potplayer", "potplayermini", "potplayermini64", "wmplayer", "microsoft.media.player", "video.ui", "kodi", "smplayer", "netflix",
    ];
    let e = exe.to_lowercase();
    let e = e.strip_suffix(".exe").unwrap_or(&e);
    PLAYERS.contains(&e) || e.starts_with("kmplayer") || e.starts_with("gom") || e.starts_with("plex")
}

/// Is this browser window on a film streaming site? Judged by its title.
pub fn is_streaming(exe: &str, title: &str) -> bool {
    const BROWSERS: [&str; 9] = ["chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc", "zen", "applicationframehost"];
    const SITES: [&str; 10] = ["netflix", "prime video", "primevideo", "disney+", "hotstar", "sonyliv", "zee5", "crunchyroll", "hulu", "apple tv+"];
    let e = exe.to_lowercase();
    let e = e.strip_suffix(".exe").unwrap_or(&e);
    // a search for the site, or a page about it, is not a film
    let t = title.to_lowercase();
    BROWSERS.contains(&e) && !t.contains("search") && SITES.iter().any(|s| t.contains(s))
}

/// Is `exe` on the user's own list?
pub fn is_listed(list: &[String], exe: &str) -> bool {
    !exe.is_empty() && list.iter().any(|l| l.eq_ignore_ascii_case(exe))
}

#[cfg(test)]
mod hide_tests {
    use super::*;

    #[test]
    fn games_are_recognised_by_where_they_live() {
        assert!(is_game("D:\\SteamLibrary\\steamapps\\common\\Some Game\\game.exe"));
        assert!(is_game("C:\\Riot Games\\VALORANT\\live\\VALORANT.exe"));
        assert!(is_game("C:\\Program Files\\Epic Games\\Thing\\Thing.exe"));
        assert!(!is_game("C:\\Program Files (x86)\\Epic Games\\Launcher\\Portal\\EpicGamesLauncher.exe"));
        assert!(!is_game("C:\\Riot Games\\Riot Client\\RiotClientServices.exe"));
        assert!(!is_game("C:\\Program Files\\Microsoft VS Code\\Code.exe"));
        assert!(!is_game("D:\\SteamLibrary\\steamapps\\common\\wallpaper_engine\\wallpaper64.exe"));
    }

    #[test]
    fn players_and_streaming_sites_are_films_other_pages_are_not() {
        assert!(is_player("VLC.exe") && is_player("PotPlayerMini64.exe") && is_player("Plex.exe"));
        assert!(!is_player("Spotify.exe") && !is_player("chrome.exe") && !is_player("Code.exe"));
        assert!(is_streaming("chrome.exe", "Some Film | Netflix - Google Chrome"));
        assert!(is_streaming("msedge.exe", "Prime Video: The Show"));
        assert!(!is_streaming("chrome.exe", "netflix price - Google Search"));
        assert!(!is_streaming("chrome.exe", "lofi beats - YouTube"));
        assert!(!is_streaming("Code.exe", "netflix-clone - Visual Studio Code"));
    }

    #[test]
    fn the_users_own_list_ignores_case() {
        let l = vec!["Photoshop.exe".to_string()];
        assert!(is_listed(&l, "photoshop.EXE"));
        assert!(!is_listed(&l, "paint.exe") && !is_listed(&l, ""));
    }
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
mod fit_tests {
    use super::*;

    #[test]
    fn tallest_window_fits_every_common_screen() {
        let tallest = crate::state::FOCUS_H.max(crate::state::MENU_H);
        // (work-area height in physical px, DPI scale)
        for (h, dpi) in [(728, 1.0f32), (1040, 1.0), (1040, 1.25), (1040, 1.5), (1040, 2.0), (2120, 2.0), (680, 1.5)] {
            for size in [0.75f32, 1.0, 1.4] {
                let sc = (dpi * size).min(max_scale(h, tallest));
                assert!((tallest as f32 * sc).round() as i32 <= h, "{h}px at {dpi}x size {size}");
            }
        }
    }

    #[test]
    fn roomy_screens_are_not_shrunk() {
        assert!(max_scale(2120, 440) > 4.0);
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
            for pet_rows in [11, crate::cast::PET_MAX_H] {
                // the biggest pet
                let pet_h = pet_rows * u(6);
                for flipy in [false, true] {
                    let base_y = if flipy { u(crate::body::PAD_TOP) } else { win_h - pet_h - u(crate::body::PAD_BOTTOM) };
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
                    let pet_h = crate::cast::PET_MAX_H * u(6);
                    let base_y = if flipy { u(crate::body::PAD_TOP) } else { win_h - pet_h - u(crate::body::PAD_BOTTOM) };
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
