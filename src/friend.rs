//! A second pet. It is the same program running a second time with everything but the pet switched
//! off; the two find each other's windows and tell each other where they stand.

use crate::prelude::*;
use std::sync::OnceLock;

/// Sent to the friend: the settings changed.
pub(crate) const WM_RELOAD: u32 = WM_APP + 3;
/// Sent to the other pet: let's do something. wParam = what, lParam = where the sender stands.
pub(crate) const WM_PLAY: u32 = WM_APP + 4;

/// Is this the second pet? Holds the process id of the first.
pub(crate) fn of() -> Option<u32> {
    static OF: OnceLock<Option<u32>> = OnceLock::new();
    *OF.get_or_init(|| std::env::args().find_map(|a| a.strip_prefix("--friend=").and_then(|p| p.parse().ok())))
}

pub(crate) fn is_friend() -> bool {
    of().is_some()
}

// ---- rules (pure, so they can be tested) ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Play {
    Greet,
    Dance,
    Jump,
    Chase, // the sender comes running, the other runs away
    Bump,  // the sender flew into the other
}

const PLAYS: [Play; 5] = [Play::Greet, Play::Dance, Play::Jump, Play::Chase, Play::Bump];

pub(crate) fn code(p: Play) -> usize {
    PLAYS.iter().position(|q| *q == p).unwrap_or(0)
}

pub(crate) fn decode(c: usize) -> Option<Play> {
    PLAYS.get(c).copied()
}

/// What to do when they meet, for a roll of 0..1.
pub(crate) fn meeting(r: f32) -> Play {
    if r < 0.4 {
        Play::Greet
    } else if r < 0.75 {
        Play::Dance
    } else {
        Play::Jump
    }
}

/// Both do the same when they meet.
pub(crate) fn clip_of(p: Play) -> Option<Clip> {
    match p {
        Play::Greet => Some(Clip::Wave),
        Play::Dance => Some(Clip::Groove),
        Play::Jump => Some(Clip::Hop),
        _ => None,
    }
}

/// The settings of the second pet: those of the first, minus everything that only one should do.
pub(crate) fn trim(s: &mut State) {
    s.pet = pack::still(s.friend.saturating_sub(1));
    s.friend = 0;
    (s.focus, s.tokens_on, s.facts_on, s.care_on, s.sys_on, s.talk_on, s.buddy_on, s.react_on) = (false, false, false, false, false, false, false, false);
    (s.show_speed, s.show_up, s.show_pc, s.show_now) = (false, false, false, false);
    s.intro = intro::DONE;
    s.hat = 0;
    s.todos.clear();
    s.launch.clear();
}

// ---- finding each other ----

struct Search {
    title: String,
    found: HWND,
}

unsafe extern "system" fn visit(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Search);
    let mut cls = [0u16; 16];
    let n = GetClassNameW(h, cls.as_mut_ptr(), 16).max(0) as usize;
    if String::from_utf16_lossy(&cls[..n]) != "PixelPetWnd" {
        return 1;
    }
    if s.title.is_empty() {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        if Some(pid) == of() {
            s.found = h;
            return 0;
        }
    } else {
        let mut t = [0u16; 64];
        let n = GetWindowTextW(h, t.as_mut_ptr(), 64).max(0) as usize;
        if String::from_utf16_lossy(&t[..n]) == s.title {
            s.found = h;
            return 0;
        }
    }
    1
}

/// The window title of the friend of process `pid`.
pub(crate) fn title(pid: u32) -> String {
    format!("Pixel Pet Friend {pid}")
}

/// There is (or should be) a second pet.
static TWO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The other pet's window, looked up at most once a second.
pub(crate) unsafe fn other() -> Option<HWND> {
    if !is_friend() && !TWO.load(Relaxed) {
        return None; // the usual case costs nothing
    }
    static KNOWN: Mutex<(isize, u32)> = Mutex::new((0, 0));
    let mut k = KNOWN.lock().unwrap();
    if k.0 != 0 && IsWindow(k.0 as HWND) != 0 {
        return Some(k.0 as HWND);
    }
    let now = GetTickCount();
    if k.1 != 0 && now.wrapping_sub(k.1) < 1000 {
        return None;
    }
    let mut s = Search { title: if is_friend() { String::new() } else { title(std::process::id()) }, found: null_mut() };
    EnumWindows(Some(visit), &mut s as *mut _ as LPARAM);
    *k = (s.found as isize, now.max(1));
    (!s.found.is_null()).then_some(s.found)
}

const SHIFT: isize = 1 << 20; // a property of 0 would mean "none"

/// Tell whoever asks where this pet stands.
pub(crate) unsafe fn publish(hwnd: HWND, feet: (f32, f32)) {
    if !is_friend() && !TWO.load(Relaxed) {
        return;
    }
    SetPropW(hwnd, wide("PixelPetX").as_ptr(), (feet.0 as isize + SHIFT) as HANDLE);
    SetPropW(hwnd, wide("PixelPetY").as_ptr(), (feet.1 as isize + SHIFT) as HANDLE);
}

/// Where the other one stands, if it can be seen.
pub(crate) unsafe fn other_feet() -> Option<(f32, f32)> {
    let w = other()?;
    let (x, y) = (GetPropW(w, wide("PixelPetX").as_ptr()) as isize, GetPropW(w, wide("PixelPetY").as_ptr()) as isize);
    (IsWindowVisible(w) != 0 && x != 0 && y != 0).then_some(((x - SHIFT) as f32, (y - SHIFT) as f32))
}

pub(crate) unsafe fn tell(p: Play, x: f32) {
    if let Some(w) = other() {
        PostMessageW(w, WM_PLAY, code(p), x as LPARAM);
    }
}

// ---- looking after the second process ----

/// Twice a second, in both: the first starts or ends the second; the second follows the first.
pub(crate) unsafe fn tend(hwnd: HWND) {
    static ALONE: AtomicI32 = AtomicI32::new(0);
    let wanted = ST.lock().unwrap().friend > 0;
    if is_friend() {
        match other() {
            Some(w) => {
                ALONE.store(0, Relaxed);
                // when the first one is put away, so is the second
                ST.lock().unwrap().hidden = IsWindowVisible(w) == 0;
            }
            None if ALONE.fetch_add(1, Relaxed) > 5 => {
                DestroyWindow(hwnd);
            }
            None => {}
        }
        return;
    }
    match (wanted, other()) {
        // give a new one time to appear before trying again
        (true, None) if !TWO.swap(true, Relaxed) => {} // look for one first
        (true, None) if ALONE.fetch_add(1, Relaxed) % 12 == 0 => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).arg(format!("--friend={}", std::process::id())).spawn();
            }
        }
        (true, Some(_)) => ALONE.store(0, Relaxed),
        (false, Some(w)) => {
            PostMessageW(w, WM_CLOSE, 0, 0);
            ALONE.store(0, Relaxed);
        }
        (false, None) => TWO.store(false, Relaxed),
        _ => {}
    }
}

/// The settings changed: the second pet should look again.
pub(crate) unsafe fn reload() {
    if !is_friend() {
        if let Some(w) = other() {
            PostMessageW(w, WM_RELOAD, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_game_survives_the_trip_to_the_other_pet() {
        for p in PLAYS {
            assert_eq!(decode(code(p)), Some(p));
        }
        assert_eq!(decode(99), None);
    }

    #[test]
    fn meetings_are_friendly_and_end_by_themselves() {
        for i in 0..100 {
            let c = clip_of(meeting(i as f32 / 100.0)).expect("they do something together");
            assert!(c.secs().is_some());
        }
    }
}
