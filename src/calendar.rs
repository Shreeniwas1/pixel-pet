//! Calendar reminders. Reads calendars in the iCalendar format (`.ics`): files, or the private link
//! that Google Calendar, Outlook and others give out. Only those links are asked, and only because
//! the user added them; what is in a calendar never leaves the PC.

use crate::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32};

/// Minutes before, 0 = never.
pub(crate) const BEFORE: [u32; 5] = [0, 5, 10, 15, 30];
pub(crate) const USUAL: usize = 2;
pub(crate) const MOST: usize = 4;
const BIGGEST: u64 = 12 << 20;
/// Links are asked this often, in seconds; files are looked at every time.
const EVERY: u64 = 15 * 60;

// ---- days and seconds (pure) ----

const DAY: i64 = 86400;

fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}

fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, doy - (153 * mp + 2) / 5 + 1)
}

/// 0 = Monday.
fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

fn month_len(y: i64, m: i64) -> i64 {
    if m == 12 { 31 } else { days(y, m + 1, 1) - days(y, m, 1) }
}

/// The n-th (from the end if negative) of a weekday in a month.
fn nth(y: i64, m: i64, n: i64, wd: i64) -> Option<i64> {
    let (first, len) = (days(y, m, 1), month_len(y, m));
    let d = if n > 0 {
        first + (wd - weekday(first)).rem_euclid(7) + (n - 1) * 7
    } else {
        let last = first + len - 1;
        last - (weekday(last) - wd).rem_euclid(7) + (n + 1) * 7
    };
    (first..first + len).contains(&d).then_some(d)
}

// ---- reading (pure) ----

/// A moment as it is written: on somebody's clock.
#[derive(Clone, Debug, PartialEq)]
struct When {
    wall: i64, // seconds on that clock
    utc: bool,
    zone: Option<String>,
    all_day: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, PartialEq)]
struct Rule {
    freq: Freq,
    every: i64,
    count: Option<u32>,
    until: Option<When>,
    by_day: Vec<(i64, i64)>, // which one (0 = each), weekday
    by_month_day: Vec<i64>,
    by_month: Vec<i64>,
    set_pos: Vec<i64>,
    week_start: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct Event {
    uid: String,
    title: String,
    start: When,
    rule: Option<Rule>,
    except: Vec<When>,
    also: Vec<When>,
    instead_of: Option<When>,
    cancelled: bool,
}

/// One change of a clock's offset.
#[derive(Clone, Debug)]
struct Shift {
    to: i64,
    start: i64,
    rule: Option<Rule>,
}

type Zones = HashMap<String, Vec<Shift>>;

#[derive(Default)]
pub(crate) struct Calendar {
    events: Vec<Event>,
    zones: Zones,
}

/// Lines that were folded, together again.
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        match (l.strip_prefix([' ', '\t']), out.last_mut()) {
            (Some(more), Some(last)) => last.push_str(more),
            _ => out.push(l.to_string()),
        }
    }
    out
}

/// NAME;PARAM=x;PARAM="y:z":value
fn property(line: &str) -> Option<(String, Vec<(String, String)>, &str)> {
    let mut quoted = false;
    let colon = line.char_indices().find(|(_, c)| {
        quoted ^= *c == '"';
        *c == ':' && !quoted
    })?;
    let mut parts = Vec::new();
    let (mut from, mut quoted) = (0, false);
    let head = &line[..colon.0];
    for (i, c) in head.char_indices() {
        quoted ^= c == '"';
        if c == ';' && !quoted {
            parts.push(&head[from..i]);
            from = i + 1;
        }
    }
    parts.push(&head[from..]);
    let params = parts[1..].iter().filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_uppercase(), v.trim_matches('"').to_string())).collect();
    Some((parts[0].to_uppercase(), params, &line[colon.0 + 1..]))
}

fn plain(v: &str) -> String {
    let mut o = String::new();
    let mut it = v.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.next() {
                Some('n' | 'N') => o.push(' '),
                Some(c) => o.push(c),
                None => {}
            },
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o.trim().chars().take(120).collect()
}

fn digits(s: &str, a: usize, b: usize) -> Option<i64> {
    let t = s.get(a..b)?;
    t.bytes().all(|c| c.is_ascii_digit()).then(|| t.parse().ok())?
}

