//! "Start with Windows": a per-user startup entry (no admin rights, easy to see and remove in
//! Task Manager > Startup apps). Off unless the user turns it on.

use crate::prelude::*;
use windows_sys::Win32::System::Registry::*;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
pub(crate) const VALUE: &str = "PixelPet";

unsafe fn open(write: bool) -> Option<HKEY> {
    open_at(RUN_KEY, write)
}

/// A new account has no such key until something wants to start with Windows.
unsafe fn open_at(path: &str, write: bool) -> Option<HKEY> {
    let mut k: HKEY = null_mut();
    let path = wide(path);
    let r = if write { RegCreateKeyW(HKEY_CURRENT_USER, path.as_ptr(), &mut k) } else { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut k) };
    (r == 0).then_some(k)
}

/// The command stored under `name`, if there is one.
pub(crate) unsafe fn command(name: &str) -> Option<String> {
    let k = open(false)?;
    let mut size = 0u32;
    let n = wide(name);
    let found = RegQueryValueExW(k, n.as_ptr(), null(), null_mut(), null_mut(), &mut size) == 0 && size >= 2;
    let out = if found {
        let mut buf = vec![0u16; size as usize / 2 + 1];
        let mut sz = size;
        if RegQueryValueExW(k, n.as_ptr(), null(), null_mut(), buf.as_mut_ptr() as *mut u8, &mut sz) == 0 {
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..len]))
        } else {
            None
        }
    } else {
        None
    };
    RegCloseKey(k);
    out
}

/// Add (Some) or remove (None) the startup command stored under `name`.
pub(crate) unsafe fn write(name: &str, cmd: Option<&str>) -> bool {
    let Some(k) = open(true) else { return false };
    let n = wide(name);
    let ok = match cmd {
        Some(c) => {
            let w = wide(c); // includes the trailing NUL
            RegSetValueExW(k, n.as_ptr(), 0, REG_SZ, w.as_ptr() as *const u8, (w.len() * 2) as u32) == 0
        }
        None => {
            let r = RegDeleteValueW(k, n.as_ptr());
            r == 0 || r == 2 // 2 = it was not there
        }
    };
    RegCloseKey(k);
    ok
}

/// How this exe is launched at login: quoted path, plus a flag so a later version could tell the
/// difference between a manual start and a login start.
pub(crate) fn launch_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\"", exe.display()))
}

pub(crate) unsafe fn is_enabled() -> bool {
    command(VALUE).is_some()
}

pub(crate) unsafe fn set_enabled(on: bool) -> bool {
    if on {
        match launch_command() {
            Some(c) => write(VALUE, Some(&c)),
            None => false,
        }
    } else {
        write(VALUE, None)
    }
}

/// If the user moved or updated the exe, point the startup entry at the new location.
pub(crate) unsafe fn refresh() {
    if let (Some(cur), Some(want)) = (command(VALUE), launch_command()) {
        if cur != want {
            write(VALUE, Some(&want));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_reads_and_removes_a_startup_entry() {
        let name = format!("PixelPetTest{}", std::process::id());
        unsafe {
            assert!(command(&name).is_none());
            assert!(write(&name, Some("\"C:\\Some Folder\\pet.exe\"")));
            assert_eq!(command(&name).as_deref(), Some("\"C:\\Some Folder\\pet.exe\""));
            assert!(write(&name, None));
            assert!(command(&name).is_none());
            assert!(write(&name, None), "removing something already gone is fine");
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_key_is_made_if_there_is_none() {
        let path = format!("Software\\PixelPetTest{}", std::process::id());
        unsafe {
            assert!(open_at(&path, false).is_none());
            for _ in 0..2 {
                let k = open_at(&path, true).expect("made the first time, found the second");
                assert_eq!(RegSetValueExW(k, wide("x").as_ptr(), 0, REG_SZ, wide("y").as_ptr() as *const u8, 4), 0, "it can be written to");
                RegCloseKey(k);
            }
            assert_eq!(RegDeleteKeyW(HKEY_CURRENT_USER, wide(&path).as_ptr()), 0);
        }
    }

    #[test]
    fn launch_command_is_a_quoted_path() {
        let c = launch_command().unwrap();
        assert!(c.starts_with('"') && c.ends_with('"') && (c.contains(".exe") || !cfg!(windows)));
    }
}
