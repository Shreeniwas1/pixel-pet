//! AI coding token usage, read from the logs Claude Code already keeps on this PC.
//!
//! Only the numbers are read (token counts and a timestamp), never the conversation, and nothing
//! leaves the machine. A background thread follows the log files incrementally, so the pet never
//! stalls on a big file.
//!
//! Sources: OpenCode (see `opencode.rs`), Claude Code (`~/.claude/projects`) and Codex CLI (`~/.codex/sessions`, plain or
//! zstd-compressed). How Codex's cumulative `token_count` events turn into per-response deltas
//! (cached tokens are a subset of input, reasoning a subset of output, stale snapshots are
//! skipped) follows the approach of tokscale by Junho Yeo (MIT, https://github.com/junhoyeo/tokscale).
//! The code here is our own compact rewrite; see NOTICE.md for the license text.

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

/// How many days of history the view keeps (the longest range the Stats window offers).
pub const DAYS: usize = 30;
pub const SOURCES: [&str; 3] = ["CLAUDE", "CODEX", "OPENCODE"];

/// What the pet and the stats window show: per source, per day, oldest first, today last.
#[derive(Clone)]
pub struct TokenView {
    pub found: bool,
    pub src: [[Counts; DAYS]; 3],
}

impl TokenView {
    pub const EMPTY: TokenView = TokenView { found: false, src: [[Counts { input: 0, output: 0, cache_write: 0, cache_read: 0 }; DAYS]; 3] };

    /// All sources added up over the last `n` days (today included).
    pub fn sum(&self, n: usize) -> Counts {
        let mut t = Counts::default();
        for s in 0..3 {
            t.add(&self.sum_src(s, n));
        }
        t
    }

    pub fn sum_src(&self, src: usize, n: usize) -> Counts {
        let mut t = Counts::default();
        for c in &self.src[src][DAYS - n.min(DAYS)..] {
            t.add(c);
        }
        t
    }

    /// One total per day over the last `n` days, oldest first.
    pub fn per_day(&self, n: usize, cache: bool) -> Vec<u64> {
        (DAYS - n.min(DAYS)..DAYS)
            .map(|i| {
                let mut t = Counts::default();
                for s in 0..3 {
                    t.add(&self.src[s][i]);
                }
                shown(&t, cache)
            })
            .collect()
    }
}

/// The headline number: fresh tokens, plus cache reads when the user wants them counted.
pub fn shown(c: &Counts, cache: bool) -> u64 {
    c.fresh() + if cache { c.cache_read } else { 0 }
}

pub static TOK: Mutex<TokenView> = Mutex::new(TokenView::EMPTY);

pub fn view_for(t: &Tally, pending: &[Rec], codex: &BTreeMap<i64, Counts>, opencode: &BTreeMap<i64, Counts>, today: i64) -> TokenView {
    let mut claude = t.clone();
    for p in pending {
        claude.add(p);
    }
    let mut v = TokenView { found: true, ..TokenView::EMPTY };
    for (s, days) in [&claude.days, codex, opencode].into_iter().enumerate() {
        for i in 0..DAYS {
            if let Some(c) = days.get(&(today - (DAYS - 1 - i) as i64)) {
                v.src[s][i] = *c;
            }
        }
    }
    v
}

/// Thresholds the pet cheers for (fresh tokens today).
const MILESTONES: [u64; 6] = [1_000_000, 5_000_000, 10_000_000, 25_000_000, 50_000_000, 100_000_000];

/// How many milestones `total` has passed (0 = none yet).
pub fn milestone(total: u64) -> u32 {
    MILESTONES.iter().filter(|m| total >= **m).count() as u32
}

pub fn milestone_text(level: u32) -> String {
    format!("{} tokens today. Busy day!", fmt_tokens(MILESTONES[(level as usize).clamp(1, MILESTONES.len()) - 1]))
}

// ---- Codex ----

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Cum {
    input: u64,
    cached: u64,
    cache_write: u64,
    output: u64,
}

impl Cum {
    fn read(obj: &str) -> Option<Cum> {
        Some(Cum {
            input: num_after(obj, "\"input_tokens\":")?,
            cached: num_after(obj, "\"cached_input_tokens\":").unwrap_or(0),
            cache_write: num_after(obj, "\"cache_write_input_tokens\":").unwrap_or(0),
            output: num_after(obj, "\"output_tokens\":")?,
        })
    }
    fn total(&self) -> u64 {
        self.input + self.output + self.cache_write
    }
    /// `self - prev`, or None when any counter went backwards.
    fn delta(&self, prev: &Cum) -> Option<Cum> {
        (self.input >= prev.input && self.output >= prev.output && self.cached >= prev.cached && self.cache_write >= prev.cache_write).then(|| Cum {
            input: self.input - prev.input,
            cached: self.cached - prev.cached,
            cache_write: self.cache_write - prev.cache_write,
            output: self.output - prev.output,
        })
    }
    /// Codex input includes cached tokens; split them apart. Reasoning is already inside output.
    fn counts(&self) -> Counts {
        let cached = self.cached.min(self.input);
        Counts { input: self.input - cached, output: self.output, cache_write: self.cache_write, cache_read: cached }
    }
}

