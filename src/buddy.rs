//! The coding buddy: notices when you type in an editor or terminal, and when Claude Code finishes
//! or asks something. It never sees what you type, only that the keyboard was used; and from
//! Claude Code's own log it reads only who spoke last and whether they were done.

use crate::prelude::*;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32};

/// The log watcher only works while somebody cares.
pub(crate) static WANTED: AtomicBool = AtomicBool::new(false);
/// Claude is working on something.
pub(crate) static WORKING: AtomicBool = AtomicBool::new(false);
/// Counts finished jobs and questions, so whoever looks can tell a new one.
pub(crate) static FINISHED: AtomicU32 = AtomicU32::new(0);
pub(crate) static ASKED: AtomicU32 = AtomicU32::new(0);

// ---- rules (pure, so they can be tested) ----

/// Is this a program people write code in?
pub(crate) fn is_coding(exe: &str) -> bool {
    const CODE: &[&str] = &[
        "code", "code - insiders", "cursor", "windsurf", "zed", "devenv", "idea64", "pycharm64", "webstorm64", "clion64", "rider64", "goland64", "rustrover64", "studio64", "sublime_text", "notepad++",
        "windowsterminal", "powershell", "pwsh", "cmd", "wezterm-gui", "alacritty", "hyper", "mintty", "nvim-qt", "gvim", "emacs", "eclipse",
    ];
    let e = exe.to_lowercase();
    CODE.contains(&e.strip_suffix(".exe").unwrap_or(&e))
}

/// Tells typing from mouse work: input happened, and the mouse did not move.
pub(crate) struct Keys {
    last_input: u32,
    cursor: (i32, i32),
    pub heat: f32, // 0 idle .. 1 typing away
}

impl Keys {
    pub const fn new() -> Keys {
        Keys { last_input: 0, cursor: (0, 0), heat: 0.0 }
    }

