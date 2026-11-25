//! What is playing, and its play, pause and skip buttons, as Windows' own media controls know it
//! (the same ones the volume flyout and media keys use).

use crate::prelude::*;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32};
use windows_sys::core::GUID;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type HString = *mut c_void;

/// The few WinRT functions needed, looked up at run time.
struct Rt {
    init: unsafe extern "system" fn(i32) -> i32,
    factory: unsafe extern "system" fn(HString, *const GUID, *mut *mut c_void) -> i32,
    make: unsafe extern "system" fn(*const u16, u32, *mut HString) -> i32,
    free: unsafe extern "system" fn(HString) -> i32,
    raw: unsafe extern "system" fn(HString, *mut u32) -> *const u16,
}

fn rt() -> Option<&'static Rt> {
    static RT: std::sync::OnceLock<Option<Rt>> = std::sync::OnceLock::new();
    RT.get_or_init(|| unsafe {
        let lib = LoadLibraryW(wide("combase.dll").as_ptr());
        if lib.is_null() {
            return None;
        }
        let f = |name: &[u8]| GetProcAddress(lib, name.as_ptr());
        Some(Rt {
            init: std::mem::transmute(f(b"RoInitialize\0")?),
            factory: std::mem::transmute(f(b"RoGetActivationFactory\0")?),
            make: std::mem::transmute(f(b"WindowsCreateString\0")?),
            free: std::mem::transmute(f(b"WindowsDeleteString\0")?),
            raw: std::mem::transmute(f(b"WindowsGetStringRawBuffer\0")?),
        })
    })
    .as_ref()
}

#[derive(Clone, Default, PartialEq, Debug)]
pub(crate) struct Now {
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

pub(crate) static NOW: Mutex<Option<Now>> = Mutex::new(None);
/// What is playing calls itself a video: nothing to dance to.
pub(crate) static VIDEO: AtomicBool = AtomicBool::new(false);
/// Somebody wants to know.
pub(crate) static WANTED: AtomicBool = AtomicBool::new(false);

pub(crate) const TOGGLE: u32 = 1;
pub(crate) const NEXT: u32 = 2;
pub(crate) const BACK: u32 = 3;
static COMMAND: AtomicU32 = AtomicU32::new(0);

pub(crate) fn command(c: u32) {
    COMMAND.store(c, Relaxed);
}

/// One line for a small space.
pub(crate) fn line(n: &Now) -> String {
    match (n.title.trim(), n.artist.trim()) {
        ("", a) => a.to_string(),
        (t, "") => t.to_string(),
        (t, a) => format!("{t} - {a}"),
    }
}

// ---- a WinRT object; calls go through its table of functions ----

struct Obj(*mut c_void);

impl Drop for Obj {
    fn drop(&mut self) {
        unsafe {
            let f: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(self.f(2));
            f(self.0);
        }
    }
}

impl Obj {
    /// What a call that worked gave. Windows may well say "fine" and give nothing, and then there must
    /// never be an object, not even for a moment: dropping it calls through the pointer.
    fn found(hr: i32, out: *mut c_void) -> Option<Obj> {
        (hr >= 0 && !out.is_null()).then(|| Obj(out))
    }

    unsafe fn f(&self, n: usize) -> *const c_void {
        *(*(self.0 as *const *const *const c_void)).add(n)
    }

    /// A method whose only argument is where to put another object.
    unsafe fn get(&self, n: usize) -> Option<Obj> {
        let f: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 = std::mem::transmute(self.f(n));
        let mut out = null_mut();
        Obj::found(f(self.0, &mut out), out)
    }

    unsafe fn query(&self, iid: &GUID) -> Option<Obj> {
        let f: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32 = std::mem::transmute(self.f(0));
        let mut out = null_mut();
        Obj::found(f(self.0, iid, &mut out), out)
    }

    unsafe fn num(&self, n: usize) -> Option<i32> {
        let f: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32 = std::mem::transmute(self.f(n));
        let mut out = 0;
        (f(self.0, &mut out) >= 0).then_some(out)
    }

    unsafe fn text(&self, n: usize) -> String {
        let Some(rt) = rt() else { return String::new() };
        let f: unsafe extern "system" fn(*mut c_void, *mut HString) -> i32 = std::mem::transmute(self.f(n));
        let mut h: HString = null_mut();
        if f(self.0, &mut h) < 0 {
            return String::new();
        }
        let mut len = 0u32;
        let p = (rt.raw)(h, &mut len);
        let s = if p.is_null() { String::new() } else { String::from_utf16_lossy(std::slice::from_raw_parts(p, len as usize)) };
        (rt.free)(h);
        s
    }

