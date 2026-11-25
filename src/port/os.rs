//! The clock, the load, memory, battery, network, sounds and starting with the system.

use super::*;
use std::time::Instant;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::System::Power::SYSTEM_POWER_STATUS;
use windows_sys::Win32::System::Registry::HKEY;
use windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX;

type Void = *mut std::ffi::c_void;

#[repr(C)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const u8,
}

extern "C" {
    fn time(t: *mut i64) -> i64;
    fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
}

#[no_mangle]
pub unsafe extern "system" fn GetTickCount() -> u32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    // never 0, which some callers take for "never"
    (START.get_or_init(Instant::now).elapsed().as_millis() as u32).wrapping_add(100_000)
}

#[no_mangle]
pub unsafe extern "system" fn GetLocalTime(t: *mut SYSTEMTIME) {
    let now = time(std::ptr::null_mut());
    let mut m: Tm = std::mem::zeroed();
    localtime_r(&now, &mut m);
    *t = SYSTEMTIME { wYear: (m.year + 1900) as u16, wMonth: (m.mon + 1) as u16, wDayOfWeek: m.wday as u16, wDay: m.mday as u16, wHour: m.hour as u16, wMinute: m.min as u16, wSecond: m.sec as u16, wMilliseconds: 0 };
}

fn filetime(v: u64) -> FILETIME {
    FILETIME { dwLowDateTime: v as u32, dwHighDateTime: (v >> 32) as u32 }
}

// ---- Linux keeps all of this in files ----

#[cfg(not(target_os = "macos"))]
mod here {
    use std::fs::read_to_string;

    /// (idle, everything), counted from the start of the system.
    pub fn cpu(stat: &str) -> Option<(u64, u64)> {
        let l = stat.lines().next()?.strip_prefix("cpu ")?;
        let v: Vec<u64> = l.split_whitespace().take(8).filter_map(|n| n.parse().ok()).collect();
        (v.len() >= 5).then(|| (v[3] + v[4], v.iter().sum()))
    }

    /// Percent of the memory in use.
    pub fn memory(info: &str) -> Option<u32> {
        let get = |k: &str| info.lines().find_map(|l| l.strip_prefix(k)?.trim().split_whitespace().next()?.parse::<u64>().ok());
        let (all, free) = (get("MemTotal:")?, get("MemAvailable:")?);
        (all > 0).then(|| ((all - free.min(all)) * 100 / all) as u32)
    }

    /// Bytes (received, sent) by everything but the loopback and things made of software.
    pub fn network(dev: &str) -> (u64, u64) {
        dev.lines()
            .filter_map(|l| l.split_once(':'))
            .filter(|(n, _)| !["lo", "docker", "veth", "br-", "virbr", "tun", "tap"].iter().any(|p| n.trim().starts_with(p)))
            .filter_map(|(_, v)| {
                let v: Vec<u64> = v.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                Some((*v.first()?, *v.get(8)?))
            })
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    }

    pub fn cpu_now() -> Option<(u64, u64)> {
        cpu(&read_to_string("/proc/stat").ok()?)
    }

    pub fn memory_now() -> Option<u32> {
        memory(&read_to_string("/proc/meminfo").ok()?)
    }

    pub fn network_now() -> (u64, u64) {
        network(&read_to_string("/proc/net/dev").unwrap_or_default())
    }

    /// (percent, plugged in)
    pub fn battery() -> Option<(u32, bool)> {
        for e in std::fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
            let p = e.path();
            let get = |f: &str| read_to_string(p.join(f)).ok().map(|t| t.trim().to_string());
            if get("type").as_deref() == Some("Battery") && get("scope").as_deref() != Some("Device") {
                return Some((get("capacity")?.parse().ok()?, get("status").is_some_and(|s| s != "Discharging")));
            }
        }
        None
    }

    pub fn program(pid: u32) -> Option<String> {
        Some(std::fs::read_link(format!("/proc/{pid}/exe")).ok()?.to_string_lossy().into_owned())
    }

    /// Programs that can play a sound, best first.
    pub const PLAYERS: &[(&str, &[&str])] = &[("pw-play", &[]), ("paplay", &[]), ("aplay", &["-q"])];
}