    pub fn look(&mut self, last_input: u32, cursor: (i32, i32), buttons: bool, dt: f32) {
        let typed = last_input != self.last_input && cursor == self.cursor && !buttons;
        (self.last_input, self.cursor) = (last_input, cursor);
        self.heat = if typed { (self.heat + dt * 1.6).min(1.0) } else { (self.heat - dt * 0.22).max(0.0) };
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Turn {
    Working,
    Done,
    Asking,
}

/// What the last line of a Claude Code log says is going on. None = this line does not tell.
pub(crate) fn turn_of(line: &str) -> Option<Turn> {
    // subagents keep their own counsel
    if line.contains("\"isSidechain\":true") {
        return None;
    }
    let kind = |k: &str| line.contains(&format!("\"type\":\"{k}\""));
    if line.contains("\"role\":\"assistant\"") {
        if line.contains("\"name\":\"AskUserQuestion\"") || line.contains("\"name\":\"ExitPlanMode\"") {
            return Some(Turn::Asking);
        }
        if line.contains("\"stop_reason\":\"end_turn\"") {
            return Some(Turn::Done);
        }
        return Some(Turn::Working);
    }
    if line.contains("\"role\":\"user\"") && (kind("user") || kind("tool_result")) {
        return Some(if line.contains("[Request interrupted") { Turn::Done } else { Turn::Working });
    }
    None
}

/// Follows one conversation and says when something worth telling happened.
pub(crate) struct Watcher {
    at: Option<Turn>,
    worked: f32, // seconds
}

/// Only jobs longer than this are worth a cheer.
const LONG: f32 = 15.0;

impl Watcher {
    pub const fn new() -> Watcher {
        Watcher { at: None, worked: 0.0 }
    }

    /// `now` is what the log says; `dt` the seconds since the last look.
    pub fn see(&mut self, now: Option<Turn>, dt: f32) -> Option<Turn> {
        let now = now?;
        let was = self.at.replace(now);
        if was == Some(Turn::Working) {
            self.worked += dt;
        }
        if was == Some(now) || was.is_none() {
            if was.is_none() {
                self.worked = 0.0;
            }
            return None;
        }
        match now {
            Turn::Working => {
                if was == Some(Turn::Done) {
                    self.worked = 0.0;
                }
                None
            }
            Turn::Done => {
                let long = self.worked >= LONG;
                self.worked = 0.0;
                (long && was == Some(Turn::Working)).then_some(Turn::Done)
            }
            Turn::Asking => Some(Turn::Asking),
        }
    }
}

// ---- the log ----

fn newest(dir: &Path, depth: u32, best: &mut Option<(std::time::SystemTime, PathBuf)>) {
    let Ok(list) = std::fs::read_dir(dir) else { return };
    for e in list.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        let p = e.path();
        if meta.is_dir() {
            if depth > 0 && !p.ends_with("subagents") {
                newest(&p, depth - 1, best);
            }
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            if let Ok(t) = meta.modified() {
                if best.as_ref().map_or(true, |(b, _)| t > *b) {
                    *best = Some((t, p));
                }
            }
        }
    }
}

/// The most recent state of a conversation, from the end of its log.
fn state_of(p: &Path) -> Option<Turn> {
    let mut f = std::fs::File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    let from = len.saturating_sub(96 * 1024);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut buf = Vec::new();
    f.take(96 * 1024).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    // the first line may be cut in half; the last may still be being written
    text.lines().skip((from > 0) as usize).filter(|l| l.ends_with('}')).filter_map(turn_of).last()
}

pub(crate) fn spawn() {
    if std::env::var_os("PIXEL_PET_TEST").is_some() {
        return;
    }
    std::thread::spawn(|| {
        let Some(root) = std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".claude").join("projects")) else { return };
        let mut current: Option<PathBuf> = None;
        let mut stamp = None;
        let mut turn = None;
        let mut watch = Watcher::new();
        let mut rescan = 0u32;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if !WANTED.load(Relaxed) || !root.is_dir() {
                WORKING.store(false, Relaxed);
                watch = Watcher::new();
                (current, stamp, turn) = (None, None, None);
                std::thread::sleep(Duration::from_secs(4));
                continue;
            }
            // looking for the newest conversation is the costly part, so not every time
            if rescan == 0 {
                let mut best = None;
                newest(&root, 1, &mut best);
                let found = best.map(|(_, p)| p);
                if found != current {
                    current = found;
                    stamp = None;
                    watch = Watcher::new();
                }
            }
            rescan = (rescan + 1) % 8;
            let Some(p) = &current else { continue };
            let modified = std::fs::metadata(p).and_then(|m| m.modified()).ok();
            if modified != stamp {
                stamp = modified;
                turn = state_of(p);
            }
            let fresh = modified.and_then(|t| t.elapsed().ok()).is_some_and(|e| e < Duration::from_secs(600));
            WORKING.store(turn == Some(Turn::Working) && fresh, Relaxed);
            match watch.see(turn, 2.0) {
                Some(Turn::Done) => FINISHED.fetch_add(1, Relaxed),
                Some(Turn::Asking) => ASKED.fetch_add(1, Relaxed),
                _ => 0,
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editors_and_terminals_count_as_coding() {
        assert!(is_coding("Code.exe") && is_coding("WindowsTerminal.exe") && is_coding("idea64.exe"));
        assert!(!is_coding("chrome.exe") && !is_coding("Spotify.exe") && !is_coding(""));
    }

    #[test]
    fn typing_is_input_without_mouse_movement() {
        let mut k = Keys::new();
        for i in 0..40 {
            k.look(i * 80, (500, 500), false, 0.1);
        }
        assert!(k.heat > 0.9, "typing: {}", k.heat);
        for i in 0..100 {
            k.look(9000, (500, 500), false, 0.1);
            assert!(i > 90 || k.heat >= 0.0);
        }
        assert_eq!(k.heat, 0.0, "stopped");
        for i in 0..40 {
            k.look(10_000 + i * 80, (500 + i as i32, 500), false, 0.1);
        }
        assert_eq!(k.heat, 0.0, "that was the mouse");
        for i in 0..40 {
            k.look(20_000 + i * 80, (540, 500), true, 0.1);
        }
        assert!(k.heat < 0.05, "that was clicking: {}", k.heat);
    }

    #[test]
    fn log_lines_tell_who_is_up() {
        let said = r#"{"type":"assistant","isSidechain":false,"message":{"role":"assistant","content":[{"type":"text","text":"Done."}],"stop_reason":"end_turn"}}"#;
        let tool = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash"}],"stop_reason":"tool_use"}}"#;
        let asks = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"AskUserQuestion"}],"stop_reason":"tool_use"}}"#;
        let user = r#"{"type":"user","message":{"role":"user","content":"please fix it"}}"#;
        let side = r#"{"type":"assistant","isSidechain":true,"message":{"role":"assistant","stop_reason":"end_turn"}}"#;
        assert_eq!(turn_of(said), Some(Turn::Done));
        assert_eq!(turn_of(tool), Some(Turn::Working));
        assert_eq!(turn_of(asks), Some(Turn::Asking));
        assert_eq!(turn_of(user), Some(Turn::Working));
        assert_eq!(turn_of(side), None);
        assert_eq!(turn_of(r#"{"type":"summary","summary":"x"}"#), None);
    }

    #[test]
    fn only_long_jobs_are_cheered_and_questions_always_told() {
        let mut w = Watcher::new();
        assert_eq!(w.see(Some(Turn::Done), 2.0), None, "old news at startup");
        w.see(Some(Turn::Working), 2.0);
        assert_eq!(w.see(Some(Turn::Done), 2.0), None, "a quick answer");
        for _ in 0..10 {
            assert_eq!(w.see(Some(Turn::Working), 2.0), None);
        }
        assert_eq!(w.see(Some(Turn::Done), 2.0), Some(Turn::Done));
        assert_eq!(w.see(Some(Turn::Done), 2.0), None, "told once");
        w.see(Some(Turn::Working), 2.0);
        assert_eq!(w.see(Some(Turn::Asking), 2.0), Some(Turn::Asking));
        assert_eq!(w.see(None, 2.0), None);
    }
}