/// Turn one Codex session's lines into per-day counts. Totals are cumulative, so each
/// event contributes the difference from the one before; a counter that goes backwards is
/// either an out-of-order snapshot (ignored) or a reset (the event's own `last` usage counts).
pub fn parse_codex<'a>(lines: impl Iterator<Item = &'a str>, utc_to_local: i64) -> BTreeMap<i64, Counts> {
    let mut days: BTreeMap<i64, Counts> = BTreeMap::new();
    let mut prev = Cum::default();
    for line in lines {
        if !line.contains("\"token_count\"") {
            continue;
        }
        let Some(t) = line.find("\"total_token_usage\":{") else { continue };
        let Some(total) = Cum::read(&line[t..]) else { continue };
        let Some(ts) = text_after(line, "\"timestamp\":\"", 16) else { continue };
        let (Some(y), Some(mo), Some(d), Some(h), Some(mi)) = (
            ts.get(0..4).and_then(|v| v.parse().ok()),
            ts.get(5..7).and_then(|v| v.parse().ok()),
            ts.get(8..10).and_then(|v| v.parse().ok()),
            ts.get(11..13).and_then(|v| v.parse().ok()),
            ts.get(14..16).and_then(|v| v.parse().ok()),
        ) else {
            continue;
        };
        let day = (todo::minutes_from_civil(y, mo, d, h, mi) + utc_to_local).div_euclid(1440);
        let last = line.find("\"last_token_usage\":{").and_then(|l| Cum::read(&line[l..]));
        let add = match total.delta(&prev) {
            Some(d) => {
                prev = total;
                Some(d)
            }
            None => match last {
                // a little behind the high-water mark: a stale snapshot, count nothing
                Some(l) if prev.total() > 0 && (total.total() * 100 >= prev.total() * 98 || total.total() + 2 * l.total() >= prev.total()) => None,
                // otherwise the session restarted its counters
                Some(l) => {
                    prev = total;
                    Some(l)
                }
                None => None,
            },
        };
        if let Some(a) = add {
            days.entry(day).or_default().add(&a.counts());
        }
    }
    days
}

fn read_codex_file(path: &Path, off: i64) -> Option<BTreeMap<i64, Counts>> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).ok()?;
    let name = path.to_string_lossy();
    let mut lines: Vec<String> = Vec::new();
    // keep only the lines that matter, so a 50 MB session does not sit in memory
    let mut keep = |l: String| {
        if l.contains("\"token_count\"") {
            lines.push(l);
        }
    };
    if name.ends_with(".zst") {
        let d = ruzstd::decoding::StreamingDecoder::new(f).ok()?;
        for l in std::io::BufReader::new(d).lines().map_while(Result::ok) {
            keep(l);
        }
    } else {
        for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
            keep(l);
        }
    }
    Some(parse_codex(lines.iter().map(|s| s.as_str()), off))
}

fn collect_codex(dir: &Path, depth: u32, since: std::time::SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(md) = e.metadata() else { continue };
        if md.is_dir() {
            if depth > 0 {
                collect_codex(&p, depth - 1, since, out);
            }
        } else {
            let n = p.to_string_lossy();
            if (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst")) && md.modified().map_or(false, |m| m >= since) {
                out.push(p);
            }
        }
    }
}

