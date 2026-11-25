//! Hears when music is playing, so the pet can dance to it. It only reads Windows' own loudness
//! meters (the bars in the volume mixer); it never records or sees the sound itself.

use crate::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

/// Is music playing right now?
pub(crate) static PLAYING: AtomicBool = AtomicBool::new(false);
/// Counts beats, so whoever looks can tell a new one.
pub(crate) static BEATS: AtomicU32 = AtomicU32::new(0);
/// The listener only works while somebody cares.
pub(crate) static WANTED: AtomicBool = AtomicBool::new(false);

// ---- rules (pure, so they can be tested) ----

/// Decides from loudness readings whether this is music, and where the beats are. Music is loud
/// without a pause for seconds; speech and notification sounds are full of gaps.
pub(crate) struct Ear {
    heard: f32, // how much of the last seconds was audible, 0..1
    slow: f32,  // average loudness
    last: f32,
    gap: f32, // seconds since the last beat
    pub on: bool,
}

impl Ear {
    pub const fn new() -> Ear {
        Ear { heard: 0.0, slow: 0.0, last: 0.0, gap: 0.0, on: false }
    }

    /// One reading (0..1). Returns true on a beat.
    pub fn hear(&mut self, level: f32, dt: f32) -> bool {
        let k = (dt / 2.2).min(1.0);
        self.heard += ((level > 0.015) as u32 as f32 - self.heard) * k;
        if !self.on && self.heard > 0.86 {
            self.on = true;
        } else if self.on && self.heard < 0.35 {
            self.on = false;
        }
        self.slow += (level - self.slow) * (dt / 0.7).min(1.0);
        self.gap += dt;
        let beat = self.on && self.gap > 0.28 && level > self.slow * 1.12 && level > self.last;
        self.last = level;
        // a steady drone has no beats of its own, so it gets a slow pulse
        let pulse = self.on && self.gap > 0.75;
        if beat || pulse {
            self.gap = 0.0;
        }
        beat || pulse
    }
}

/// Programs whose sound is not music to dance to: calls, recorders.
pub(crate) fn not_music(exe: &str) -> bool {
    const TALK: &[&str] = &["teams", "ms-teams", "zoom", "discord", "skype", "slack", "webex", "whatsapp", "telegram", "signal", "obs64", "lync", "pixel-pet"];
    let name = exe.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    TALK.iter().any(|t| name == *t || name.starts_with(&format!("{t}."))) || name.starts_with("zoom")
}

// ---- the meters ----

/// A COM object; calls go through its table of functions.
struct Com(*mut std::ffi::c_void);

impl Com {
    unsafe fn f(&self, n: usize) -> *const std::ffi::c_void {
        *(*(self.0 as *const *const *const std::ffi::c_void)).add(n)
    }

    unsafe fn query(&self, iid: &GUID) -> Option<Com> {
        let f: unsafe extern "system" fn(*mut std::ffi::c_void, *const GUID, *mut *mut std::ffi::c_void) -> i32 = std::mem::transmute(self.f(0));
        let mut out = null_mut();
        (f(self.0, iid, &mut out) >= 0 && !out.is_null()).then_some(Com(out))
    }

    /// A method whose only argument is where to put another object.
    unsafe fn get(&self, n: usize) -> Option<Com> {
        let f: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32 = std::mem::transmute(self.f(n));
        let mut out = null_mut();
        (f(self.0, &mut out) >= 0 && !out.is_null()).then_some(Com(out))
    }

    /// A method whose only argument is where to put a 32-bit number.
    unsafe fn num(&self, n: usize) -> Option<u32> {
        let f: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32 = std::mem::transmute(self.f(n));
        let mut out = 0u32;
        (f(self.0, &mut out) >= 0).then_some(out)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            let f: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32 = std::mem::transmute(self.f(2));
            f(self.0);
        }
    }
}