#[cfg(target_os = "macos")]
mod here {
    type Void = *mut std::ffi::c_void;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPSCopyPowerSourcesInfo() -> Void;
        fn IOPSCopyPowerSourcesList(info: Void) -> Void;
        fn IOPSGetPowerSourceDescription(info: Void, source: Void) -> Void;
        fn IOPSGetProvidingPowerSourceType(info: Void) -> Void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(a: Void) -> isize;
        fn CFArrayGetValueAtIndex(a: Void, i: isize) -> Void;
        fn CFDictionaryGetValue(d: Void, k: Void) -> Void;
        fn CFNumberGetValue(n: Void, kind: isize, to: *mut i32) -> u8;
        fn CFStringCreateWithCString(alloc: Void, s: *const u8, enc: u32) -> Void;
        fn CFStringGetCString(s: Void, to: *mut u8, n: isize, enc: u32) -> u8;
        fn CFRelease(o: Void);
    }

    extern "C" {
        fn mach_host_self() -> u32;
        fn host_statistics(host: u32, flavor: i32, out: *mut i32, n: *mut u32) -> i32;
        fn host_statistics64(host: u32, flavor: i32, out: *mut i32, n: *mut u32) -> i32;
        fn sysctlbyname(name: *const u8, out: Void, n: *mut usize, new: Void, len: usize) -> i32;
        fn getifaddrs(p: *mut *mut IfAddrs) -> i32;
        fn freeifaddrs(p: *mut IfAddrs);
        fn proc_pidpath(pid: i32, to: *mut u8, n: u32) -> i32;
    }

    #[repr(C)]
    struct IfAddrs {
        next: *mut IfAddrs,
        name: *const u8,
        flags: u32,
        addr: *const u8,
        mask: Void,
        dst: Void,
        data: *const u32,
    }

    /// Asked for once: every answer has to be given back otherwise.
    fn host() -> u32 {
        static HOST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *HOST.get_or_init(|| unsafe { mach_host_self() })
    }

    unsafe fn key(s: &str) -> Void {
        CFStringCreateWithCString(std::ptr::null_mut(), format!("{s}\0").as_ptr(), 0x0800_0100)
    }

    pub fn cpu_now() -> Option<(u64, u64)> {
        unsafe {
            let (mut v, mut n) = ([0i32; 4], 4u32);
            // HOST_CPU_LOAD_INFO: user, system, idle, nice
            (host_statistics(host(), 3, v.as_mut_ptr(), &mut n) == 0).then(|| (v[2] as u32 as u64, v.iter().map(|t| *t as u32 as u64).sum()))
        }
    }

    pub fn memory_now() -> Option<u32> {
        unsafe {
            let (mut all, mut len) = (0u64, 8usize);
            if sysctlbyname(b"hw.memsize\0".as_ptr(), &mut all as *mut _ as Void, &mut len, std::ptr::null_mut(), 0) != 0 || all == 0 {
                return None;
            }
            let (mut page, mut len) = (0u32, 4usize);
            sysctlbyname(b"hw.pagesize\0".as_ptr(), &mut page as *mut _ as Void, &mut len, std::ptr::null_mut(), 0);
            let (mut v, mut n) = ([0i32; 64], 38u32);
            // HOST_VM_INFO64: free, active, inactive, wired, ...
            if host_statistics64(host(), 4, v.as_mut_ptr(), &mut n) != 0 {
                return None;
            }
            let free = (v[0] as u32 as u64 + v[2] as u32 as u64) * page.max(4096) as u64;
            Some(((all - free.min(all)) * 100 / all) as u32)
        }
    }

    pub fn network_now() -> (u64, u64) {
        let mut sum = (0u64, 0u64);
        unsafe {
            let mut first = std::ptr::null_mut();
            if getifaddrs(&mut first) != 0 {
                return sum;
            }
            let mut p = first;
            while !p.is_null() {
                let a = &*p;
                // a link-level address, of something called en...
                if !a.addr.is_null() && *a.addr.add(1) == 18 && !a.data.is_null() && *a.name == b'e' && *a.name.add(1) == b'n' {
                    // if_data: 8 bytes of types, then mtu, metric, baudrate, ipackets, ierrors, opackets, oerrors, collisions, ibytes, obytes
                    sum.0 += *a.data.add(10) as u64;
                    sum.1 += *a.data.add(11) as u64;
                }
                p = a.next;
            }
            freeifaddrs(first);
        }
        sum
    }

    pub fn battery() -> Option<(u32, bool)> {
        unsafe {
            let info = IOPSCopyPowerSourcesInfo();
            if info.is_null() {
                return None;
            }
            let list = IOPSCopyPowerSourcesList(info);
            let mut out = None;
            if !list.is_null() && CFArrayGetCount(list) > 0 {
                let d = IOPSGetPowerSourceDescription(info, CFArrayGetValueAtIndex(list, 0));
                if !d.is_null() {
                    let get = |k: &str| {
                        let k = key(k);
                        let n = CFDictionaryGetValue(d, k);
                        CFRelease(k);
                        let mut v = 0;
                        (!n.is_null() && CFNumberGetValue(n, 3, &mut v) != 0).then_some(v)
                    };
                    if let (Some(now), Some(max)) = (get("Current Capacity"), get("Max Capacity")) {
                        let kind = IOPSGetProvidingPowerSourceType(info);
                        let mut b = [0u8; 64];
                        let ac = !kind.is_null() && CFStringGetCString(kind, b.as_mut_ptr(), 64, 0x0800_0100) != 0 && b.starts_with(b"AC Power");
                        out = (max > 0).then_some(((now * 100 / max) as u32, ac));
                    }
                }
            }
            if !list.is_null() {
                CFRelease(list);
            }
            CFRelease(info);
            out
        }
    }

    pub fn program(pid: u32) -> Option<String> {
        let mut b = [0u8; 4096];
        let n = unsafe { proc_pidpath(pid as i32, b.as_mut_ptr(), 4096) };
        (n > 0).then(|| String::from_utf8_lossy(&b[..n as usize]).into_owned())
    }

    pub const PLAYERS: &[(&str, &[&str])] = &[("afplay", &[])];
}

