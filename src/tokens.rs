//! AI coding token usage, read from the logs Claude Code already keeps on this PC.
//!
//! Only the numbers are read (token counts and a timestamp), never the conversation, and nothing
//! leaves the machine. A background thread follows the log files incrementally, so the pet never
//! stalls on a big file.

use crate::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Counts {
    /// Fresh tokens the model actually processed or produced (cache reads are shown separately).
    pub fn fresh(&self) -> u64 {
        self.input + self.output + self.cache_write
    }
    fn add(&mut self, o: &Counts) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
    }
    fn max(&self, o: &Counts) -> Counts {
        Counts {
            input: self.input.max(o.input),
            output: self.output.max(o.output),
            cache_write: self.cache_write.max(o.cache_write),
            cache_read: self.cache_read.max(o.cache_read),
        }
    }
}

/// One assistant message's usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rec {
    pub id: String,
    pub day: i64, // local day number
    pub c: Counts,
}

fn num_after(s: &str, key: &str) -> Option<u64> {
    let i = s.find(key)? + key.len();
    let digits: String = s[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn text_after<'a>(s: &'a str, key: &str, n: usize) -> Option<&'a str> {
    let i = s.find(key)? + key.len();
    s.get(i..i + n)
}

/// Parse one log line. `utc_to_local` is the local offset from UTC in minutes.
pub fn parse_line(line: &str, utc_to_local: i64) -> Option<Rec> {
    let u = line.find("\"usage\":{")?;
    let usage = &line[u..];
    let ts = text_after(line, "\"timestamp\":\"", 16)?; // 2026-11-07T10:15
    let (y, mo, d) = (ts.get(0..4)?.parse().ok()?, ts.get(5..7)?.parse().ok()?, ts.get(8..10)?.parse().ok()?);
    let (h, mi) = (ts.get(11..13)?.parse().ok()?, ts.get(14..16)?.parse().ok()?);
    let utc_min = todo::minutes_from_civil(y, mo, d, h, mi);
    let id_at = line.find("\"id\":\"msg_")? + 6;
    let id: String = line[id_at..].chars().take_while(|c| *c != '"').collect();
    let c = Counts {
        input: num_after(usage, "\"input_tokens\":")?,
        output: num_after(usage, "\"output_tokens\":")?,
        cache_write: num_after(usage, "\"cache_creation_input_tokens\":").unwrap_or(0),
        cache_read: num_after(usage, "\"cache_read_input_tokens\":").unwrap_or(0),
    };
    Some(Rec { id, day: (utc_min + utc_to_local).div_euclid(1440), c })
}

/// Local days to totals. A message is logged once per content block with the same usage, so
/// consecutive lines with one id count once.
#[derive(Default, Clone)]
pub struct Tally {
    pub days: BTreeMap<i64, Counts>,
}

impl Tally {
    fn add(&mut self, r: &Rec) {
        self.days.entry(r.day).or_default().add(&r.c);
    }
}

#[derive(Default)]
pub struct FileScan {
    pub offset: u64,
    pending: Option<Rec>,
}

impl FileScan {
    pub fn feed(&mut self, rec: Rec, t: &mut Tally) {
        match &mut self.pending {
            Some(p) if p.id == rec.id => p.c = p.c.max(&rec.c),
            _ => {
                if let Some(p) = self.pending.take() {
                    t.add(&p);
                }
                self.pending = Some(rec);
            }
        }
    }
}

/// What the pet and the stats window show.
#[derive(Clone, Default)]
pub struct TokenView {
    pub found: bool,
    pub today: Counts,
    pub week: Counts,
    pub per_day: [u64; 7], // fresh tokens, oldest first, today last
}

pub static TOK: Mutex<TokenView> = Mutex::new(TokenView {
    found: false,
    today: Counts { input: 0, output: 0, cache_write: 0, cache_read: 0 },
    week: Counts { input: 0, output: 0, cache_write: 0, cache_read: 0 },
    per_day: [0; 7],
});

pub fn view_for(t: &Tally, pending: &[Rec], today: i64) -> TokenView {
    let mut all = t.clone();
    for p in pending {
        all.add(p);
    }
    let mut v = TokenView { found: true, ..Default::default() };
    for i in 0..7 {
        let day = today - 6 + i as i64;
        if let Some(c) = all.days.get(&day) {
            v.per_day[i] = c.fresh();
            v.week.add(c);
            if day == today {
                v.today = *c;
            }
        }
    }
    v
}

/// 1234 -> "1.2K", 3_400_000 -> "3.4M"
pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1e9)
    } else if n >= 10_000_000 {
        format!("{:.0}M", n as f64 / 1e6)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 10_000 {
        format!("{:.0}K", n as f64 / 1e3)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

// ---- finding and following the files ----

fn logs_root() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let p = PathBuf::from(home).join(".claude").join("projects");
    p.is_dir().then_some(p)
}

fn collect(dir: &Path, depth: u32, since: std::time::SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() {
            if depth > 0 {
                collect(&p, depth - 1, since, out);
            }
        } else if p.extension().map_or(false, |x| x == "jsonl") && md.modified().map_or(false, |m| m >= since) {
            out.push(p);
        }
    }
}