const ENUMERATOR: GUID = GUID::from_u128(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
const I_ENUMERATOR: GUID = GUID::from_u128(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
const I_SESSIONS: GUID = GUID::from_u128(0x77AA99A0_1BD6_484F_8BC7_2C654C9A9B6F);
const I_SESSION2: GUID = GUID::from_u128(0xBFB7FF88_7239_4FC9_8FA2_07C950BE9C6D);
const I_METER: GUID = GUID::from_u128(0xC02216F6_8C67_4B5B_9D00_D008E73E0064);

/// The loudness meters of every program that is playing something worth dancing to.
unsafe fn meters() -> Vec<Com> {
    let mut out = Vec::new();
    let mut raw = null_mut();
    if CoCreateInstance(&ENUMERATOR, null_mut(), CLSCTX_ALL, &I_ENUMERATOR, &mut raw) < 0 || raw.is_null() {
        return out;
    }
    let all = Com(raw);
    // GetDefaultAudioEndpoint(render, multimedia)
    let default: unsafe extern "system" fn(*mut std::ffi::c_void, i32, i32, *mut *mut std::ffi::c_void) -> i32 = std::mem::transmute(all.f(4));
    let mut raw = null_mut();
    if default(all.0, 0, 1, &mut raw) < 0 || raw.is_null() {
        return out;
    }
    let device = Com(raw);
    let activate: unsafe extern "system" fn(*mut std::ffi::c_void, *const GUID, u32, *const std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32 = std::mem::transmute(device.f(3));
    let mut raw = null_mut();
    if activate(device.0, &I_SESSIONS, CLSCTX_ALL, null(), &mut raw) < 0 || raw.is_null() {
        return out;
    }
    let manager = Com(raw);
    let Some(list) = manager.get(5) else { return out }; // GetSessionEnumerator
    let count = list.num(3).unwrap_or(0).min(64);
    let session: unsafe extern "system" fn(*mut std::ffi::c_void, i32, *mut *mut std::ffi::c_void) -> i32 = std::mem::transmute(list.f(4));
    for i in 0..count {
        let mut raw = null_mut();
        if session(list.0, i as i32, &mut raw) < 0 || raw.is_null() {
            continue;
        }
        let one = Com(raw);
        if one.num(3) != Some(1) {
            continue; // not playing
        }
        let Some(two) = one.query(&I_SESSION2) else { continue };
        let system: unsafe extern "system" fn(*mut std::ffi::c_void) -> i32 = std::mem::transmute(two.f(15)); // IsSystemSoundsSession
        if system(two.0) == 0 {
            continue;
        }
        let path = two.num(14).map(|pid| process_path(pid)).unwrap_or_default(); // GetProcessId
        let exe = path.rsplit('\\').next().unwrap_or("");
        if not_music(exe) || rules::is_game(&path) {
            continue;
        }
        if let Some(m) = one.query(&I_METER) {
            out.push(m);
        }
    }
    out
}

unsafe fn loudest(meters: &[Com]) -> f32 {
    let mut top = 0.0f32;
    for m in meters {
        if let Some(bits) = m.num(3) {
            let v = f32::from_bits(bits);
            if v.is_finite() {
                top = top.max(v);
            }
        }
    }
    top
}

/// Start listening in the background.
pub(crate) fn spawn() {
    if let Some(how) = std::env::var_os("PIXEL_PET_TEST") {
        if how == "music" {
            PLAYING.store(true, Relaxed); // pretend, so the dancing can be tested in silence
            std::thread::spawn(|| loop {
                BEATS.fetch_add(1, Relaxed);
                std::thread::sleep(Duration::from_millis(500));
            });
        }
        if how != "audio" {
            return;
        }
    }
    std::thread::spawn(|| unsafe {
        CoInitializeEx(null(), COINIT_MULTITHREADED as u32);
        let mut ear = Ear::new();
        let mut found: Vec<Com> = Vec::new();
        let mut scanned: Option<Instant> = None;
        loop {
            if !WANTED.load(Relaxed) {
                found.clear();
                scanned = None;
                ear = Ear::new();
                PLAYING.store(false, Relaxed);
                std::thread::sleep(Duration::from_millis(1000));
                continue;
            }
            if scanned.map_or(true, |t| t.elapsed() > Duration::from_millis(1500)) {
                found = meters();
                scanned = Some(Instant::now());
            }
            // nothing is playing: look again in a while. Something is: follow it closely.
            let pause = if found.is_empty() && !ear.on { 1.0 } else { 0.04 };
            if ear.hear(loudest(&found), pause) {
                BEATS.fetch_add(1, Relaxed);
            }
            PLAYING.store(ear.on, Relaxed);
            std::thread::sleep(Duration::from_secs_f32(pause));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.04;

    fn play(ear: &mut Ear, secs: f32, level: impl Fn(f32) -> f32) -> u32 {
        let mut beats = 0;
        for i in 0..(secs / DT) as u32 {
            beats += ear.hear(level(i as f32 * DT), DT) as u32;
        }
        beats
    }

    /// 120 beats a minute: a thump that fades, over a steady bed.
    fn song(t: f32) -> f32 {
        0.12 + 0.5 * (-((t % 0.5) * 8.0)).exp()
    }

    #[test]
    fn music_is_noticed_after_a_few_seconds_and_forgotten_after_it_stops() {
        let mut e = Ear::new();
        play(&mut e, 1.5, song);
        assert!(!e.on, "too early to tell");
        play(&mut e, 5.0, song);
        assert!(e.on);
        play(&mut e, 0.8, |_| 0.0);
        assert!(e.on, "a short break between songs");
        play(&mut e, 4.0, |_| 0.0);
        assert!(!e.on);
    }

    #[test]
    fn it_follows_the_beat() {
        let mut e = Ear::new();
        play(&mut e, 8.0, song);
        let beats = play(&mut e, 10.0, song);
        assert!((17..=24).contains(&beats), "heard {beats} beats in 10 s at 120 a minute");
    }

    #[test]
    fn notification_sounds_and_speech_are_not_music() {
        let mut e = Ear::new();
        play(&mut e, 1.2, |_| 0.6);
        assert!(!e.on);
        let mut e = Ear::new();
        play(&mut e, 40.0, |t| if t % 1.0 < 0.65 { 0.4 } else { 0.0 });
        assert!(!e.on, "speech has pauses");
    }

    #[test]
    fn calls_are_not_music_but_players_and_browsers_are() {
        for e in ["Teams.exe", "ms-teams.exe", "Zoom.exe", "ZoomIt.exe", "Discord.exe"] {
            assert!(not_music(e), "{e}");
        }
        for e in ["Spotify.exe", "chrome.exe", "vlc.exe", "msedge.exe", ""] {
            assert!(!not_music(e), "{e}");
        }
    }
}