fn codex_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else { return Vec::new() };
    let base = PathBuf::from(home).join(".codex");
    ["sessions", "archived_sessions"].iter().map(|d| base.join(d)).filter(|p| p.is_dir()).collect()
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
        let mut codex_files: HashMap<PathBuf, (u64, BTreeMap<i64, Counts>)> = HashMap::new();
        let mut oc_msgs: HashMap<String, crate::opencode::Msg> = HashMap::new();
        let mut oc_since: i64 = 0;
        let mut round: u32 = 0;
        loop {
            std::thread::sleep(Duration::from_secs(15));
            if !ST.lock().unwrap().tokens_on {
                continue;
            }
            round = round.wrapping_add(1);
            let root = logs_root();
            let croots = codex_roots();
            let ocdbs = crate::opencode::databases();
            if root.is_none() && croots.is_empty() && ocdbs.is_empty() {
                continue;
            }
            let (local_min, _, _) = unsafe { local_now() };
            let utc_min = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| (d.as_secs() / 60) as i64);
            let off = ((local_min - utc_min) as f64 / 30.0).round() as i64 * 30; // whole half hours
            let today = (utc_min + off).div_euclid(1440);
            let since = std::time::SystemTime::now() - Duration::from_secs((DAYS as u64 + 1) * 86400);
            let mut paths = Vec::new();
            if let Some(root) = &root {
                collect(root, 3, since, &mut paths);
            }
            let mut cpaths = Vec::new();
            for r in &croots {
                collect_codex(r, 5, since, &mut cpaths);
            }
            for p in &cpaths {
                let len = std::fs::metadata(p).map_or(0, |m| m.len());
                if codex_files.get(p).map_or(true, |(l, _)| *l != len) {
                    if let Some(d) = read_codex_file(p, off) {
                        codex_files.insert(p.clone(), (len, d));
                    }
                }
            }
            // OpenCode's database can be huge, so ask it only for what changed, and not every round
            if round % 3 == 1 {
                let now_ms = utc_min * 60_000;
                let since = if oc_since == 0 { now_ms - (DAYS as i64 + 1) * 86_400_000 } else { oc_since };
                let mut all_ok = !ocdbs.is_empty();
                for db in &ocdbs {
                    all_ok &= crate::opencode::read_db(db, since, off, &mut oc_msgs);
                }
                if all_ok {
                    oc_since = now_ms - 300_000; // small overlap: rows are rewritten while streaming
                }
            }
            let oc_days = crate::opencode::days_of(&oc_msgs);
            let mut codex: BTreeMap<i64, Counts> = BTreeMap::new();
            for (_, (_, days)) in codex_files.iter().filter(|(p, _)| cpaths.contains(p)) {
                for (d, c) in days {
                    codex.entry(*d).or_default().add(c);
                }
            }
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
            *TOK.lock().unwrap() = view_for(&tally, &pending, &codex, &oc_days, today);
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
        let v = view_for(&t, &[f.pending.clone().unwrap()], &BTreeMap::new(), &BTreeMap::new(), r.day);
        assert_eq!(v.sum(1).output, 682);
    }

    #[test]
    fn periods_sum_the_right_days_and_sources() {
        let mut t = Tally::default();
        let mk = |day, out| Rec { id: format!("m{day}"), day, c: Counts { output: out, ..Default::default() } };
        for (d, o) in [(100, 5), (104, 7), (106, 11), (80, 999), (60, 12345)] {
            t.add(&mk(d, o));
        }
        let mut codex = BTreeMap::new();
        codex.insert(106, Counts { output: 100, cache_read: 1000, ..Default::default() });
        let v = view_for(&t, &[], &codex, &BTreeMap::new(), 106);
        assert_eq!(v.sum(1).output, 111); // today: claude 11 + codex 100
        assert_eq!(v.sum(7).output, 5 + 7 + 11 + 100);
        assert_eq!(v.sum(30).output, 5 + 7 + 11 + 100 + 999);
        assert_eq!(v.sum_src(1, 7).output, 100);
        assert_eq!(v.per_day(7, false), vec![5, 0, 0, 0, 7, 0, 111]);
        // counting cache reads changes the headline
        assert_eq!(*v.per_day(1, true).last().unwrap(), 111 + 1000);
        assert_eq!(shown(&v.sum(1), false), 111);
    }

    #[test]
    fn milestones_step_up() {
        assert_eq!(milestone(999_999), 0);
        assert_eq!(milestone(1_000_000), 1);
        assert_eq!(milestone(30_000_000), 4);
        assert_eq!(milestone(u64::MAX), 6);
        assert_eq!(milestone_text(1), "1.0M tokens today. Busy day!");
    }

    fn codex_line(ts: &str, total: (u64, u64, u64), last: (u64, u64, u64)) -> String {
        format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{},"cached_input_tokens":{},"cache_write_input_tokens":0,"output_tokens":{},"reasoning_output_tokens":5}},"last_token_usage":{{"input_tokens":{},"cached_input_tokens":{},"cache_write_input_tokens":0,"output_tokens":{},"reasoning_output_tokens":2}}}}}}}}"#,
            total.0, total.1, total.2, last.0, last.1, last.2
        )
    }

    #[test]
    fn codex_totals_become_deltas_with_cached_split_out() {
        let l = [
            codex_line("2026-11-07T10:00:00Z", (100, 0, 10), (100, 0, 10)),
            codex_line("2026-11-07T10:01:00Z", (300, 150, 30), (200, 150, 20)),
            codex_line("2026-11-07T10:01:30Z", (300, 150, 30), (200, 150, 20)), // repeated snapshot
        ];
        let d = parse_codex(l.iter().map(|s| s.as_str()), 0);
        let c = d.values().next().unwrap();
        // total input 300 of which 150 cached -> 150 fresh input; output 30
        assert_eq!(*c, Counts { input: 150, output: 30, cache_write: 0, cache_read: 150 });
    }

    #[test]
    fn codex_stale_snapshot_is_ignored_but_a_reset_counts() {
        let l = [
            codex_line("2026-11-07T10:00:00Z", (1000, 0, 100), (1000, 0, 100)),
            codex_line("2026-11-07T10:01:00Z", (990, 0, 100), (50, 0, 5)), // slightly behind: stale
            codex_line("2026-11-07T10:02:00Z", (40, 0, 4), (40, 0, 4)),    // counters restarted
        ];
        let d = parse_codex(l.iter().map(|s| s.as_str()), 0);
        let c = d.values().next().unwrap();
        assert_eq!(c.input, 1000 + 40);
        assert_eq!(c.output, 100 + 4);
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
