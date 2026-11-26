//! It cheers when a big download lands in the Downloads folder, and when Windows has finished
//! copying files. Only names and sizes are looked at, on this computer; nothing is read or sent.

use crate::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;

/// Looking at all.
pub(crate) static ON: AtomicBool = AtomicBool::new(false);
/// What there is to say, oldest first.
static NEWS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Smaller downloads are not worth a cheer.
pub(crate) const BIG: u64 = 10 << 20;
/// How browsers and download tools name a file they are still writing.
const PARTIAL: [&str; 10] = ["crdownload", "part", "partial", "download", "tmp", "opdownload", "!ut", "aria2", "filepart", "crswap"];
const EVERY: Duration = Duration::from_secs(2);

fn partial(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| PARTIAL.iter().any(|p| ext.eq_ignore_ascii_case(p)))
}

/// What the Downloads folder looked like: a file is done when it has kept its size for one look
/// and nothing beside it is still being written.
#[derive(Default)]
pub(crate) struct Folder {
    started: bool,
    seen: HashMap<String, u64>,
    growing: HashMap<String, u64>,
}

impl Folder {
    /// The files that are there now, and their sizes. Returns the big ones that just finished.
    pub fn look(&mut self, now: &[(String, u64)]) -> Vec<(String, u64)> {
        if !self.started {
            // what was there before the pet looked is old news
            self.started = true;
            self.seen = now.iter().filter(|(n, _)| !partial(n)).cloned().collect();
            return Vec::new();
        }
        let names: HashSet<&str> = now.iter().map(|(n, _)| n.as_str()).collect();
        let mut done = Vec::new();
        for (name, size) in now {
            if partial(name) || self.seen.contains_key(name) {
                continue;
            }
            if PARTIAL.iter().any(|e| names.contains(format!("{name}.{e}").as_str())) {
                self.growing.remove(name); // Firefox keeps an empty one until the end
                continue;
            }
            match self.growing.insert(name.clone(), *size) {
                Some(before) if before == *size => {
                    self.growing.remove(name);
                    self.seen.insert(name.clone(), *size);
                    if *size >= BIG {
                        done.push((name.clone(), *size));
                    }
                }
                _ => {}
            }
        }
        // what was taken away may come again
        self.seen.retain(|n, _| names.contains(n.as_str()));
        self.growing.retain(|n, _| names.contains(n.as_str()));
        done
    }

    /// Something is still on its way, so look again even if the folder did not change.
    pub fn waiting(&self) -> bool {
        !self.growing.is_empty()
    }
}

/// Windows' window that shows a copy going: seen for how many looks, and how far it got.
#[derive(Default)]
pub(crate) struct Copy {
    looks: u32,
    last: Option<u32>,
}

impl Copy {
    /// `now` is None without the window, else what percent its title says. True when a copy that
    /// took a while has just finished; one that was cancelled half way is not cheered.
    pub fn look(&mut self, now: Option<Option<u32>>) -> bool {
        match now {
            Some(p) => {
                self.looks += 1;
                self.last = p.or(self.last);
                false
            }
            None => {
                let done = self.looks >= 3 && self.last.map_or(true, |p| p >= 90);
                *self = Copy::default();
                done
            }
        }
    }
}

/// "34% complete", "34 % terminé": the number before the percent sign.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn percent(title: &str) -> Option<u32> {
    let at = title.find('%')?;
    let digits: String = title[..at].trim_end().chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().ok().filter(|p| *p <= 100)
}

pub(crate) fn size_text(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b => format!("{} MB", (b + (1 << 19)) >> 20),
    }
}

/// The name, cut short if it is long.
fn short(name: &str) -> String {
    if name.chars().count() <= 34 {
        return name.to_string();
    }
    let (stem, ext) = name.rsplit_once('.').filter(|(_, e)| e.len() <= 6).unwrap_or((name, ""));
    let keep: String = stem.chars().take(28).collect();
    if ext.is_empty() { format!("{keep}...") } else { format!("{keep}....{ext}") }
}