#[no_mangle]
pub unsafe extern "system" fn GetSystemTimes(idle: *mut FILETIME, kernel: *mut FILETIME, user: *mut FILETIME) -> BOOL {
    let Some((i, all)) = here::cpu_now() else { return 0 };
    *idle = filetime(i);
    *kernel = filetime(all);
    *user = filetime(0);
    1
}

#[no_mangle]
pub unsafe extern "system" fn GlobalMemoryStatusEx(m: *mut MEMORYSTATUSEX) -> BOOL {
    let Some(p) = here::memory_now() else { return 0 };
    (*m).dwMemoryLoad = p;
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetSystemPowerStatus(p: *mut SYSTEM_POWER_STATUS) -> BOOL {
    let Some((percent, ac)) = here::battery() else { return 0 };
    *p = std::mem::zeroed();
    (*p).BatteryLifePercent = percent.min(100) as u8;
    (*p).ACLineStatus = ac as u8;
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetIfTable(t: *mut MIB_IFTABLE, size: *mut u32, _: BOOL) -> u32 {
    let need = std::mem::size_of::<MIB_IFTABLE>() as u32;
    if t.is_null() || *size < need {
        *size = need;
        return 122;
    }
    // one that stands for all of them
    *t = std::mem::zeroed();
    (*t).dwNumEntries = 1;
    (*t).table[0].dwIndex = 1;
    (*t).table[0].dwType = 6;
    (*t).table[0].dwOperStatus = 5;
    0
}

#[no_mangle]
pub unsafe extern "system" fn GetIfEntry(row: *mut MIB_IFROW) -> u32 {
    let (rx, tx) = here::network_now();
    (*row).dwOperStatus = 5;
    (*row).dwInOctets = rx as u32;
    (*row).dwOutOctets = tx as u32;
    0
}

#[no_mangle]
pub unsafe extern "system" fn GetCurrentProcessId() -> u32 {
    std::process::id()
}

/// A process is its number.
#[no_mangle]
pub unsafe extern "system" fn OpenProcess(_: u32, _: BOOL, pid: u32) -> HANDLE {
    pid as usize as HANDLE
}

#[no_mangle]
pub unsafe extern "system" fn CloseHandle(_: HANDLE) -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn QueryFullProcessImageNameW(h: HANDLE, _: u32, to: *mut u16, n: *mut u32) -> BOOL {
    let Some(p) = here::program(h as usize as u32) else { return 0 };
    // the rest of the program takes paths apart at backslashes
    *n = put(&p.replace('/', "\\"), to, *n as i32) as u32;
    1
}

#[no_mangle]
pub unsafe extern "system" fn SHQueryUserNotificationState(s: *mut i32) -> i32 {
    *s = 5; // nothing special
    0
}

// ---- sounds ----

/// Which of the players there is.
fn player() -> Option<(std::path::PathBuf, &'static [&'static str])> {
    let path = std::env::var_os("PATH")?;
    here::PLAYERS.iter().find_map(|(name, args)| std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file()).map(|p| (p, *args)))
}

