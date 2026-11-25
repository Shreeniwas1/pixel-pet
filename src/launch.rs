//! Quick launch: the user's own apps, files, folders and links, one click away. Things are added by
//! dropping them on the pet, from the program in front, or from a copied link.

use crate::prelude::*;
use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};

pub(crate) const MOST: usize = 8;

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Entry {
    pub name: String,
    pub target: String,
}

// ---- rules (pure, so they can be tested) ----

pub(crate) fn is_link(t: &str) -> bool {
    let l = t.to_lowercase();
    (l.starts_with("http://") || l.starts_with("https://")) && l.len() > 10 && !t.contains(char::is_whitespace) && !t.contains('"')
}

/// A short name for a target: the site, the program or the folder.
pub(crate) fn name_of(target: &str) -> String {
    let short = if is_link(target) {
        let host = target.split("//").nth(1).unwrap_or("").split(['/', '?', '#']).next().unwrap_or("");
        host.trim_start_matches("www.").to_string()
    } else {
        let last = target.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(target);
        let l = last.to_lowercase();
        let cut = [".exe", ".lnk", ".url", ".bat", ".cmd"].iter().find(|e| l.ends_with(**e)).map_or(0, |e| e.len());
        last[..last.len() - cut].to_string()
    };
    let name: String = short.chars().filter(|c| !c.is_control()).take(24).collect();
    if name.is_empty() { "?".into() } else { name }
}

/// Add a target unless it is there already or the list is full. True if it was added.
pub(crate) fn add(list: &mut Vec<Entry>, target: &str) -> bool {
    let t = target.trim();
    if t.is_empty() || t.contains(['\t', '\n', '\r']) || list.len() >= MOST || list.iter().any(|e| e.target.eq_ignore_ascii_case(t)) {
        return false;
    }
    list.push(Entry { name: name_of(t), target: t.to_string() });
    true
}

pub(crate) fn to_text(list: &[Entry]) -> String {
    list.iter().map(|e| format!("{}\t{}\n", e.name, e.target)).collect()
}

pub(crate) fn from_text(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(n, t)| !n.is_empty() && !t.is_empty())
        .take(MOST)
        .map(|(n, t)| Entry { name: n.to_string(), target: t.to_string() })
        .collect()
}

// ---- doing it ----

const FILE: &str = "pixel-pet-launch.txt";

pub(crate) fn load() -> Vec<Entry> {
    data_path(FILE).and_then(|p| std::fs::read_to_string(p).ok()).map_or(Vec::new(), |t| from_text(&t))
}

pub(crate) fn save(list: &[Entry]) {
    if let Some(p) = data_path(FILE) {
        let _ = std::fs::write(p, to_text(list));
    }
}

/// Open it. Explorer does the opening, so nothing of the shell is loaded into the pet, and a
/// program is started the way a double click would.
pub(crate) fn open(e: &Entry) -> bool {
    if std::env::var_os("PIXEL_PET_TEST").is_some() {
        return true;
    }
    std::process::Command::new("explorer.exe").arg(&e.target).spawn().is_ok()
}

/// What was dropped on the pet.
pub(crate) unsafe fn dropped(drop: HDROP) -> Vec<String> {
    let n = DragQueryFileW(drop, u32::MAX, null_mut(), 0).min(MOST as u32);
    let mut out = Vec::new();
    for i in 0..n {
        let mut buf = [0u16; 1024];
        let len = DragQueryFileW(drop, i, buf.as_mut_ptr(), buf.len() as u32);
        if len > 0 {
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
    }
    DragFinish(drop);
    out
}

/// The link on the clipboard, if that is what is there.
pub(crate) unsafe fn copied_link() -> Option<String> {
    const UNICODE_TEXT: u32 = 13;
    if IsClipboardFormatAvailable(UNICODE_TEXT) == 0 || OpenClipboard(null_mut()) == 0 {
        return None;
    }
    let h = GetClipboardData(UNICODE_TEXT);
    let mut text = String::new();
    if !h.is_null() {
        let p = GlobalLock(h) as *const u16;
        if !p.is_null() {
            let most = (GlobalSize(h) / 2).min(2000);
            let s = std::slice::from_raw_parts(p, most);
            let len = s.iter().position(|c| *c == 0).unwrap_or(most);
            text = String::from_utf16_lossy(&s[..len]);
            GlobalUnlock(h);
        }
    }
    CloseClipboard();
    let t = text.trim().to_string();
    is_link(&t).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_short_and_sensible() {
        assert_eq!(name_of("C:\\Program Files\\Blender\\blender.exe"), "blender");
        assert_eq!(name_of("D:\\Projects\\"), "Projects");
        assert_eq!(name_of("https://www.example.com/a/b?c=1"), "example.com");
        assert_eq!(name_of("C:\\Users\\me\\Desktop\\My Game.lnk"), "My Game");
        assert_eq!(name_of("C:\\notes.txt"), "notes.txt");
        assert!(name_of(&format!("C:\\{}.exe", "x".repeat(90))).len() <= 24);
    }

    #[test]
    fn only_real_links_count_as_links() {
        assert!(is_link("https://example.com/x") && is_link("HTTP://a.example"));
        for t in ["javascript:alert(1)", "file:///c:/x", "https://", "see https://a.b", "https://a.b/\" --run", "calc.exe"] {
            assert!(!is_link(t), "{t}");
        }
    }

    #[test]
    fn the_list_takes_no_doubles_and_has_an_end() {
        let mut l = Vec::new();
        assert!(add(&mut l, "C:\\a.exe") && !add(&mut l, "c:\\A.EXE") && !add(&mut l, "  ") && !add(&mut l, "a\tb"));
        for i in 0..20 {
            add(&mut l, &format!("C:\\p{i}.exe"));
        }
        assert_eq!(l.len(), MOST);
    }

    #[test]
    fn saved_and_loaded_again_it_is_the_same() {
        let mut l = Vec::new();
        add(&mut l, "C:\\Some Folder\\thing.exe");
        add(&mut l, "https://example.com");
        assert_eq!(from_text(&to_text(&l)), l);
        assert!(from_text("nonsense\n\t\nname\t\n").is_empty());
    }
}
