//! A text box, standing in for the one Windows has.

use super::gdi::{self, Font};
use super::*;
use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

pub(crate) struct Edit {
    text: Vec<char>, // lines end in \n
    caret: usize,
    anchor: usize, // the other end of the selection
    scroll: usize, // first line shown, or first letter of a single line
    multi: bool,
    right: bool,
    pub readonly: bool,
    limit: usize,
}

const ES_RIGHT: u32 = 2;
const ES_MULTILINE: u32 = 4;

impl Edit {
    pub fn new(style: u32) -> Edit {
        Edit { text: Vec::new(), caret: 0, anchor: 0, scroll: 0, multi: style & ES_MULTILINE != 0, right: style & ES_RIGHT != 0, readonly: false, limit: 30_000 }
    }

    pub fn is_multi(&self) -> bool {
        self.multi
    }

    /// With the line ends Windows uses.
    pub fn text(&self) -> String {
        self.text.iter().collect::<String>().replace('\n', "\r\n")
    }

    pub fn set(&mut self, t: &str) {
        self.text = t.replace("\r\n", "\n").chars().filter(|c| self.multi || *c != '\n').collect();
        (self.caret, self.anchor, self.scroll) = (0, 0, 0);
    }

    pub fn limit(&mut self, n: usize) {
        self.limit = n;
    }