/// Where downloads go.
fn folder() -> Option<std::path::PathBuf> {
    if let Some(d) = std::env::var_os("PIXEL_PET_DOWNLOADS").filter(|_| std::env::var_os("PIXEL_PET_TEST").is_some()) {
        return Some(d.into()); // a test must not depend on what is in the real folder
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath};
        let mut p: windows_sys::core::PWSTR = null_mut();
        if SHGetKnownFolderPath(&FOLDERID_Downloads, 0, null_mut(), &mut p) == 0 && !p.is_null() {
            let n = (0..).take_while(|i| *p.add(*i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            CoTaskMemFree(p as *const _);
            return Some(s.into());
        }
        CoTaskMemFree(p as *const _);
        std::env::var_os("USERPROFILE").map(|h| std::path::PathBuf::from(h).join("Downloads"))
    }
    #[cfg(not(windows))]
    {
        let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
        // Linux desktops may call it something else, in the language of the user
        let named = std::fs::read_to_string(home.join(".config/user-dirs.dirs")).ok().and_then(|t| xdg_download(&t, &home));
        Some(named.unwrap_or_else(|| home.join("Downloads")))
    }
}

/// XDG_DOWNLOAD_DIR="$HOME/Downloads" from user-dirs.dirs.
#[cfg(not(windows))]
fn xdg_download(t: &str, home: &std::path::Path) -> Option<std::path::PathBuf> {
    let line = t.lines().find(|l| l.trim_start().starts_with("XDG_DOWNLOAD_DIR="))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let p = match v.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => std::path::PathBuf::from(v),
    };
    (p != home && p.is_absolute()).then_some(p)
}

fn files(dir: &std::path::Path) -> Vec<(String, u64)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok().filter(|m| m.is_file())?;
            Some((e.file_name().to_string_lossy().into_owned(), m.len()))
        })
        .filter(|(n, _)| !n.starts_with('.'))
        .take(5000)
        .collect()
}

/// Is Windows copying files: None without its window, else the percent in its title.
#[cfg(windows)]
fn copying() -> Option<Option<u32>> {
    unsafe {
        let h = FindWindowW(wide("OperationStatusWindow").as_ptr(), null());
        if h.is_null() || IsWindowVisible(h) == 0 {
            return None;
        }
        let mut buf = [0u16; 160];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        Some(percent(&String::from_utf16_lossy(&buf[..n])))
    }
}

#[cfg(not(windows))]
fn copying() -> Option<Option<u32>> {
    None
}

fn tell(text: String) {
    let mut n = NEWS.lock().unwrap();
    if n.len() < 4 && !n.contains(&text) {
        n.push(text);
    }
}

pub(crate) fn spawn() {
    std::thread::spawn(|| {
        let (mut dl, mut copy) = (Folder::default(), Copy::default());
        let mut stamp = None;
        loop {
            std::thread::sleep(EVERY);
            if !ON.load(Relaxed) {
                (dl, copy, stamp) = (Folder::default(), Copy::default(), None);
                continue;
            }
            if copy.look(copying()) {
                tell("All done! Your files have finished copying.".into());
            }
            let Some(dir) = folder() else { continue };
            // a folder only changes its time when something comes or goes; growing files are watched
            let now = std::fs::metadata(&dir).and_then(|m| m.modified()).ok();
            if now == stamp && !dl.waiting() {
                continue;
            }
            stamp = now;
            for (name, size) in dl.look(&files(&dir)) {
                tell(format!("Your download is done: {} ({}).", short(&name), size_text(size)));
            }
        }
    });
}

