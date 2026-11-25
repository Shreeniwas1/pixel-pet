//! End-to-end checks on the real exe: the pet window must exist, be visible, and stay on top.
//! Each test starts its own pet with a private APPDATA, so the user's settings are untouched,
//! and PIXEL_PET_TEST=1 so the do-not-disturb logic can't depend on what is on screen.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

struct Pet {
    child: Child,
    dir: PathBuf,
}

impl Pet {
    fn start(tag: &str) -> Pet {
        Pet::start_with(tag, "")
    }

    /// Start with these lines already in the settings file.
    fn start_with(tag: &str, cfg: &str) -> Pet {
        let dir = std::env::temp_dir().join(format!("pixel-pet-it-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pixel-pet.cfg"), cfg).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_pixel-pet"))
            .env("APPDATA", &dir)
            .env("PIXEL_PET_TEST", "1")
            .spawn()
            .expect("could not start pixel-pet");
        Pet { child, dir }
    }

    fn cfg(&self) -> String {
        std::fs::read_to_string(self.dir.join("pixel-pet.cfg")).unwrap_or_default()
    }

    /// The pet's main window, once it exists.
    fn window(&self) -> HWND {
        let mut found = None;
        wait_for(10, || {
            found = find_window(self.child.id());
            found.is_some()
        });
        found.expect("the pet never created its window")
    }
}

impl Drop for Pet {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Search {
    pid: u32,
    class: &'static str,
    found: Option<HWND>,
}

unsafe extern "system" fn visit(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Search);
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, &mut pid);
    if pid == s.pid {
        let mut cls = [0u16; 32];
        let n = GetClassNameW(h, cls.as_mut_ptr(), 32).max(0) as usize;
        if String::from_utf16_lossy(&cls[..n]) == s.class {
            s.found = Some(h);
            return 0;
        }
    }
    1
}

fn find_window(pid: u32) -> Option<HWND> {
    find_class(pid, "PixelPetWnd")
}

fn find_class(pid: u32, class: &'static str) -> Option<HWND> {
    let mut s = Search { pid, class, found: None };
    unsafe { EnumWindows(Some(visit), &mut s as *mut _ as LPARAM) };
    s.found
}

fn is_topmost(h: HWND) -> bool {
    unsafe { GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 }
}

fn is_visible(h: HWND) -> bool {
    unsafe { IsWindowVisible(h) != 0 }
}

/// Poll `cond` until it is true or `secs` pass. Returns the last result.
fn wait_for(secs: u64, mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn pet_window_is_visible_and_topmost_at_startup() {
    let pet = Pet::start("startup");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)), "pet window is not visible");
    assert!(is_topmost(w), "pet window lacks WS_EX_TOPMOST");
}

#[test]
fn pet_regains_topmost_after_being_pushed_down() {
    // Other topmost windows (the taskbar, flyouts) can sit above us; the pet must climb back.
    let pet = Pet::start("regain");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)), "pet window is not visible");
    // the pet's own heartbeat may put it back up between the push and the look, so try a few times
    let pushed = (0..5).any(|_| {
        unsafe {
            SetWindowPos(w, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        !is_topmost(w)
    });
    assert!(pushed, "test setup failed: window is still topmost");
    assert!(wait_for(4, || is_topmost(w)), "pet did not return to the topmost band within 4 s");
    assert!(is_visible(w), "pet window disappeared");
}

#[test]
fn pet_does_not_steal_focus() {
    let pet = Pet::start("focus");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1500));
    let fg = unsafe { GetForegroundWindow() };
    assert_ne!(fg, w, "pet window became the foreground window");
}

fn window_size(w: HWND) -> (i32, i32) {
    let mut r = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(w, &mut r) };
    (r.right - r.left, r.bottom - r.top)
}

/// Left-click the pet in the bottom-right corner of its (default-placed) window.
fn click_pet(w: HWND) {
    let (width, height) = window_size(w);
    // default placement puts the pet in the bottom-right corner, 40 px in from the window's edge
    let lp = (((height - 41) as isize) << 16) | ((width - width * 88 / 380) as isize & 0xFFFF);
    unsafe {
        PostMessageW(w, WM_LBUTTONDOWN, 1, lp);
        PostMessageW(w, WM_LBUTTONUP, 0, lp);
    }
}