fn when(v: &str, params: &[(String, String)]) -> Option<When> {
    let v = v.trim();
    let (y, m, d) = (digits(v, 0, 4)?, digits(v, 4, 6)?, digits(v, 6, 8)?);
    if !(1900..2300).contains(&y) || !(1..=12).contains(&m) || !(1..=month_len(y, m)).contains(&d) {
        return None;
    }
    let day = days(y, m, d) * DAY;
    if v.len() == 8 {
        return Some(When { wall: day, utc: false, zone: None, all_day: true });
    }
    if v.as_bytes().get(8) != Some(&b'T') {
        return None;
    }
    let (h, mi, s) = (digits(v, 9, 11)?, digits(v, 11, 13)?, digits(v, 13, 15).unwrap_or(0));
    if h > 23 || mi > 59 {
        return None;
    }
    let zone = params.iter().find(|(k, _)| k == "TZID").map(|(_, z)| z.clone());
    Some(When { wall: day + h * 3600 + mi * 60 + s.min(59), utc: v.ends_with('Z'), zone, all_day: false })
}

fn rule(v: &str) -> Option<Rule> {
    let mut r = Rule { freq: Freq::Daily, every: 1, count: None, until: None, by_day: Vec::new(), by_month_day: Vec::new(), by_month: Vec::new(), set_pos: Vec::new(), week_start: 0 };
    let mut known = false;
    let wd = |t: &str| ["MO", "TU", "WE", "TH", "FR", "SA", "SU"].iter().position(|d| *d == t).map(|i| i as i64);
    let nums = |t: &str| t.split(',').filter_map(|n| n.trim().parse::<i64>().ok()).collect::<Vec<_>>();
    for part in v.split(';') {
        let Some((k, val)) = part.split_once('=') else { continue };
        match k.trim().to_uppercase().as_str() {
            "FREQ" => {
                r.freq = match val {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => return None,
                };
                known = true;
            }
            "INTERVAL" => r.every = val.parse::<i64>().ok()?.clamp(1, 1000),
            "COUNT" => r.count = val.parse().ok(),
            "UNTIL" => r.until = when(val, &[]),
            "BYDAY" => {
                for d in val.split(',') {
                    let d = d.trim();
                    let Some(cut) = d.len().checked_sub(2).filter(|c| d.is_char_boundary(*c)) else { continue };
                    if let Some(w) = wd(&d[cut..]) {
                        r.by_day.push((d[..cut].parse().unwrap_or(0), w));
                    }
                }
            }
            "BYMONTHDAY" => r.by_month_day = nums(val),
            "BYMONTH" => r.by_month = nums(val),
            "BYSETPOS" => r.set_pos = nums(val),
            "WKST" => r.week_start = wd(val).unwrap_or(0),
            _ => {}
        }
    }
    known.then_some(r)
}

/// "+0530" in seconds.
fn offset(v: &str) -> Option<i64> {
    let v = v.trim();
    let sign = if v.starts_with('-') { -1 } else { 1 };
    let d = v.trim_start_matches(['+', '-']);
    Some(sign * (digits(d, 0, 2)? * 3600 + digits(d, 2, 4)? * 60))
}

