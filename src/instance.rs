//! One pet at a time, and the key that opens and closes it. Windows itself listens for the key
//! (through a Start menu shortcut), because a closed pet cannot.

use crate::prelude::*;
use windows_sys::core::GUID;
use windows_sys::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_CREATE, SHCNE_DELETE, SHCNF_PATHW};

/// Sent to a running pet when it is started again: come out.
pub(crate) const WM_COME_OUT: u32 = WM_APP + 2;

/// Marks the windows of pets that share our settings; others are none of our business.
fn mark() -> Vec<u16> {
    let home = std::env::var_os("APPDATA").unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in home.to_string_lossy().to_lowercase().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    wide(&format!("PixelPet-{h:x}"))
}

pub(crate) unsafe fn claim(hwnd: HWND) {
    SetPropW(hwnd, mark().as_ptr(), 1 as HANDLE);
}

struct Search {
    mark: Vec<u16>,
    found: HWND,
}

unsafe extern "system" fn visit(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Search);
    if GetPropW(h, s.mark.as_ptr()).is_null() {
        return 1;
    }
    s.found = h;
    0
}

/// What to do at startup: go ahead (true), or leave because there already is a pet (false).
pub(crate) unsafe fn may_start() -> bool {
    let toggle = std::env::args().any(|a| a == "--toggle");
    let mut s = Search { mark: mark(), found: null_mut() };
    EnumWindows(Some(visit), &mut s as *mut _ as LPARAM);
    if s.found.is_null() {
        return true;
    }
    PostMessageW(s.found, if toggle { WM_CLOSE } else { WM_COME_OUT }, 0, 0);
    false
}

/// Post something to the pet that runs.
pub(crate) unsafe fn tell(msg: u32, wp: usize) {
    let mut s = Search { mark: mark(), found: null_mut() };
    EnumWindows(Some(visit), &mut s as *mut _ as LPARAM);
    if !s.found.is_null() {
        PostMessageW(s.found, msg, wp, 0);
    }
}

// ---- the shortcut ----

fn link_path() -> Option<std::path::PathBuf> {
    data_path("Microsoft\\Windows\\Start Menu\\Programs\\Pixel Pet.lnk")
}

pub(crate) fn has_shortcut() -> bool {
    link_path().is_some_and(|p| p.exists())
}

unsafe fn slot<F: Copy>(object: *mut std::ffi::c_void, n: usize) -> F {
    let f = *(*(object as *const *const *const std::ffi::c_void)).add(n);
    std::mem::transmute_copy::<*const std::ffi::c_void, F>(&f)
}

type Text = unsafe extern "system" fn(*mut std::ffi::c_void, *const u16) -> i32;

/// Write a shortcut to this exe with Ctrl+Alt+Q as its key.
unsafe fn write_link(to: &std::path::Path) -> bool {
    const LINK: GUID = GUID::from_u128(0x00021401_0000_0000_C000_000000000046);
    const I_LINK: GUID = GUID::from_u128(0x000214F9_0000_0000_C000_000000000046);
    const I_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_C000_000000000046);
    let Ok(exe) = std::env::current_exe() else { return false };
    CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
    let mut link = null_mut();
    if CoCreateInstance(&LINK, null_mut(), CLSCTX_INPROC_SERVER, &I_LINK, &mut link) < 0 || link.is_null() {
        return false;
    }
    slot::<Text>(link, 20)(link, wide(&exe.to_string_lossy()).as_ptr()); // SetPath
    slot::<Text>(link, 11)(link, wide("--toggle").as_ptr()); // SetArguments
    slot::<Text>(link, 7)(link, wide("Open or close Pixel Pet").as_ptr()); // SetDescription
    if let Some(dir) = exe.parent() {
        slot::<Text>(link, 9)(link, wide(&dir.to_string_lossy()).as_ptr()); // SetWorkingDirectory
    }
    slot::<unsafe extern "system" fn(*mut std::ffi::c_void, u16) -> i32>(link, 13)(link, 0x0600 | b'Q' as u16); // SetHotkey
    let mut file = null_mut();
    let query: unsafe extern "system" fn(*mut std::ffi::c_void, *const GUID, *mut *mut std::ffi::c_void) -> i32 = slot(link, 0);
    let mut ok = query(link, &I_FILE, &mut file) >= 0 && !file.is_null();
    type Release = unsafe extern "system" fn(*mut std::ffi::c_void) -> u32;
    if ok {
        let save: unsafe extern "system" fn(*mut std::ffi::c_void, *const u16, BOOL) -> i32 = slot(file, 6);
        ok = save(file, wide(&to.to_string_lossy()).as_ptr(), 1) >= 0;
        slot::<Release>(file, 2)(file);
    }
    slot::<Release>(link, 2)(link);
    ok
}

/// Add or remove the shortcut. Returns whether there is one now.
pub(crate) unsafe fn set_shortcut(on: bool) -> bool {
    let Some(p) = link_path() else { return false };
    if on {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        write_link(&p);
    } else {
        let _ = std::fs::remove_file(&p);
    }
    // tell Explorer, so the key works (or stops working) straight away
    let w = wide(&p.to_string_lossy());
    SHChangeNotify(if on { SHCNE_CREATE } else { SHCNE_DELETE } as i32, SHCNF_PATHW, w.as_ptr() as *const _, null());
    has_shortcut()
}

/// Keep an existing shortcut pointing at this exe if it moved.
pub(crate) unsafe fn refresh() {
    if has_shortcut() {
        if let Some(p) = link_path() {
            write_link(&p);
        }
    }
}
