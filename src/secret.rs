//! Keeping keys and private links. On Windows they are locked with the user's account (DPAPI), on a
//! Mac they are in the keychain; on Linux in the keyring if there is one (secret-tool), otherwise in a
//! file that only the user can read. They are never in the settings file.

use crate::prelude::*;

#[cfg(not(windows))]
use std::io::Write;

fn path(name: &str) -> Option<std::path::PathBuf> {
    data_path(&format!("pixel-pet-{name}.key"))
}

/// Keep it; an empty one is forgotten. False if that did not work.
pub(crate) fn set(name: &str, value: &str) -> bool {
    if value.is_empty() {
        return forget(name);
    }
    store(name, value)
}

// ---- Windows ----

#[cfg(windows)]
mod lock {
    use super::*;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

    /// Lock (or unlock) with the account of the user.
    pub fn turn(data: &[u8], locking: bool) -> Option<Vec<u8>> {
        unsafe {
            let from = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
            let mut to: CRYPT_INTEGER_BLOB = zeroed();
            const NO_UI: u32 = 1;
            let ok = if locking { CryptProtectData(&from, null(), null(), null(), null(), NO_UI, &mut to) } else { CryptUnprotectData(&from, null_mut(), null(), null(), null(), NO_UI, &mut to) };
            if ok == 0 || to.pbData.is_null() {
                return None;
            }
            let out = std::slice::from_raw_parts(to.pbData, to.cbData as usize).to_vec();
            LocalFree(to.pbData as *mut std::ffi::c_void);
            Some(out)
        }
    }
}

#[cfg(windows)]
fn store(name: &str, value: &str) -> bool {
    let Some(p) = path(name) else { return false };
    lock::turn(value.as_bytes(), true).is_some_and(|b| std::fs::write(p, b).is_ok())
}

#[cfg(windows)]
pub(crate) fn get(name: &str) -> Option<String> {
    let b = std::fs::read(path(name)?).ok()?;
    String::from_utf8(lock::turn(&b, false)?).ok().filter(|v| !v.is_empty())
}

#[cfg(windows)]
fn forget(name: &str) -> bool {
    path(name).is_some_and(|p| std::fs::remove_file(&p).is_ok() || !p.exists())
}

// ---- elsewhere ----

/// Run a program with something to read. What it says, if it went well.
#[cfg(not(windows))]
fn run(program: &str, args: &[&str], input: &str) -> Option<String> {
    use std::process::{Command, Stdio};
    // a pet with settings of its own (a test, say) keeps its secrets there too
    if std::env::var_os("PIXEL_PET_TEST").is_some() || cfg!(test) {
        return None;
    }
    let mut c = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    c.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let o = c.wait_with_output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string())
}

#[cfg(not(windows))]
fn write_private(p: &std::path::Path, value: &str) -> bool {
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(p); // one that others can read stays that way
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(p).and_then(|mut f| f.write_all(value.as_bytes())).is_ok()
}

/// For `security`'s own command line: no quotes or line breaks can get out.
#[cfg(not(windows))]
fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

#[cfg(not(windows))]
fn unhex(s: &str) -> Option<String> {
    let b: Option<Vec<u8>> = (0..s.len() / 2).map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()).collect();
    String::from_utf8(b?).ok()
}

#[cfg(not(windows))]
fn store(name: &str, value: &str) -> bool {
    let Some(p) = path(name) else { return false };
    let kept = if cfg!(target_os = "macos") {
        // read from a pipe, so that the key is on no command line
        run("/usr/bin/security", &["-i"], &format!("add-generic-password -U -a \"{name}\" -s \"Pixel Pet\" -w \"{}\"\n", hex(value))).is_some() && get(name).as_deref() == Some(value)
    } else {
        run("secret-tool", &["store", "--label=Pixel Pet", "app", "pixel-pet", "name", name], value).is_some()
    };
    if kept {
        let _ = std::fs::remove_file(&p);
        return true;
    }
    write_private(&p, value)
}

#[cfg(not(windows))]
pub(crate) fn get(name: &str) -> Option<String> {
    let kept = if cfg!(target_os = "macos") {
        run("/usr/bin/security", &["find-generic-password", "-a", name, "-s", "Pixel Pet", "-w"], "").and_then(|h| unhex(&h))
    } else {
        run("secret-tool", &["lookup", "app", "pixel-pet", "name", name], "")
    };
    kept.filter(|v| !v.is_empty()).or_else(|| std::fs::read_to_string(path(name)?).ok().filter(|v| !v.is_empty()))
}

#[cfg(not(windows))]
fn forget(name: &str) -> bool {
    if cfg!(target_os = "macos") {
        run("/usr/bin/security", &["delete-generic-password", "-a", name, "-s", "Pixel Pet"], "");
    } else {
        run("secret-tool", &["clear", "app", "pixel-pet", "name", name], "");
    }
    path(name).is_some_and(|p| std::fs::remove_file(&p).is_ok() || !p.exists()) && get(name).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn what_is_locked_can_be_unlocked_and_cannot_be_read() {
        let key = "sk-test-\u{20ac}-123456";
        let b = lock::turn(key.as_bytes(), true).unwrap();
        assert!(!b.windows(8).any(|w| w == b"sk-test-"));
        assert_eq!(lock::turn(&b, false).unwrap(), key.as_bytes());
        assert!(lock::turn(b"not locked by us", false).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn the_file_is_for_the_user_alone() {
        use std::os::unix::fs::PermissionsExt;
        let p = std::env::temp_dir().join(format!("pp-secret-{}", std::process::id()));
        std::fs::write(&p, "readable by all").unwrap();
        assert!(write_private(&p, "a key"));
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "a key");
        std::fs::remove_file(&p).unwrap();
        assert_eq!(unhex(&hex("a \"b\"\n\u{20ac}")).as_deref(), Some("a \"b\"\n\u{20ac}"));
        assert_eq!((unhex("zz"), unhex("")), (None, Some(String::new())));
    }
}