pub(crate) fn read(text: &str) -> Option<Calendar> {
    if !text.trim_start_matches('\u{feff}').trim_start().starts_with("BEGIN:VCALENDAR") {
        return None;
    }
    let mut c = Calendar::default();
    let mut inside: Vec<String> = Vec::new();
    let mut ev: Option<Event> = None;
    let mut zone = String::new();
    let mut shift: Option<(Option<i64>, Option<i64>, Option<Rule>)> = None;
    for line in unfold(text) {
        let Some((name, params, value)) = property(&line) else { continue };
        let at = inside.last().map_or("", |s| s.as_str());
        match (name.as_str(), at) {
            ("BEGIN", _) => {
                match value.trim() {
                    "VEVENT" if at == "VCALENDAR" => ev = Some(Event { uid: String::new(), title: String::new(), start: When { wall: i64::MIN, utc: true, zone: None, all_day: false }, rule: None, except: Vec::new(), also: Vec::new(), instead_of: None, cancelled: false }),
                    "VTIMEZONE" => zone.clear(),
                    "STANDARD" | "DAYLIGHT" if at == "VTIMEZONE" => shift = Some((None, None, None)),
                    _ => {}
                }
                if inside.len() > 8 {
                    return None;
                }
                inside.push(value.trim().to_string());
            }
            ("END", _) => {
                inside.pop();
                match value.trim() {
                    "VEVENT" => {
                        if let Some(e) = ev.take().filter(|e| e.start.wall != i64::MIN) {
                            c.events.push(e);
                        }
                    }
                    "STANDARD" | "DAYLIGHT" => {
                        if let Some((Some(to), Some(start), rule)) = shift.take() {
                            c.zones.entry(zone.clone()).or_default().push(Shift { to, start, rule });
                        }
                    }
                    _ => {}
                }
                if c.events.len() > 20_000 {
                    break;
                }
            }
            ("TZID", "VTIMEZONE") => zone = value.trim().to_string(),
            (_, "STANDARD" | "DAYLIGHT") => {
                if let Some(s) = shift.as_mut() {
                    match name.as_str() {
                        "TZOFFSETTO" => s.0 = offset(value),
                        "DTSTART" => s.1 = when(value, &[]).map(|w| w.wall),
                        "RRULE" => s.2 = rule(value),
                        _ => {}
                    }
                }
            }
            (_, "VEVENT") => {
                let Some(e) = ev.as_mut() else { continue };
                match name.as_str() {
                    "UID" => e.uid = value.trim().to_string(),
                    "SUMMARY" => e.title = plain(value),
                    "DTSTART" => {
                        if let Some(w) = when(value, &params) {
                            e.start = w;
                        }
                    }
                    "RRULE" => e.rule = rule(value),
                    "EXDATE" => e.except.extend(value.split(',').filter_map(|v| when(v, &params))),
                    "RDATE" => e.also.extend(value.split(',').filter_map(|v| when(v, &params))),
                    "RECURRENCE-ID" => e.instead_of = when(value, &params),
                    "STATUS" => e.cancelled = value.trim().eq_ignore_ascii_case("CANCELLED"),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Some(c)
}

// ---- when things happen (pure) ----

impl Rule {
    /// Does this rule fall on a day, for something that first was on day `first`? (Not how often yet.)
    fn on(&self, first: i64, day: i64) -> bool {
        if day < first {
            return false;
        }
        let ((y0, m0, d0), (y, m, d)) = (civil(first), civil(day));
        if !self.by_month.is_empty() && !self.by_month.contains(&m) {
            return false;
        }
        let wd = weekday(day);
        let named = |each: bool| self.by_day.iter().any(|(n, w)| *w == wd && (each || *n == 0 || nth(y, m, *n, *w) == Some(day)));
        let month_day = |list: &[i64]| list.iter().any(|n| *n == d || (*n < 0 && month_len(y, m) + 1 + n == d));
        match self.freq {
            Freq::Daily => (day - first) % self.every == 0 && (self.by_day.is_empty() || named(true)) && (self.by_month_day.is_empty() || month_day(&self.by_month_day)),
            Freq::Weekly => {
                let week = |x: i64| x - (weekday(x) - self.week_start).rem_euclid(7);
                (week(day) - week(first)) / 7 % self.every == 0 && if self.by_day.is_empty() { wd == weekday(first) } else { named(true) }
            }
            Freq::Monthly => ((y - y0) * 12 + m - m0) % self.every == 0 && self.in_month(y, m, d0, day),
            Freq::Yearly => {
                if (y - y0) % self.every != 0 {
                    return false;
                }
                if self.by_month.is_empty() && self.by_day.is_empty() && self.by_month_day.is_empty() {
                    return (m, d) == (m0, d0);
                }
                (!self.by_month.is_empty() || m == m0) && self.in_month(y, m, d0, day)
            }
        }
    }

    /// One of the days a monthly (or yearly) rule picks in this month?
    fn in_month(&self, y: i64, m: i64, d0: i64, day: i64) -> bool {
        let first = days(y, m, 1);
        let len = month_len(y, m);
        let picked: Vec<i64> = (first..first + len)
            .filter(|x| {
                let d = x - first + 1;
                let by_day = self.by_day.is_empty() || self.by_day.iter().any(|(n, w)| *w == weekday(*x) && (*n == 0 || nth(y, m, *n, *w) == Some(*x)));
                let by_num = self.by_month_day.is_empty() || self.by_month_day.iter().any(|n| *n == d || (*n < 0 && len + 1 + n == d));
                let plain = !self.by_day.is_empty() || !self.by_month_day.is_empty() || d == d0;
                by_day && by_num && plain
            })
            .collect();
        match self.set_pos.is_empty() {
            true => picked.contains(&day),
            false => self.set_pos.iter().any(|p| {
                let i = if *p > 0 { p - 1 } else { picked.len() as i64 + p };
                usize::try_from(i).ok().and_then(|i| picked.get(i)) == Some(&day)
            }),
        }
    }
}

impl Calendar {
    /// What a clock shows ahead of UTC at a moment it shows itself.
    fn ahead(&self, zone: &str, wall: i64) -> Option<i64> {
        let shifts = self.zones.get(zone)?;
        let mut best: Option<(i64, i64)> = None;
        for s in shifts {
            let mut at = (s.start <= wall).then_some(s.start);
            if let (Some(r), true) = (&s.rule, s.start <= wall) {
                // the last time it changed: this year, or the one before
                let (y, _, _) = civil(wall.div_euclid(DAY));
                'years: for y in [y, y - 1] {
                    let (a, b) = (days(y, 1, 1), days(y + 1, 1, 1));
                    for d in (a..b).rev() {
                        let t = d * DAY + s.start.rem_euclid(DAY);
                        if t <= wall && t >= s.start && r.on(s.start.div_euclid(DAY), d) && r.until.as_ref().map_or(true, |u| t <= u.wall + DAY) {
                            at = Some(t);
                            break 'years;
                        }
                    }
                }
            }
            if let Some(t) = at.filter(|t| best.map_or(true, |b| *t > b.0)) {
                best = Some((t, s.to));
            }
        }
        best.map(|b| b.1).or_else(|| shifts.first().map(|s| s.to))
    }

    /// In UTC. `here` is how far this PC's clock is ahead.
    fn utc(&self, w: &When, here: i64) -> i64 {
        if w.utc {
            return w.wall;
        }
        let ahead = w.zone.as_deref().and_then(|z| self.ahead(z, w.wall)).unwrap_or(here);
        w.wall - ahead
    }

    /// Everything that starts between two moments, in order. All-day things are left out: there is
    /// nothing to be on time for.
    pub fn between(&self, from: i64, to: i64, here: i64) -> Vec<(i64, String, String)> {
        let mut out = Vec::new();
        // meetings of a series that were moved or cancelled
        let moved: HashSet<(&str, i64)> = self.events.iter().filter_map(|e| e.instead_of.as_ref().map(|w| (e.uid.as_str(), self.utc(w, here)))).collect();
        for e in &self.events {
            if e.start.all_day || e.cancelled {
                continue;
            }
            let mut add = |t: i64| {
                if (from..to).contains(&t) {
                    out.push((t, e.title.clone(), e.uid.clone()));
                }
            };
            if e.instead_of.is_some() {
                add(self.utc(&e.start, here));
                continue;
            }
            let except: Vec<i64> = e.except.iter().map(|w| self.utc(w, here)).collect();
            let whole: Vec<i64> = e.except.iter().filter(|w| w.all_day).map(|w| w.wall.div_euclid(DAY)).collect();
            let gone = |t: i64, day: i64| except.contains(&t) || whole.contains(&day) || moved.contains(&(e.uid.as_str(), t));
            let Some(r) = &e.rule else {
                let t = self.utc(&e.start, here);
                if !gone(t, e.start.wall.div_euclid(DAY)) {
                    add(t);
                }
                for w in &e.also {
                    add(self.utc(w, here));
                }
                continue;
            };
            let (first, time) = (e.start.wall.div_euclid(DAY), e.start.wall.rem_euclid(DAY));
            let (a, b) = (from.div_euclid(DAY) - 2, to.div_euclid(DAY) + 2);
            // how many there were before only matters if there is a last one
            let mut left = r.count.map(i64::from);
            if let Some(n) = left.as_mut() {
                let mut d = first;
                while d < a && *n > 0 && d < first + 366 * 60 {
                    *n -= r.on(first, d) as i64;
                    d += 1;
                }
            }
            for d in a.max(first)..b {
                if !(r.on(first, d) || d == first) {
                    continue;
                }
                if let Some(n) = left.as_mut() {
                    if *n <= 0 {
                        break;
                    }
                    *n -= 1;
                }
                let w = When { wall: d * DAY + time, ..e.start.clone() };
                let t = self.utc(&w, here);
                let over = r.until.as_ref().is_some_and(|u| if u.all_day { d > u.wall.div_euclid(DAY) } else { t > if u.utc { u.wall } else { self.utc(&When { zone: e.start.zone.clone(), ..u.clone() }, here) } });
                if !over && !gone(t, d) {
                    add(t);
                }
            }
            for w in &e.also {
                add(self.utc(w, here));
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// "IN 25 MIN", "NOW", "TOMORROW".
pub(crate) fn until(t: i64, now: i64, here: i64) -> String {
    let m = (t - now + 59).div_euclid(60);
    let (d, today) = ((t + here).div_euclid(DAY), (now + here).div_euclid(DAY));
    match m {
        ..=0 => "NOW".into(),
        1..=90 => format!("IN {m} MIN"),
        _ if d == today => format!("IN {} H", (m + 30) / 60),
        _ => "TOMORROW".into(),
    }
}

/// "09:30" on this PC's clock.
pub(crate) fn clock(t: i64, here: i64) -> String {
    let s = (t + here).rem_euclid(DAY);
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// A link as it is asked: some programs write webcal://.
pub(crate) fn link(t: &str) -> Option<String> {
    let t = t.trim();
    let t = t.strip_prefix("webcal://").map_or_else(|| t.to_string(), |r| format!("https://{r}"));
    (t.starts_with("https://") && net::fine(&t)).then_some(t)
}

/// What to call a source. A private link is as good as a key, so only where it leads is shown.
pub(crate) fn source_name(s: &str) -> String {
    match s.strip_prefix("https://") {
        Some(r) => r.split('/').next().unwrap_or("").trim_start_matches("www.").to_string(),
        None => std::path::Path::new(s).file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned()),
    }
}

// ---- keeping up ----

/// A calendar the user added.
struct Source {
    from: String,
    read: Option<Calendar>,
    bad: bool,
    asked: Option<Instant>,
    stamp: Option<std::time::SystemTime>,
}

static SOURCES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static BAD: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// (when, what, which) of the next two days.
static NEXT: Mutex<Vec<(i64, String, String)>> = Mutex::new(Vec::new());
static CHANGED: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Hurry up: something was added.
static TURN: AtomicU32 = AtomicU32::new(0);

pub(crate) fn utc_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// How far this PC's clock is ahead of UTC, in seconds.
pub(crate) fn ahead_here() -> i64 {
    let wall = unsafe { local_now().0 };
    // to the nearest quarter of an hour, as no clock is set otherwise
    let d = wall * 60 - utc_now().div_euclid(60) * 60;
    (d + 450).div_euclid(900) * 900
}

/// At startup, and when the list changed.
pub(crate) fn begin() {
    if friend::is_friend() {
        return;
    }
    let list: Vec<String> = secret::get("calendar").map_or(Vec::new(), |t| t.lines().filter(|l| !l.is_empty()).take(MOST).map(str::to_string).collect());
    let none = list.is_empty();
    *SOURCES.lock().unwrap() = list;
    if !none && !RUNNING.swap(true, Relaxed) {
        std::thread::spawn(keep_up);
    }
}

pub(crate) fn sources() -> Vec<(String, bool)> {
    let bad = BAD.lock().unwrap().clone();
    SOURCES.lock().unwrap().iter().map(|s| (source_name(s), bad.contains(s))).collect()
}

/// False if it is there already, there are too many, or it could not be kept.
pub(crate) fn add(from: &str) -> bool {
    let list = {
        let mut l = SOURCES.lock().unwrap();
        if l.len() >= MOST || l.iter().any(|s| s == from) || from.contains(['\n', '\r']) {
            return false;
        }
        l.push(from.to_string());
        l.join("\n")
    };
    let ok = secret::set("calendar", &list);
    CHANGED.store(true, Relaxed);
    TURN.fetch_add(1, Relaxed);
    if ok && !RUNNING.swap(true, Relaxed) {
        std::thread::spawn(keep_up);
    }
    ok
}

pub(crate) fn remove(i: usize) {
    let list = {
        let mut l = SOURCES.lock().unwrap();
        if i < l.len() {
            l.remove(i);
        }
        l.join("\n")
    };
    secret::set("calendar", &list);
    CHANGED.store(true, Relaxed);
    TURN.fetch_add(1, Relaxed);
}

fn fetch(s: &mut Source) {
    if s.from.starts_with("https://") {
        if s.asked.is_some_and(|t| t.elapsed().as_secs() < EVERY) {
            return;
        }
        s.asked = Some(Instant::now());
        let got = net::ask(&net::Ask { url: s.from.clone(), follow: true, most: BIGGEST, secs: 40, ..Default::default() });
        match got.ok().filter(|a| a.code == 200).and_then(|a| read(&a.text())) {
            Some(c) => (s.read, s.bad) = (Some(c), false),
            None => s.bad = s.read.is_none(), // what it knows is better than nothing
        }
        return;
    }
    let p = std::path::Path::new(&s.from);
    let meta = std::fs::metadata(p).ok().filter(|m| m.len() <= BIGGEST);
    let stamp = meta.and_then(|m| m.modified().ok());
    if stamp.is_some() && stamp == s.stamp {
        return;
    }
    s.stamp = stamp;
    match std::fs::read(p).ok().filter(|_| stamp.is_some()).and_then(|b| read(&String::from_utf8_lossy(&b))) {
        Some(c) => (s.read, s.bad) = (Some(c), false),
        None => s.bad = true,
    }
}

fn keep_up() {
    let mut mine: Vec<Source> = Vec::new();
    loop {
        let turn = TURN.load(Relaxed);
        let want = SOURCES.lock().unwrap().clone();
        mine.retain(|s| want.contains(&s.from));
        for w in want {
            if !mine.iter().any(|s| s.from == w) {
                mine.push(Source { from: w, read: None, bad: false, asked: None, stamp: None });
            }
        }
        for s in &mut mine {
            fetch(s);
        }
        let (now, here) = (utc_now(), ahead_here());
        let mut next: Vec<_> = mine.iter().filter_map(|s| s.read.as_ref()).flat_map(|c| c.between(now - 3600, now + 2 * DAY, here)).collect();
        next.sort();
        next.dedup();
        *NEXT.lock().unwrap() = next;
        *BAD.lock().unwrap() = mine.iter().filter(|s| s.bad).map(|s| s.from.clone()).collect();
        for _ in 0..60 {
            std::thread::sleep(Duration::from_secs(1));
            if TURN.load(Relaxed) != turn {
                break;
            }
        }
    }
}

/// What comes next, soonest first.
pub(crate) fn coming(most: usize) -> Vec<(i64, String)> {
    let now = utc_now();
    NEXT.lock().unwrap().iter().filter(|e| e.0 > now - 300).take(most).map(|(t, w, _)| (*t, if w.is_empty() { "Something".into() } else { w.clone() })).collect()
}

/// What has been said already: (which, when, at the start?)
static SAID: Mutex<Vec<(String, i64, bool)>> = Mutex::new(Vec::new());

/// Something to say now: `before` minutes ahead, and when it starts.
pub(crate) fn due(before: u32) -> Option<String> {
    if before == 0 || !RUNNING.load(Relaxed) {
        return None;
    }
    let (now, here) = (utc_now(), ahead_here());
    let next = NEXT.lock().unwrap();
    let mut said = SAID.lock().unwrap();
    said.retain(|s| s.1 > now - 7200);
    pick(&next, &mut said, now, here, before)
}

fn pick(next: &[(i64, String, String)], said: &mut Vec<(String, i64, bool)>, now: i64, here: i64, before: u32) -> Option<String> {
    for (t, title, uid) in next {
        let title = if title.is_empty() { "Something" } else { title.as_str() };
        let short: String = title.chars().take(52).collect();
        let key = |start: bool| (uid.clone(), *t, start);
        if (t - now) <= 0 && now - t < 120 && !said.contains(&key(true)) {
            said.push(key(true));
            return Some(format!("{short} starts now!"));
        }
        if t - now > 0 && t - now <= before as i64 * 60 && !said.contains(&key(false)) {
            said.push(key(false));
            let m = (t - now + 59) / 60;
            return Some(format!("{short} in {m} minute{} ({}).", if m == 1 { "" } else { "s" }, clock(*t, here)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(y: i64, m: i64, d: i64, h: i64, mi: i64) -> i64 {
        days(y, m, d) * DAY + h * 3600 + mi * 60
    }

    fn cal(events: &str) -> Calendar {
        read(&format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VTIMEZONE\r\nTZID:Berlin\r\nBEGIN:DAYLIGHT\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0200\r\nDTSTART:19700329T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nEND:DAYLIGHT\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nDTSTART:19701025T030000\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n{events}END:VCALENDAR\r\n"
        ))
        .unwrap()
    }

    fn event(lines: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Meet\r\n{lines}\r\nEND:VEVENT\r\n")
    }

    /// The days on which it happens in 2026, as (month, day).
    fn on(lines: &str) -> Vec<(i64, i64)> {
        let c = cal(&event(lines));
        c.between(t(2026, 1, 1, 0, 0), t(2027, 1, 1, 0, 0), 0).iter().map(|e| civil(e.0.div_euclid(DAY))).map(|(_, m, d)| (m, d)).collect()
    }

    #[test]
    fn days_are_counted_right() {
        assert_eq!((days(1970, 1, 1), days(2026, 11, 26), weekday(days(2026, 11, 26))), (0, 20783, 3));
        for d in -100_000..100_000i64 {
            let (y, m, dd) = civil(d * 7);
            assert_eq!(days(y, m, dd), d * 7);
        }
        assert_eq!((month_len(2024, 2), month_len(2026, 2), month_len(2026, 12)), (29, 28, 31));
        assert_eq!(nth(2026, 11, -1, 0), Some(days(2026, 11, 30)));
        assert_eq!(nth(2026, 11, 5, 3), None);
    }

    #[test]
    fn simple_things_are_read() {
        let c = cal(&event("DTSTART:20261126T093000Z"));
        assert_eq!(c.between(t(2026, 11, 26, 9, 0), t(2026, 11, 26, 10, 0), 0), [(t(2026, 11, 26, 9, 30), "Meet".to_string(), "a".to_string())]);
        assert!(c.between(t(2026, 11, 26, 9, 31), t(2026, 11, 26, 10, 0), 0).is_empty());
        // on this PC's own clock
        let c = cal(&event("DTSTART:20261126T093000"));
        assert_eq!(c.between(0, i64::MAX / 2, 19800)[0].0, t(2026, 11, 26, 4, 0));
        // folded, escaped, with odd parameters
        let c = cal("BEGIN:VEVENT\r\nUID:x\r\nSUMMARY;LANGUAGE=\"a;b:c\":Plan\\, talk\\n an\r\n d eat\r\nDTSTART;X=1:20261126T093000Z\r\nBEGIN:VALARM\r\nSUMMARY:no\r\nEND:VALARM\r\nEND:VEVENT\r\n");
        assert_eq!(c.between(0, i64::MAX / 2, 0)[0].1, "Plan, talk  and eat");
    }

    #[test]
    fn clocks_elsewhere_change_in_summer() {
        for (day, hour) in [("20260115", 8), ("20260328", 8), ("20260329", 7), ("20260715", 7), ("20261024", 7), ("20261025", 8)] {
            let c = cal(&event(&format!("DTSTART;TZID=Berlin:{day}T090000")));
            let at = c.between(0, i64::MAX / 2, 0)[0].0;
            assert_eq!(at.rem_euclid(DAY) / 3600, hour, "{day}");
        }
        // a series keeps its hour there, so it moves here
        let c = cal(&event("DTSTART;TZID=Berlin:20260320T090000\r\nRRULE:FREQ=WEEKLY"));
        let hours: Vec<i64> = c.between(t(2026, 3, 19, 0, 0), t(2026, 4, 5, 0, 0), 0).iter().map(|e| e.0.rem_euclid(DAY) / 3600).collect();
        assert_eq!(hours, [8, 8, 7]);
    }

    #[test]
    fn series_fall_on_the_right_days() {
        assert_eq!(on("DTSTART:20261229T090000Z\r\nRRULE:FREQ=DAILY"), [(12, 29), (12, 30), (12, 31)]);
        assert_eq!(on("DTSTART:20261220T090000Z\r\nRRULE:FREQ=DAILY;INTERVAL=5"), [(12, 20), (12, 25), (12, 30)]);
        assert_eq!(on("DTSTART:20261221T090000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR;COUNT=4"), [(12, 21), (12, 23), (12, 25), (12, 28)]);
        assert_eq!(on("DTSTART:20261202T090000Z\r\nRRULE:FREQ=WEEKLY;INTERVAL=2"), [(12, 2), (12, 16), (12, 30)]);
        assert_eq!(on("DTSTART:20261001T090000Z\r\nRRULE:FREQ=MONTHLY"), [(10, 1), (11, 1), (12, 1)]);
        assert_eq!(on("DTSTART:20261013T090000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=2TU"), [(10, 13), (11, 10), (12, 8)]);
        assert_eq!(on("DTSTART:20261030T090000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=-1FR"), [(10, 30), (11, 27), (12, 25)]);
        assert_eq!(on("DTSTART:20261031T090000Z\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=-1"), [(10, 31), (11, 30), (12, 31)]);
        assert_eq!(on("DTSTART:20261030T090000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1"), [(10, 30), (11, 30), (12, 31)]);
        assert_eq!(on("DTSTART:20200214T090000Z\r\nRRULE:FREQ=YEARLY"), [(2, 14)]);
        assert_eq!(on("DTSTART:20200101T090000Z\r\nRRULE:FREQ=YEARLY;BYMONTH=5;BYDAY=2SU"), [(5, 10)]);
        assert_eq!(on("DTSTART:20261220T090000Z\r\nRRULE:FREQ=DAILY;UNTIL=20261222T090000Z"), [(12, 20), (12, 21), (12, 22)]);
        assert_eq!(on("DTSTART:20261220T090000Z\r\nRRULE:FREQ=DAILY;UNTIL=20261221"), [(12, 20), (12, 21)]);
        // long gone
        assert!(on("DTSTART:20200101T090000Z\r\nRRULE:FREQ=DAILY;COUNT=10").is_empty());
        // days nobody has
        assert_eq!(on("DTSTART:20260131T090000Z\r\nRRULE:FREQ=MONTHLY;COUNT=3"), [(1, 31), (3, 31), (5, 31)]);
    }

    #[test]
    fn single_meetings_can_be_left_out_moved_or_cancelled() {
        assert_eq!(on("DTSTART:20261229T090000Z\r\nRRULE:FREQ=DAILY\r\nEXDATE:20261230T090000Z"), [(12, 29), (12, 31)]);
        let series = event("DTSTART:20261229T090000Z\r\nRRULE:FREQ=DAILY");
        let moved = event("RECURRENCE-ID:20261230T090000Z\r\nDTSTART:20261230T150000Z");
        let c = cal(&format!("{series}{moved}"));
        let at: Vec<i64> = c.between(t(2026, 12, 29, 0, 0), t(2027, 1, 1, 0, 0), 0).iter().map(|e| e.0).collect();
        assert_eq!(at, [t(2026, 12, 29, 9, 0), t(2026, 12, 30, 15, 0), t(2026, 12, 31, 9, 0)]);
        let off = event("RECURRENCE-ID:20261230T090000Z\r\nDTSTART:20261230T090000Z\r\nSTATUS:CANCELLED");
        assert_eq!(cal(&format!("{series}{off}")).between(t(2026, 12, 29, 0, 0), t(2027, 1, 1, 0, 0), 0).len(), 2);
        assert!(on("DTSTART:20261229T090000Z\r\nSTATUS:CANCELLED").is_empty());
        assert!(on("DTSTART;VALUE=DATE:20261229").is_empty(), "nothing to be on time for");
    }

    #[test]
    fn anything_else_is_refused_and_nothing_brings_it_down() {
        assert!(read("").is_none() && read("<html>").is_none());
        let good = format!("BEGIN:VCALENDAR\r\n{}END:VCALENDAR", event("DTSTART;TZID=Nowhere:20261226T090000\r\nRRULE:FREQ=MONTHLY;BYDAY=-5FR,2,XX;INTERVAL=0;BYSETPOS=9\r\nEXDATE:junk"));
        let mut seed = 3u32;
        for _ in 0..4000 {
            let mut b = good.clone().into_bytes();
            for _ in 0..3 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let i = (seed >> 8) as usize % b.len();
                match seed >> 30 {
                    0 => b.truncate(i.max(20)),
                    1 => b[i] = b"9-:;=\nTZ,"[(seed >> 4) as usize % 9],
                    _ => drop(b.remove(i)),
                }
            }
            if let Some(c) = read(&String::from_utf8_lossy(&b)) {
                c.between(t(2026, 12, 20, 0, 0), t(2026, 12, 28, 0, 0), 19800);
            }
        }
    }

    #[test]
    fn each_is_announced_once_ahead_and_once_at_the_start() {
        let start = t(2026, 11, 26, 10, 0);
        let next = [(start, "Standup".to_string(), "u".to_string())];
        let mut said = Vec::new();
        let mut at = |now: i64| pick(&next, &mut said, now, 0, 10);
        assert_eq!(at(start - 660), None);
        assert_eq!(at(start - 600).as_deref(), Some("Standup in 10 minutes (10:00)."));
        assert_eq!(at(start - 540), None);
        assert_eq!(at(start - 1), None);
        assert_eq!(at(start).as_deref(), Some("Standup starts now!"));
        assert_eq!(at(start + 5), None);
        // started late: still told how long there is
        let mut said = Vec::new();
        assert_eq!(pick(&next, &mut said, start - 61, 0, 10).as_deref(), Some("Standup in 2 minutes (10:00)."));
        assert_eq!(pick(&next, &mut Vec::new(), start + 500, 0, 10), None, "long over");
    }

    #[test]
    fn times_and_places_are_shown_short() {
        let now = t(2026, 11, 26, 10, 0);
        assert_eq!((until(now + 1500, now, 0).as_str(), until(now, now, 0).as_str(), until(now + 5 * 3600, now, 0).as_str(), until(now + 20 * 3600, now, 0).as_str()), ("IN 25 MIN", "NOW", "IN 5 H", "TOMORROW"));
        assert_eq!(clock(now, 19800), "15:30");
        assert_eq!(link(" webcal://a.b/c.ics ").as_deref(), Some("https://a.b/c.ics"));
        assert!(link("http://a.b/c").is_none() && link("file:///c").is_none());
        assert_eq!(source_name("https://calendar.google.com/calendar/ical/secret/basic.ics"), "calendar.google.com");
        assert!(!source_name("https://www.a.b/private-123").contains("123"));
    }
}
