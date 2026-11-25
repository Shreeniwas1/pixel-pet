//! Is there a newer Pixel Pet? Asks GitHub once a day if the user said yes to it. It only tells, and
//! opens the page; nothing is downloaded or changed by itself.

use crate::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32};

const HOME: &str = "https://github.com/Shreeniwas1/pixel-pet/";
pub(crate) const HERE: &str = env!("CARGO_PKG_VERSION");

/// The setting: not asked yet, yes, no.
pub(crate) const ASK: u8 = 0;
pub(crate) const YES: u8 = 1;
pub(crate) const NO: u8 = 2;

/// A newer one: its version and page.
static NEWER: Mutex<Option<(String, String)>> = Mutex::new(None);
static NOTE: Mutex<&str> = Mutex::new("");
static BUSY: AtomicBool = AtomicBool::new(false);
/// It has not been told yet.
static TELL: AtomicBool = AtomicBool::new(false);
/// The day it last asked.
static ASKED: AtomicU32 = AtomicU32::new(0);

fn numbers(v: &str) -> Option<Vec<u64>> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let n: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    n.filter(|n| !n.is_empty())
}

/// Is `a` newer than `b`? A trial version never is.
pub(crate) fn newer(a: &str, b: &str) -> bool {
    let (Some(mut x), Some(mut y)) = (numbers(a), numbers(b)) else { return false };
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    !a.contains('-') && x > y
}

/// The version and page in GitHub's answer, if it is newer than ours.
pub(crate) fn found(answer: &str) -> Option<Option<(String, String)>> {
    let j = Json::parse(answer)?;
    let (tag, url) = (j.text("tag_name"), j.text("html_url"));
    if tag.is_empty() {
        return None;
    }
    let shown: String = tag.trim_start_matches(['v', 'V']).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.').take(16).collect();
    let page = if url.starts_with(HOME) && net::fine(url) { url.to_string() } else { format!("{HOME}releases") };
    Some(newer(tag, HERE).then_some((shown, page)))
}

fn look() {
    let url = format!("{}/repos/Shreeniwas1/pixel-pet/releases/latest", net::base("PIXEL_PET_GITHUB", "https://api.github.com"));
    let got = net::ask(&net::Ask { url, headers: vec![("Accept".into(), "application/vnd.github+json".into())], follow: true, most: 1 << 20, secs: 25, ..Default::default() });
    let said = got.ok().filter(|a| a.code == 200).and_then(|a| found(&a.text()));
    *NOTE.lock().unwrap() = match &said {
        None => "CANNOT ASK",
        Some(None) => "UP TO DATE",
        Some(Some(_)) => "",
    };
    if let Some(n) = said {
        TELL.store(n.is_some(), Relaxed);
        *NEWER.lock().unwrap() = n;
    }
    BUSY.store(false, Relaxed);
}

/// Look now, on a thread of its own.
pub(crate) fn check() {
    if !BUSY.swap(true, Relaxed) {
        *NOTE.lock().unwrap() = "LOOKING...";
        std::thread::spawn(look);
    }
}

/// Once a day.
pub(crate) fn daily(setting: u8, day: u32) {
    if setting == YES && !friend::is_friend() && ASKED.swap(day, Relaxed) != day {
        check();
    }
}

pub(crate) fn note() -> &'static str {
    *NOTE.lock().unwrap()
}

pub(crate) fn newest() -> Option<(String, String)> {
    NEWER.lock().unwrap().clone()
}

/// Something to say, once for each version. `told` is the one said last.
pub(crate) fn due(told: &str) -> Option<String> {
    if !TELL.swap(false, Relaxed) {
        return None;
    }
    newest().map(|n| n.0).filter(|v| v != told)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_by_their_numbers() {
        for (a, b) in [("0.2.0", "0.1.0"), ("v0.10.0", "0.9.9"), ("1.0", "0.9.9"), ("0.1.1", "0.1"), ("V2", "1.9")] {
            assert!(newer(a, b) && !newer(b, a), "{a} {b}");
        }
        for (a, b) in [("0.1.0", "0.1.0"), ("0.1", "0.1.0"), ("0.2.0-beta", "0.1.0"), ("latest", "0.1.0"), ("", "0.1.0"), ("99999999999999999999", "0.1.0")] {
            assert!(!newer(a, b), "{a} {b}");
        }
    }

    #[test]
    fn only_our_own_page_is_opened() {
        let a = |tag: &str, url: &str| found(&format!(r#"{{"tag_name": "{tag}", "html_url": "{url}"}}"#));
        assert_eq!(a("v99.0.0", "https://github.com/Shreeniwas1/pixel-pet/releases/tag/v99.0.0"), Some(Some(("99.0.0".into(), "https://github.com/Shreeniwas1/pixel-pet/releases/tag/v99.0.0".into()))));
        for evil in ["https://evil.com", "https://github.com/Shreeniwas1/pixel-pet.evil.com/", "file:///c:/windows/system32/calc.exe"] {
            assert_eq!(a("v99.0.0", evil), Some(Some(("99.0.0".into(), format!("{HOME}releases")))));
        }
        assert_eq!(a(HERE, ""), Some(None));
        assert_eq!((found("{}"), found("nope")), (None, None));
        assert!(a("v99.0.0+<script>", "").unwrap().unwrap().0.chars().all(|c| c.is_ascii_alphanumeric() || c == '.'));
    }
}