#[no_mangle]
pub unsafe extern "system" fn PlaySoundW(wav: *const u16, _: HMODULE, _: u32) -> BOOL {
    static FILES: Mutex<Option<std::collections::HashMap<usize, std::path::PathBuf>>> = Mutex::new(None);
    if wav.is_null() {
        return 1;
    }
    let mut f = FILES.lock().unwrap();
    let path = f.get_or_insert_with(Default::default).entry(wav as usize).or_insert_with(|| {
        let p = wav as *const u8;
        let n = u32::from_le_bytes([*p.add(4), *p.add(5), *p.add(6), *p.add(7)]) as usize + 8;
        let to = std::env::temp_dir().join(format!("pixel-pet-{}-{:x}.wav", std::process::id(), wav as usize));
        let _ = std::fs::write(&to, std::slice::from_raw_parts(p, n));
        to
    });
    let Some((exe, args)) = player() else { return 0 };
    std::process::Command::new(exe).args(args).arg(path).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().is_ok() as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn MessageBeep(_: u32) -> BOOL {
    1
}

// ---- starting with the system: the one entry of the registry the pet knows ----

/// Where the system looks for programs to start.
fn entry(name: &str) -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        Some(home.join("Library/LaunchAgents").join(format!("io.github.pixel-pet.{name}.plist")))
    } else {
        let base = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()).map(std::path::PathBuf::from).unwrap_or_else(|| home.join(".config"));
        Some(base.join("autostart").join(format!("{name}.desktop")))
    }
}

fn unquoted(cmd: &str) -> &str {
    cmd.trim().trim_matches('"')
}

pub(crate) fn entry_text(cmd: &str, mac: bool) -> String {
    let exe = unquoted(cmd);
    if mac {
        let exe = exe.replace('&', "&amp;").replace('<', "&lt;");
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>io.github.pixel-pet</string>\n<key>ProgramArguments</key><array><string>{exe}</string></array>\n<key>RunAtLoad</key><true/>\n</dict></plist>\n")
    } else {
        format!("[Desktop Entry]\nType=Application\nName=Pixel Pet\nComment=A tiny pixel pet that lives on your desktop\nExec=\"{exe}\"\nX-GNOME-Autostart-enabled=true\n")
    }
}

/// The command in such a file, the way it was given.
pub(crate) fn entry_command(text: &str) -> Option<String> {
    let exe = match text.lines().find_map(|l| l.strip_prefix("Exec=")) {
        Some(e) => unquoted(e).to_string(),
        None => text.split("<array><string>").nth(1)?.split("</string>").next()?.replace("&lt;", "<").replace("&amp;", "&"),
    };
    Some(format!("\"{exe}\""))
}

#[no_mangle]
pub unsafe extern "system" fn RegCreateKeyW(_: HKEY, _: *const u16, k: *mut HKEY) -> u32 {
    *k = 1usize as HKEY;
    0
}

#[no_mangle]
pub unsafe extern "system" fn RegOpenKeyExW(_: HKEY, _: *const u16, _: u32, _: u32, k: *mut HKEY) -> u32 {
    *k = 1usize as HKEY;
    0
}