/// Something to say now, if there is.
pub(crate) fn due() -> Option<String> {
    let mut n = NEWS.lock().unwrap();
    (!n.is_empty()).then(|| n.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(list: &[(&str, u64)]) -> Vec<(String, u64)> {
        list.iter().map(|(n, s)| (n.to_string(), *s)).collect()
    }

    const MB: u64 = 1 << 20;

    #[test]
    fn what_was_there_first_is_old_news() {
        let mut d = Folder::default();
        assert!(d.look(&f(&[("old.zip", 900 * MB)])).is_empty());
        assert!(d.look(&f(&[("old.zip", 900 * MB)])).is_empty());
        assert!(!d.waiting());
    }

    #[test]
    fn a_chrome_download_is_done_when_it_is_renamed_and_keeps_its_size() {
        let mut d = Folder::default();
        d.look(&[]);
        assert!(d.look(&f(&[("game.zip.crdownload", 200 * MB)])).is_empty());
        assert!(d.look(&f(&[("game.zip.crdownload", 400 * MB)])).is_empty());
        assert!(d.look(&f(&[("game.zip", 512 * MB)])).is_empty()); // first seen
        assert!(d.waiting());
        assert_eq!(d.look(&f(&[("game.zip", 512 * MB)])), f(&[("game.zip", 512 * MB)]));
        assert!(d.look(&f(&[("game.zip", 512 * MB)])).is_empty()); // only once
    }

    #[test]
    fn firefox_keeps_an_empty_file_beside_the_part_until_the_end() {
        let mut d = Folder::default();
        d.look(&[]);
        for k in 1..4 {
            assert!(d.look(&f(&[("film.mkv", 0), ("film.mkv.part", k * 100 * MB)])).is_empty());
        }
        assert!(d.look(&f(&[("film.mkv", 300 * MB)])).is_empty());
        assert_eq!(d.look(&f(&[("film.mkv", 300 * MB)])).len(), 1);
    }

    #[test]
    fn small_files_and_files_still_growing_are_not_cheered() {
        let mut d = Folder::default();
        d.look(&[]);
        d.look(&f(&[("note.pdf", MB)]));
        assert!(d.look(&f(&[("note.pdf", MB)])).is_empty());
        d.look(&f(&[("copy.iso", 100 * MB)]));
        assert!(d.look(&f(&[("copy.iso", 200 * MB)])).is_empty());
        assert_eq!(d.look(&f(&[("copy.iso", 200 * MB)])).len(), 1);
    }

    #[test]
    fn a_file_taken_away_and_fetched_again_is_news_again() {
        let mut d = Folder::default();
        d.look(&f(&[("a.zip", 50 * MB)]));
        d.look(&[]);
        d.look(&f(&[("a.zip", 50 * MB)]));
        assert_eq!(d.look(&f(&[("a.zip", 50 * MB)])).len(), 1);
    }

    #[test]
    fn a_copy_is_cheered_when_it_ends_but_not_when_it_was_cancelled() {
        let mut c = Copy::default();
        assert!(!c.look(None));
        for p in [10, 50, 95] {
            assert!(!c.look(Some(Some(p))));
        }
        assert!(c.look(None));
        for p in [10, 40, 45] {
            c.look(Some(Some(p)));
        }
        assert!(!c.look(None)); // stopped at 45%
        c.look(Some(None));
        assert!(!c.look(None)); // a moment is not a copy worth a cheer
        for _ in 0..3 {
            c.look(Some(None));
        }
        assert!(c.look(None)); // a title without a number: it took a while, so cheer
    }

    #[test]
    fn the_percent_is_read_in_any_language() {
        assert_eq!(percent("34% complete"), Some(34));
        assert_eq!(percent("34 % terminé"), Some(34));
        assert_eq!(percent("Kopieren: 100% abgeschlossen"), Some(100));
        assert_eq!(percent("Copying"), None);
        assert_eq!(percent("% done"), None);
    }

    #[test]
    fn names_partial_files_and_sizes() {
        assert!(partial("a.CRDOWNLOAD") && partial("b.part") && partial("x.zip.!ut"));
        assert!(!partial("report.pdf") && !partial("partial"));
        assert_eq!(size_text(1536 * MB), "1.5 GB");
        assert_eq!(size_text(12 * MB), "12 MB");
        assert_eq!(short("a-very-long-name-of-a-file-from-somewhere-far-away.tar.gz"), "a-very-long-name-of-a-file-f....gz");
        assert_eq!(short("game.zip"), "game.zip");
    }

    #[test]
    #[cfg(not(windows))]
    fn linux_says_where_downloads_go() {
        let home = std::path::Path::new("/home/a");
        assert_eq!(xdg_download("XDG_DOWNLOAD_DIR=\"$HOME/Téléchargements\"\n", home), Some(home.join("Téléchargements")));
        assert_eq!(xdg_download("XDG_DOWNLOAD_DIR=\"/data/dl\"", home), Some("/data/dl".into()));
        assert_eq!(xdg_download("XDG_DOWNLOAD_DIR=\"$HOME/\"", home), None); // the home itself is not it
        assert_eq!(xdg_download("XDG_MUSIC_DIR=\"$HOME/Music\"", home), None);
    }
}