#[test]
fn left_click_can_open_and_close_the_pixel_hub() {
    let pet = Pet::start_with("hub", "hits=0\n");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1200)); // first paint records where the pet is
    let (_, h0) = window_size(w);
    click_pet(w);
    assert!(wait_for(3, || window_size(w).1 > h0), "left click did not open the hub (window did not grow)");
    assert!(is_topmost(w) && is_visible(w));
    click_pet(w);
    assert!(wait_for(3, || window_size(w).1 == h0), "second click did not close the hub");
}

#[test]
fn right_click_opens_the_same_pixel_hub_not_a_native_menu() {
    let pet = Pet::start("rclick");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1200));
    let (width, height) = window_size(w);
    let lp = (((height - 41) as isize) << 16) | ((width - width * 88 / 380) as isize & 0xFFFF);
    let (_, h0) = window_size(w);
    unsafe { PostMessageW(w, WM_RBUTTONUP, 0, lp) };
    // a native popup menu would block the pet's message loop and never resize the window
    assert!(wait_for(3, || window_size(w).1 > h0), "right click did not open the pixel hub");
}

#[test]
fn stats_window_opens_as_its_own_pixel_window_and_closes() {
    let pet = Pet::start("stats");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    let pid = pet.child.id();
    assert!(find_class(pid, "PixelStatsWnd").is_none(), "stats window should not exist before it is asked for");
    unsafe { PostMessageW(w, WM_HOTKEY, 3, 0) }; // what Ctrl+Alt+S sends
    let mut sw = None;
    assert!(
        wait_for(4, || {
            sw = find_class(pid, "PixelStatsWnd").filter(|h| is_visible(*h));
            sw.is_some()
        }),
        "stats window did not appear"
    );
    let sw = sw.unwrap();
    let style = unsafe { GetWindowLongW(sw, GWL_STYLE) as u32 };
    assert_eq!(style & WS_CAPTION, 0, "stats window must be borderless (pixel-drawn), not a native frame");
    unsafe { PostMessageW(sw, WM_CLOSE, 0, 0) };
    assert!(wait_for(3, || !is_visible(sw)), "stats window did not close");
}

fn hotkey(w: HWND, id: usize) {
    unsafe { PostMessageW(w, WM_HOTKEY, id, 0) };
}

fn passes_clicks(h: HWND) -> bool {
    unsafe { GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT != 0 }
}

#[test]
fn hide_hotkey_hides_the_pet_until_pressed_again() {
    let pet = Pet::start("hide");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    hotkey(w, 5); // Ctrl+Alt+H
    assert!(wait_for(3, || !is_visible(w)), "pet did not hide");
    std::thread::sleep(Duration::from_millis(1500)); // the heartbeat must not bring it back
    assert!(!is_visible(w), "hidden pet came back by itself");
    hotkey(w, 5);
    assert!(wait_for(3, || is_visible(w)), "pet did not come back");
}

#[test]
fn a_hidden_pet_comes_out_for_the_hub_and_goes_away_again() {
    let pet = Pet::start("hidehub");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    hotkey(w, 5);
    assert!(wait_for(3, || !is_visible(w)));
    hotkey(w, 6); // Ctrl+Alt+P
    assert!(wait_for(3, || is_visible(w)), "hub did not open from hiding");
    hotkey(w, 6);
    assert!(wait_for(3, || !is_visible(w)), "pet stayed out after the hub closed");
}

#[test]
fn minimal_mode_is_off_by_default_and_remembered_once_chosen() {
    let pet = Pet::start("minimal");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    hotkey(w, 4); // Ctrl+Alt+M
    assert!(wait_for(3, || pet.cfg().contains("min=1")), "minimal mode was not saved: {}", pet.cfg());
    hotkey(w, 4);
    assert!(wait_for(3, || pet.cfg().contains("min=0")), "leaving minimal mode was not saved");
}

#[test]
fn without_always_on_top_only_the_hub_rises_above_other_windows() {
    let pet = Pet::start_with("ontop", "top=0\n");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    assert!(wait_for(3, || !is_topmost(w)), "pet is topmost although the option is off");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!is_topmost(w), "the heartbeat put it back on top");
    hotkey(w, 6);
    assert!(wait_for(3, || is_topmost(w)), "the hub must be readable when asked for");
    hotkey(w, 6);
    assert!(wait_for(3, || !is_topmost(w)), "still topmost after the hub closed");
}