#[no_mangle]
pub unsafe extern "system" fn RegCloseKey(_: HKEY) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "system" fn RegQueryValueExW(_: HKEY, name: *const u16, _: *const u32, _: *mut u32, to: *mut u8, size: *mut u32) -> u32 {
    let Some(cmd) = entry(&string(name)).and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| entry_command(&t)) else { return 2 };
    let w: Vec<u16> = cmd.encode_utf16().chain([0]).collect();
    if !to.is_null() && *size as usize >= w.len() * 2 {
        std::ptr::copy_nonoverlapping(w.as_ptr() as *const u8, to, w.len() * 2);
    }
    *size = (w.len() * 2) as u32;
    0
}

#[no_mangle]
pub unsafe extern "system" fn RegSetValueExW(_: HKEY, name: *const u16, _: u32, _: u32, from: *const u8, _: u32) -> u32 {
    let Some(p) = entry(&string(name)) else { return 5 };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if std::fs::write(p, entry_text(&string(from as *const u16), cfg!(target_os = "macos"))).is_ok() { 0 } else { 5 }
}

#[no_mangle]
pub unsafe extern "system" fn RegDeleteValueW(_: HKEY, name: *const u16) -> u32 {
    match entry(&string(name)).map(std::fs::remove_file) {
        Some(Ok(())) => 0,
        _ => 2,
    }
}

// ---- things only Windows has; callers do without ----

#[no_mangle]
pub unsafe extern "system" fn LoadLibraryW(_: *const u16) -> HMODULE {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "system" fn GetProcAddress(_: HMODULE, _: *const u8) -> FARPROC {
    None
}

#[no_mangle]
pub unsafe extern "system" fn GetModuleHandleW(_: *const u16) -> HMODULE {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "system" fn CoInitializeEx(_: *const std::ffi::c_void, _: u32) -> i32 {
    -1
}

#[no_mangle]
pub unsafe extern "system" fn CoCreateInstance(_: *const std::ffi::c_void, _: Void, _: u32, _: *const std::ffi::c_void, out: *mut Void) -> i32 {
    *out = std::ptr::null_mut();
    -1
}

#[no_mangle]
pub unsafe extern "system" fn SHChangeNotify(_: i32, _: u32, _: *const std::ffi::c_void, _: *const std::ffi::c_void) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_for_starting_up_says_which_program() {
        for mac in [false, true] {
            for cmd in ["\"/home/a b/pixel-pet\"", "\"/opt/x&y/pixel-pet\""] {
                assert_eq!(entry_command(&entry_text(cmd, mac)).as_deref(), Some(cmd));
            }
        }
        assert_eq!(entry_command("nonsense"), None);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn what_linux_writes_about_itself_is_understood() {
        assert_eq!(here::cpu("cpu  10 1 5 80 4 0 0 0 0 0\ncpu0 1 2 3"), Some((84, 100)));
        assert_eq!(here::cpu("garbage"), None);
        assert_eq!(here::memory("MemTotal:  1000 kB\nMemFree: 1 kB\nMemAvailable:  250 kB\n"), Some(75));
        let dev = "Inter-|   Receive\n face |bytes\n    lo: 99 1 0 0 0 0 0 0 99 1 0 0 0 0 0 0\n  eth0: 1000 5 0 0 0 0 0 0 200 4 0 0 0 0 0 0\nveth1: 7 0 0 0 0 0 0 0 7 0 0 0 0 0 0 0\n";
        assert_eq!(here::network(dev), (1000, 200));
    }

    #[test]
    fn this_machine_can_be_asked() {
        assert!(here::cpu_now().is_some_and(|(i, all)| i <= all && all > 0));
        assert!(here::memory_now().is_some_and(|p| (1..=100).contains(&p)));
        assert!(here::program(std::process::id()).is_some_and(|p| p.starts_with('/')));
        let mut t: SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut t) };
        assert!(t.wYear >= 2025 && (1..=12).contains(&t.wMonth) && t.wHour < 24);
    }
}