    /// Wait for an operation started by method `n` and take its result.
    unsafe fn wait(&self, n: usize) -> Option<Obj> {
        const INFO: GUID = GUID::from_u128(0x00000036_0000_0000_C000_000000000046);
        let op = self.get(n)?;
        let info = op.query(&INFO)?;
        for _ in 0..150 {
            match info.num(7)? {
                0 => std::thread::sleep(Duration::from_millis(20)), // still going
                1 => return op.get(8),
                _ => return None,
            }
        }
        None
    }
}

unsafe fn manager() -> Option<Obj> {
    const STATICS: GUID = GUID::from_u128(0x2050C4EE_11A0_57DE_AED7_C97C70338245);
    let name: Vec<u16> = "Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager".encode_utf16().collect();
    let rt = rt()?;
    let mut h: HString = null_mut();
    if (rt.make)(name.as_ptr(), name.len() as u32, &mut h) < 0 {
        return None;
    }
    let mut raw = null_mut();
    let ok = (rt.factory)(h, &STATICS, &mut raw) >= 0 && !raw.is_null();
    (rt.free)(h);
    if !ok {
        return None;
    }
    Obj(raw).wait(6) // RequestAsync
}

/// The session in charge right now, its state, and whether it is a video.
unsafe fn look(m: &Obj) -> Option<(Obj, Now, bool)> {
    let session = m.get(6)?; // GetCurrentSession
    let info = session.get(9)?; // GetPlaybackInfo
    let playing = info.num(7) == Some(4); // PlaybackStatus: playing
    let video = info.get(8).and_then(|k| k.num(6)) == Some(2); // PlaybackType: video
    let props = session.wait(7)?; // TryGetMediaPropertiesAsync
    let now = Now { title: props.text(6), artist: props.text(9), playing };
    Some((session, now, video))
}

pub(crate) fn spawn() {
    if let Some(how) = std::env::var_os("PIXEL_PET_TEST") {
        if how == "music" {
            *NOW.lock().unwrap() = Some(Now { title: "A Song".into(), artist: "The Pets".into(), playing: true });
        }
        if how != "audio" {
            return;
        }
    }
    std::thread::spawn(|| unsafe {
        let Some(rt) = rt() else { return };
        (rt.init)(1); // multithreaded
        let mut m: Option<Obj> = None;
        loop {
            if !WANTED.load(Relaxed) {
                *NOW.lock().unwrap() = None;
                VIDEO.store(false, Relaxed);
                std::thread::sleep(Duration::from_millis(800));
                continue;
            }
            if m.is_none() {
                m = manager();
            }
            let seen = m.as_ref().and_then(|m| look(m));
            if let Some((session, ..)) = &seen {
                // TryTogglePlayPauseAsync, TrySkipNextAsync, TrySkipPreviousAsync
                let n = match COMMAND.swap(0, Relaxed) {
                    TOGGLE => 20,
                    NEXT => 16,
                    BACK => 17,
                    _ => 0,
                };
                if n > 0 {
                    drop(session.get(n));
                }
            }
            let (now, video) = seen.map_or((None, false), |(_, n, v)| (Some(n), v));
            VIDEO.store(video && now.as_ref().is_some_and(|n| n.playing), Relaxed);
            *NOW.lock().unwrap() = now.filter(|n| !line(n).is_empty());
            // buttons should answer at once, the rest can wait
            for _ in 0..12 {
                if COMMAND.load(Relaxed) != 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_giving_nothing_is_not_an_object() {
        // these used to crash: the object was made first, and dropped when it was not wanted
        assert!(Obj::found(0, null_mut()).is_none());
        assert!(Obj::found(-1, null_mut()).is_none());
        assert!(Obj::found(-1, 8 as *mut c_void).is_none());
    }

    #[test]
    fn the_line_shows_what_there_is() {
        let n = |t: &str, a: &str| line(&Now { title: t.into(), artist: a.into(), playing: true });
        assert_eq!(n("Song", "Band"), "Song - Band");
        assert_eq!(n("Song", " "), "Song");
        assert_eq!(n("", "Band"), "Band");
        assert_eq!(n("", ""), "");
    }

    /// Not a check: prints what Windows says is playing.
    #[test]
    #[ignore]
    fn what_is_playing() {
        unsafe {
            (rt().unwrap().init)(1);
            let m = manager();
            println!("manager: {}", m.is_some());
            println!("{:?}", m.as_ref().and_then(|m| look(m)).map(|(_, n, v)| (n.title.chars().count(), n.artist.chars().count(), n.playing, v)));
        }
    }
}