/// Read what was appended to a file since last time (whole lines only).
fn read_new(path: &Path, fs: &mut FileScan, tally: &mut Tally, off: i64) -> Option<bool> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < fs.offset {
        return Some(true); // file shrank: start over
    }
    if len == fs.offset {
        return Some(false);
    }
    f.seek(SeekFrom::Start(fs.offset)).ok()?;
    let mut buf = Vec::with_capacity((len - fs.offset).min(64 << 20) as usize);
    f.take(64 << 20).read_to_end(&mut buf).ok()?;
    let end = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    for line in String::from_utf8_lossy(&buf[..end]).lines() {
        if let Some(r) = parse_line(line, off) {
            fs.feed(r, tally);
        }
    }
    fs.offset += end as u64;
    Some(false)
}

/// Start the background follower. It idles (one cheap check every few seconds) while the
/// option is off or Claude Code has never run here.
pub fn spawn() {
    std::thread::spawn(|| {
        let mut files: HashMap<PathBuf, FileScan> = HashMap::new();
        let mut tally = Tally::default();
        loop {
            std::thread::sleep(Duration::from_secs(8));
            if !ST.lock().unwrap().tokens_on {
                continue;
            }
            let Some(root) = logs_root() else { continue };
            let (local_min, _, _) = unsafe { local_now() };
            let utc_min = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| (d.as_secs() / 60) as i64);
            let off = ((local_min - utc_min) as f64 / 30.0).round() as i64 * 30; // whole half hours
            let today = (utc_min + off).div_euclid(1440);
            let since = std::time::SystemTime::now() - Duration::from_secs(8 * 86400);
            let mut paths = Vec::new();
            collect(&root, 3, since, &mut paths);
            let mut restart = false;
            for p in &paths {
                let fs = files.entry(p.clone()).or_default();
                if read_new(p, fs, &mut tally, off) == Some(true) {
                    restart = true;
                }
            }
            if restart {
                files.clear();
                tally = Tally::default();
                continue; // rebuilt from scratch on the next round
            }
            let pending: Vec<Rec> = files.values().filter_map(|f| f.pending.clone()).collect();
            *TOK.lock().unwrap() = view_for(&tally, &pending, today);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"parentUuid":"x","message":{"model":"m","id":"msg_01ABC","type":"message","role":"assistant","content":[],"usage":{"input_tokens":2,"cache_creation_input_tokens":24326,"cache_read_input_tokens":45470,"output_tokens":341,"output_tokens_details":{"thinking_tokens":29},"iterations":[{"input_tokens":9,"output_tokens":999}]}},"timestamp":"2026-11-07T10:15:30.123Z"}"#;

    #[test]
    fn parses_usage_from_a_log_line() {
        let r = parse_line(LINE, 0).unwrap();
        assert_eq!(r.id, "msg_01ABC");
        assert_eq!(r.c, Counts { input: 2, output: 341, cache_write: 24326, cache_read: 45470 });
        assert_eq!(r.c.fresh(), 24669);
    }

    #[test]
    fn local_offset_can_move_the_day() {
        let utc = parse_line(LINE, 0).unwrap().day;
        assert_eq!(parse_line(LINE, 14 * 60).unwrap().day, utc + 1); // 10:15 + 14h is past midnight
    }

    #[test]
    fn ignores_lines_without_usage() {
        assert!(parse_line(r#"{"type":"user","timestamp":"2026-11-07T10:15:30Z"}"#, 0).is_none());
    }

    #[test]
    fn one_message_logged_in_several_blocks_counts_once() {
        let mut t = Tally::default();
        let mut f = FileScan::default();
        let r = parse_line(LINE, 0).unwrap();
        f.feed(r.clone(), &mut t);
        f.feed(r.clone(), &mut t);
        let mut next = r.clone();
        next.id = "msg_02".into();
        f.feed(next, &mut t);
        let v = view_for(&t, &[f.pending.clone().unwrap()], r.day);
        assert_eq!(v.today.output, 682);
    }

    #[test]
    fn week_window_and_per_day() {
        let mut t = Tally::default();
        let mk = |day, out| Rec { id: format!("m{day}"), day, c: Counts { output: out, ..Default::default() } };
        for (d, o) in [(100, 5), (104, 7), (106, 11), (90, 999)] {
            t.add(&mk(d, o));
        }
        let v = view_for(&t, &[], 106);
        assert_eq!(v.per_day, [5, 0, 0, 0, 7, 0, 11]);
        assert_eq!(v.today.output, 11);
        assert_eq!(v.week.output, 23);
    }

    #[test]
    fn formats_big_numbers() {
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(1234), "1.2K");
        assert_eq!(fmt_tokens(48_000), "48K");
        assert_eq!(fmt_tokens(3_400_000), "3.4M");
        assert_eq!(fmt_tokens(120_000_000), "120M");
    }
}