    fn sel(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    /// Positions past the end mean the end; (0, -1) is everything.
    pub fn select(&mut self, a: isize, b: isize) {
        let at = |v: isize| if v < 0 { self.text.len() } else { (v as usize).min(self.text.len()) };
        (self.anchor, self.caret) = (at(a), at(b));
    }

    pub fn selected(&self) -> String {
        let (a, b) = self.sel();
        self.text[a..b].iter().collect()
    }

    /// Put this where the selection is. True if it changed anything.
    pub fn insert(&mut self, t: &str) -> bool {
        let (a, b) = self.sel();
        let new: Vec<char> = t.replace("\r\n", "\n").replace('\r', "\n").chars().filter(|c| (*c == '\n' && self.multi) || *c == '\t' || !c.is_control()).take(self.limit.saturating_sub(self.text.len() - (b - a))).collect();
        if new.is_empty() && a == b {
            return false;
        }
        self.caret = a + new.len();
        self.anchor = self.caret;
        self.text.splice(a..b, new);
        true
    }

    fn erase(&mut self, back: bool, word: bool) -> bool {
        if self.caret == self.anchor {
            let to = if back { if word { self.word(-1) } else { self.caret.saturating_sub(1) } } else if word { self.word(1) } else { (self.caret + 1).min(self.text.len()) };
            self.anchor = to;
        }
        let (a, b) = self.sel();
        self.text.drain(a..b);
        (self.caret, self.anchor) = (a, a);
        a != b
    }

    fn word(&self, dir: i32) -> usize {
        let mut i = self.caret;
        let letter = |c: char| c.is_alphanumeric();
        if dir < 0 {
            while i > 0 && !letter(self.text[i - 1]) {
                i -= 1;
            }
            while i > 0 && letter(self.text[i - 1]) {
                i -= 1;
            }
        } else {
            while i < self.text.len() && letter(self.text[i]) {
                i += 1;
            }
            while i < self.text.len() && !letter(self.text[i]) {
                i += 1;
            }
        }
        i
    }

    fn lines(&self, cols: usize) -> Vec<(usize, usize)> {
        if self.multi { gdi::wrap(&self.text, cols) } else { vec![(0, self.text.len())] }
    }

    /// The line a position is on.
    fn line_of(lines: &[(usize, usize)], at: usize) -> usize {
        lines.iter().rposition(|(a, _)| *a <= at).unwrap_or(0)
    }

    /// A character was typed. True if the text changed.
    pub fn typed(&mut self, c: char) -> bool {
        if self.readonly {
            return false;
        }
        match c {
            '\u{8}' => self.erase(true, false),
            '\u{7f}' => self.erase(true, true),
            '\r' | '\n' => self.multi && self.insert("\n"),
            '\t' => false,
            c if c.is_control() => false,
            c => self.insert(c.encode_utf8(&mut [0; 4])),
        }
    }

    /// A key that does something else than write. (did something, changed the text)
    pub fn key(&mut self, vk: u16, shift: bool, ctrl: bool, cols: usize, rows: usize) -> (bool, bool) {
        let lines = self.lines(cols);
        let row = Self::line_of(&lines, self.caret);
        let col = self.caret - lines[row].0;
        let on = |r: usize, col: usize| (lines[r].0 + col).min(lines[r].1);
        let to = match vk {
            VK_LEFT if ctrl => self.word(-1),
            VK_RIGHT if ctrl => self.word(1),
            VK_LEFT if !shift && self.caret != self.anchor => self.sel().0,
            VK_RIGHT if !shift && self.caret != self.anchor => self.sel().1,
            VK_LEFT => self.caret.saturating_sub(1),
            VK_RIGHT => (self.caret + 1).min(self.text.len()),
            VK_HOME if ctrl => 0,
            VK_END if ctrl => self.text.len(),
            VK_HOME => lines[row].0,
            VK_END => lines[row].1,
            VK_UP if self.multi => if row > 0 { on(row - 1, col) } else { 0 },
            VK_DOWN if self.multi => if row + 1 < lines.len() { on(row + 1, col) } else { self.text.len() },
            VK_PRIOR if self.multi => on(row.saturating_sub(rows), col),
            VK_NEXT if self.multi => on((row + rows).min(lines.len() - 1), col),
            VK_DELETE if !self.readonly => return (true, self.erase(false, ctrl)),
            _ => return (false, false),
        };
        self.caret = to;
        if !shift {
            self.anchor = to;
        }
        (true, false)
    }

    fn grid(f: &Font, w: i32, h: i32) -> (usize, usize) {
        ((w / f.adv()).max(1) as usize, (h / f.line()).max(1) as usize)
    }

    pub fn size(f: &Font, w: i32, h: i32) -> (usize, usize) {
        Self::grid(f, w, h)
    }

    fn left_of(&self, len: usize, f: &Font, w: i32) -> i32 {
        let used = len as i32 * f.adv() + 2;
        if self.right && used < w { w - used } else { 0 }
    }

    /// The position under a point.
    pub fn at(&self, x: i32, y: i32, f: &Font, w: i32, h: i32) -> usize {
        let (cols, _) = Self::grid(f, w, h);
        if !self.multi {
            let shown = self.text.len() - self.scroll.min(self.text.len());
            let col = ((x - self.left_of(shown, f, w) + f.adv() / 2) / f.adv()).max(0) as usize;
            return (self.scroll + col).min(self.text.len());
        }
        let lines = self.lines(cols);
        let row = (self.scroll + (y / f.line()).max(0) as usize).min(lines.len() - 1);
        (lines[row].0 + ((x + f.adv() / 2) / f.adv()).max(0) as usize).min(lines[row].1)
    }

    pub fn click(&mut self, at: usize, keep: bool) {
        self.caret = at;
        if !keep {
            self.anchor = at;
        }
    }

    pub fn wheel(&mut self, up: bool, f: &Font, w: i32, h: i32) {
        let (cols, rows) = Self::grid(f, w, h);
        let most = self.lines(cols).len().saturating_sub(rows);
        self.scroll = if up { self.scroll.saturating_sub(3) } else { (self.scroll + 3).min(most) };
    }

    /// Bring the caret into view.
    pub fn follow(&mut self, f: &Font, w: i32, h: i32) {
        let (cols, rows) = Self::grid(f, w, h);
        if self.multi {
            let lines = self.lines(cols);
            let row = Self::line_of(&lines, self.caret);
            self.scroll = self.scroll.min(row).max((row + 1).saturating_sub(rows)).min(lines.len().saturating_sub(1));
        } else {
            self.scroll = self.scroll.min(self.caret).max((self.caret + 1).saturating_sub(cols));
            if self.text.len() < cols {
                self.scroll = 0;
            }
        }
    }

    pub unsafe fn draw(&self, dc: HDC, f: Font, w: i32, h: i32, focused: bool) {
        let (cols, rows) = Self::grid(&f, w, h);
        let box_ = RECT { left: 0, top: 0, right: w, bottom: h };
        gdi::FillRect(dc, &box_, std::ptr::null_mut());
        let (ink, _, paper) = gdi::colours(dc);
        let (a, b) = self.sel();
        let lines = self.lines(cols);
        let first = if self.multi { self.scroll.min(lines.len() - 1) } else { 0 };
        for (n, (s, e)) in lines.iter().enumerate().skip(first).take(rows) {
            let s = if self.multi { *s } else { self.scroll.min(*e) };
            let y = if self.multi { (n - first) as i32 * f.line() } else { (h - f.line()) / 2 };
            let line = &self.text[s..*e];
            let x = self.left_of(line.len(), &f, w);
            if focused && a < b && a < *e && b > s {
                let (l, r) = (a.max(s) - s, b.min(*e) - s);
                let old = gdi::SetDCBrushColor(dc, 0x8a5a2f);
                gdi::FillRect(dc, &RECT { left: x + l as i32 * f.adv(), top: y, right: x + r as i32 * f.adv(), bottom: y + f.line() }, std::ptr::null_mut());
                let _ = (old, paper);
            }
            gdi::letters(dc, line, x, y, f, box_);
            if focused && !self.readonly && self.caret >= s && self.caret <= *e && Self::line_of(&lines, self.caret) == n {
                let cx = (x + (self.caret - s) as i32 * f.adv()).min(w - 2);
                gdi::SetDCBrushColor(dc, (ink & 0xff) << 16 | ink & 0xff00 | ink >> 16 & 0xff);
                gdi::FillRect(dc, &RECT { left: cx, top: y + 1, right: cx + 2, bottom: y + f.line() - 1 }, std::ptr::null_mut());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_moving_and_erasing() {
        let mut e = Edit::new(0);
        for c in "hello wrld".chars() {
            assert!(e.typed(c));
        }
        for _ in 0..3 {
            e.key(VK_LEFT, false, false, 80, 1);
        }
        e.typed('o');
        assert_eq!(e.text(), "hello world");
        e.key(VK_HOME, false, false, 80, 1);
        e.key(VK_RIGHT, true, true, 80, 1);
        assert_eq!(e.selected(), "hello ");
        assert!(e.typed('\u{8}'));
        assert_eq!(e.text(), "world");
        assert!(!e.typed('\r'), "a single line has no line ends");
        e.select(0, -1);
        e.insert("a\nb");
        assert_eq!(e.text(), "ab");
        e.readonly = true;
        assert!(!e.typed('x') && e.key(VK_DELETE, false, false, 80, 1) == (false, false));
    }

    #[test]
    fn several_lines_use_the_line_ends_of_windows_outside_and_move_up_and_down() {
        let mut e = Edit::new(ES_MULTILINE);
        e.set("one\r\ntwo\r\nthree");
        assert_eq!(e.text(), "one\r\ntwo\r\nthree");
        e.select(5, 5);
        e.key(VK_DOWN, false, false, 40, 4);
        e.typed('X');
        assert_eq!(e.text(), "one\r\ntwo\r\ntXhree");
        e.key(VK_UP, false, false, 40, 4);
        e.key(VK_UP, false, false, 40, 4);
        e.key(VK_END, false, false, 40, 4);
        e.typed('\r');
        assert_eq!(e.text(), "one\r\n\r\ntwo\r\ntXhree");
    }

    #[test]
    fn it_never_holds_more_than_it_may() {
        let mut e = Edit::new(0);
        e.limit(5);
        e.insert("1234567");
        assert_eq!(e.text(), "12345");
        e.select(0, -1);
        e.insert("abcdefgh");
        assert_eq!(e.text(), "abcde");
    }

    #[test]
    fn the_caret_stays_in_view() {
        let mut e = Edit::new(ES_MULTILINE);
        e.set(&"line\n".repeat(50));
        e.select(0x7fff_ffff, 0x7fff_ffff);
        let f = unsafe { gdi::font(std::ptr::null_mut()) };
        e.follow(&f, 200, 100);
        let rows = (100 / f.line()) as usize;
        assert!(e.scroll + rows >= 51 && e.scroll < 51, "{}", e.scroll);
        e.select(0, 0);
        e.follow(&f, 200, 100);
        assert_eq!(e.scroll, 0);
    }
}