#[test]
fn click_through_is_lifted_while_the_hub_is_open() {
    let pet = Pet::start_with("through", "through=1\n");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    assert!(wait_for(3, || passes_clicks(w)), "clicks do not pass through");
    hotkey(w, 6);
    assert!(wait_for(3, || !passes_clicks(w)), "the open hub cannot be clicked");
    hotkey(w, 6);
    assert!(wait_for(3, || passes_clicks(w)), "click-through did not come back");
}

#[test]
fn a_due_reminder_brings_a_hidden_click_through_pet_out() {
    // an overdue task fires on the first heartbeat
    let dir = std::env::temp_dir().join(format!("pixel-pet-it-remind-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("pixel-pet-todos.txt"), "0|1000|Stretch
").unwrap();
    let pet = Pet::start_with("remind", "through=1
min=1
top=0
");
    let w = pet.window();
    hotkey(w, 5);
    assert!(wait_for(5, || is_visible(w) && is_topmost(w) && !passes_clicks(w)), "the reminder is not reachable");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(is_visible(w), "the reminder vanished before it was answered");
}

fn window_left(w: HWND) -> i32 {
    let mut r = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(w, &mut r) };
    r.left
}

fn setting(pet: &Pet, key: &str) -> f32 {
    let cfg = pet.cfg();
    let line = cfg.lines().find_map(|l| l.strip_prefix(key).and_then(|l| l.strip_prefix('=')).map(str::to_string));
    line.and_then(|v| v.parse().ok()).unwrap_or(f32::NAN)
}

/// A pet that does nothing unless told to.
const CALM: &str = "walk=0\ntricks=0\nfacts=0\nfood=20\n";

#[test]
fn left_click_hits_the_pet_instead_of_opening_the_hub() {
    let pet = Pet::start_with("hit", CALM);
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1200));
    let (_, h0) = window_size(w);
    for _ in 0..9 {
        click_pet(w); // enough to knock it flying
        std::thread::sleep(Duration::from_millis(80));
    }
    std::thread::sleep(Duration::from_millis(2500));
    assert!(is_visible(w), "the pet did not survive a beating");
    assert_eq!(window_size(w).1, h0, "a hit must not open the hub");
    unsafe { PostMessageW(w, WM_CLOSE, 0, 0) };
    assert!(wait_for(3, || setting(&pet, "happy") < 69.0), "a hit should cost some happiness: {}", pet.cfg());
}

#[test]
fn the_feed_hotkey_makes_the_pet_fetch_and_eat() {
    let pet = Pet::start_with("feed", CALM);
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(800));
    let x0 = window_left(w);
    hotkey(w, 7); // Ctrl+Alt+E
    assert!(wait_for(5, || (window_left(w) - x0).abs() > 10), "the pet did not go and get its food");
    std::thread::sleep(Duration::from_millis(3500));
    hotkey(w, 4); // anything that saves the settings
    assert!(wait_for(3, || setting(&pet, "food") > 30.0), "eating did not fill it up: {}", pet.cfg());
}

#[test]
fn a_pet_left_to_itself_walks_about_and_stays_on_the_screen() {
    let pet = Pet::start_with("walk", "tricks=0\nfacts=0\nwalk=1\n");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    let x0 = window_left(w);
    assert!(wait_for(40, || (window_left(w) - x0).abs() > 20), "the pet never walked anywhere");
}

#[test]
fn every_character_runs_and_shows_itself() {
    for i in 0..16 {
        let pet = Pet::start_with(&format!("cast{i}"), &format!("pet={i}\nfacts=0\n"));
        let w = pet.window();
        assert!(wait_for(5, || is_visible(w)), "character {i} did not appear");
        hotkey(w, 7);
        std::thread::sleep(Duration::from_millis(700));
        assert!(is_visible(w), "character {i} crashed");
    }
}

fn window_bottom(w: HWND) -> i32 {
    let mut r = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(w, &mut r) };
    r.bottom
}

#[test]
fn a_pet_left_in_mid_air_comes_down_unless_gravity_is_off() {
    let floating = Pet::start_with("float", "walk=0
tricks=0
facts=0
gravity=0
x=400
y=100
");
    let w = floating.window();
    assert!(wait_for(5, || is_visible(w)));
    let y0 = window_bottom(w);
    let falling = Pet::start_with("fall", "walk=0
tricks=0
facts=0
x=700
y=100
");
    let v = falling.window();
    assert!(wait_for(5, || is_visible(v)));
    assert!(wait_for(6, || window_bottom(v) > y0 + 100), "it hangs in mid-air");
    assert_eq!(window_bottom(w), y0, "without gravity it must stay where it was put");
}
