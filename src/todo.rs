//! Smart todo list: model, natural-language time parsing, persistence.
//! All times are "local minutes since 1970-01-01 00:00" so there is no time-zone math.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub due: Option<i64>,
    pub done: bool,
    pub fired: bool, // reminder already shown this run (not saved)
    pub repeat: u8,  // 0 never, 1 daily, 2 weekly, 3 weekdays
    /// Set on a subtask: the id of its top-level task. Lists are two tiers deep, no more.
    pub parent: Option<u32>,
}

pub const REPEAT_NAMES: [&str; 4] = ["", "daily", "weekly", "weekdays"];

/// Finish a task. One-off tasks become done; repeating ones roll to the next occurrence.
/// Returns true if the task was completed (so the pet can cheer).
pub fn complete(t: &mut Todo, now: i64) -> bool {
    if t.repeat == 0 {
        t.done = true;
        return true;
    }
    let mut due = t.due.unwrap_or(now);
    // next occurrence strictly after now, keeping the time of day
    loop {
        due += if t.repeat == 2 { 10080 } else { 1440 };
        let wd = weekday_of_days(due.div_euclid(1440));
        let skip = t.repeat == 3 && (wd == 0 || wd == 6);
        if due > now && !skip {
            break;
        }
    }
    t.due = Some(due);
    t.fired = false;
    true
}

/// Like `parse`, but understands "daily", "every day", "weekly", "every monday", "weekdays".
/// Returns (title, first due time, repeat kind).
pub fn parse_full(input: &str, now: i64) -> (String, Option<i64>, u8) {
    let words: Vec<&str> = input.split_whitespace().collect();
    let clean = |w: &str| w.trim_matches(|c: char| ",.;!?".contains(c)).to_lowercase();
    let mut kept: Vec<&str> = Vec::new();
    let mut repeat = 0u8;
    let mut i = 0;
    while i < words.len() {
        let w = clean(words[i]);
        let next = words.get(i + 1).map(|n| clean(n));
        if w == "daily" || w == "everyday" {
            repeat = 1;
        } else if w == "weekly" {
            repeat = 2;
        } else if w == "weekdays" {
            repeat = 3;
        } else if w == "every" && next.as_deref() == Some("day") {
            repeat = 1;
            i += 1;
        } else if w == "every" && next.as_deref() == Some("week") {
            repeat = 2;
            i += 1;
        } else if w == "every" && next.as_deref() == Some("weekday") {
            repeat = 3;
            i += 1;
        } else if w == "every" && next.as_deref().and_then(weekday_name).is_some() {
            repeat = 2; // drop "every", keep the weekday word so the date parser picks the day
        } else {
            kept.push(words[i]);
        }
        i += 1;
    }
    if repeat == 0 {
        let (t, d) = parse(input, now);
        return (t, d, 0);
    }
    let rest = kept.join(" ");
    let (title, mut due) = parse(&rest, now);
    if due.is_none() {
        // repeating tasks need a time: default 9:00
        due = parse(&format!("{rest} 9am"), now).1;
    }
    (format!("{title} ({})", REPEAT_NAMES[repeat as usize]), due, repeat)
}

// ---------- calendar math (Howard Hinnant's algorithms) ----------

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn minutes_from_civil(y: i64, m: i64, d: i64, h: i64, mi: i64) -> i64 {
    days_from_civil(y, m, d) * 1440 + h * 60 + mi
}

/// 0 = Sunday .. 6 = Saturday
fn weekday_of_days(days: i64) -> i64 {
    (days + 4).rem_euclid(7) // 1970-01-01 was a Thursday
}

// ---------- natural-language parsing ----------

fn unit_minutes(s: &str) -> Option<i64> {
    Some(match s {
        "m" | "min" | "mins" | "minute" | "minutes" => 1,
        "h" | "hr" | "hrs" | "hour" | "hours" => 60,
        "d" | "day" | "days" => 1440,
        "w" | "wk" | "week" | "weeks" => 10080,
        _ => return None,
    })
}

/// After "in": "30m", "2 hours", "a day". Returns (minutes, tokens used).
fn rel_amount(low: &[String], j: usize) -> Option<(i64, usize)> {
    let t = low.get(j)?.as_str();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let suffix = &t[digits.len()..];
    if !digits.is_empty() {
        let n: i64 = digits.parse().ok()?;
        if !suffix.is_empty() {
            return unit_minutes(suffix).map(|u| (n * u, 1));
        }
        let u = unit_minutes(low.get(j + 1)?)?;
        return Some((n * u, 2));
    }
    if matches!(t, "a" | "an" | "one") {
        let u = unit_minutes(low.get(j + 1)?)?;
        return Some((u, 2));
    }
    None
}

fn weekday_name(s: &str) -> Option<i64> {
    Some(match s {
        "sun" | "sunday" => 0,
        "mon" | "monday" => 1,
        "tue" | "tues" | "tuesday" => 2,
        "wed" | "weds" | "wednesday" => 3,
        "thu" | "thur" | "thurs" | "thursday" => 4,
        "fri" | "friday" => 5,
        "sat" | "saturday" => 6,
        _ => return None,
    })
}

/// Clock time at token i: "5pm", "5:30pm", "17:30", "5 pm", "noon", or a bare
/// hour after "at". Returns (minute of day, tokens used).
fn time_at(low: &[String], i: usize, after_at: bool) -> Option<(i64, usize)> {
    let t = low[i].as_str();
    match t {
        "noon" => return Some((720, 1)),
        "midnight" => return Some((0, 1)),
        "morning" => return Some((540, 1)),
        "afternoon" => return Some((900, 1)),
        "evening" => return Some((1080, 1)),
        _ => {}
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 2 {
        return None;
    }
    let mut rest = &t[digits.len()..];
    let hour: i64 = digits.parse().ok()?;
    let mut minute = 0;
    let mut has_colon = false;
    if let Some(r) = rest.strip_prefix(':') {
        let md: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
        if md.len() != 2 {
            return None;
        }
        minute = md.parse().ok()?;
        rest = &r[2..];
        has_colon = true;
    }
    let mut used = 1;
    let mut ampm = match rest {
        "am" | "a.m." => Some(false),
        "pm" | "p.m." => Some(true),
        "" => None,
        _ => return None,
    };
    if ampm.is_none() {
        if let Some(n) = low.get(i + 1) {
            match n.as_str() {
                "am" => { ampm = Some(false); used = 2; }
                "pm" => { ampm = Some(true); used = 2; }
                _ => {}
            }
        }
    }
    if minute > 59 {
        return None;
    }
    let h = match ampm {
        Some(pm) => {
            if !(1..=12).contains(&hour) {
                return None;
            }
            (hour % 12) + if pm { 12 } else { 0 }
        }
        None => {
            if !has_colon && !after_at {
                return None; // a bare number is not a time ("buy 2 apples")
            }
            if hour > 23 {
                return None;
            }
            if !has_colon && (1..=6).contains(&hour) { hour + 12 } else { hour }
        }
    };
    Some((h * 60 + minute, used))
}

/// Split "call mom tomorrow 5pm" into ("Call mom", Some(due)).
pub fn parse(input: &str, now: i64) -> (String, Option<i64>) {
    let words: Vec<&str> = input.split_whitespace().collect();
    let low: Vec<String> = words
        .iter()
        .map(|w| w.trim_matches(|c: char| ",.;!?".contains(c)).to_lowercase())
        .collect();
    let mut used = vec![false; words.len()];
    let today = now.div_euclid(1440);
    let wd_today = weekday_of_days(today);

    let mut rel: Option<i64> = None;
    let mut day: Option<i64> = None;
    let mut tod: Option<i64> = None;
    let mut default_tod: Option<i64> = None;

    let mut i = 0;
    while i < low.len() {
        let w = low[i].as_str();
        if w == "in" {
            if let Some((n, cnt)) = rel_amount(&low, i + 1) {
                rel = Some(n);
                for k in i..=i + cnt {
                    used[k] = true;
                }
                i += cnt + 1;
                continue;
            }
        }
        if day.is_none() {
            let mut hit = false;
            match w {
                "today" => { day = Some(today); hit = true; }
                "tonight" => { day = Some(today); default_tod = Some(1200); hit = true; }
                "tomorrow" | "tmrw" | "tmr" => { day = Some(today + 1); hit = true; }
                "next" if low.get(i + 1).map(|s| s.as_str()) == Some("week") => {
                    day = Some(today + 7);
                    used[i + 1] = true;
                    hit = true;
                }
                _ => {
                    if let Some(t) = weekday_name(w) {
                        let mut delta = (t - wd_today).rem_euclid(7);
                        if delta == 0 {
                            delta = 7;
                        }
                        day = Some(today + delta);
                        if i > 0 && low[i - 1] == "next" {
                            used[i - 1] = true;
                        }
                        hit = true;
                    }
                }
            }
            if hit {
                used[i] = true;
                i += 1;
                continue;
            }
        }
        if tod.is_none() {
            let after_at = i > 0 && low[i - 1] == "at";
            if let Some((m, n)) = time_at(&low, i, after_at) {
                tod = Some(m);
                for k in i..i + n {
                    used[k] = true;
                }
                i += n;
                continue;
            }
        }
        i += 1;
    }

    let due = if let Some(r) = rel {
        Some(now + r)
    } else if day.is_some() || tod.is_some() {
        let d = day.unwrap_or(today);
        let t = tod.or(default_tod).unwrap_or(540);
        let mut m = d * 1440 + t;
        if day.is_none() && m <= now {
            m += 1440; // only a clock time, already past: tomorrow
        }
        Some(m)
    } else {
        None
    };

    // swallow a preposition that introduced a consumed phrase
    for k in 1..words.len() {
        if used[k] && !used[k - 1] && matches!(low[k - 1].as_str(), "at" | "on" | "by") {
            used[k - 1] = true;
        }
    }
    let mut title: Vec<&str> = words.iter().zip(&used).filter(|(_, u)| !**u).map(|(w, _)| *w).collect();
    while title.last().map_or(false, |w| matches!(w.to_lowercase().as_str(), "at" | "on" | "by" | "in" | "for" | "and")) {
        title.pop();
    }
    let mut title = title.join(" ");
    if title.is_empty() {
        title = input.trim().to_string();
    }
    let mut c = title.chars();
    let title = match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => title,
    };
    (title, due)
}

pub fn fmt_due(due: i64, now: i64) -> String {
    let (days, mins) = (due.div_euclid(1440), due.rem_euclid(1440));
    let diff = days - now.div_euclid(1440);
    let clock = format!("{:02}:{:02}", mins / 60, mins % 60);
    if diff == 0 && due > now && due - now < 60 {
        return format!("in {} min", due - now);
    }
    match diff {
        0 => format!("today {clock}"),
        1 => format!("tomorrow {clock}"),
        -1 => format!("yesterday {clock}"),
        2..=6 => {
            const N: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            format!("{} {clock}", N[weekday_of_days(days) as usize])
        }
        _ => {
            const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            let (_, m, d) = civil_from_days(days);
            format!("{d} {} {clock}", M[(m - 1) as usize])
        }
    }
}

/// The top-level task a new subtask should hang under when `sel` is selected:
/// the task itself, or its parent if `sel` is already a subtask.
pub fn parent_for(list: &[Todo], sel: u32) -> Option<u32> {
    let t = list.iter().find(|t| t.id == sel)?;
    Some(t.parent.unwrap_or(t.id))
}

/// Ids of `id` and all its subtasks.
pub fn family(list: &[Todo], id: u32) -> Vec<u32> {
    list.iter().filter(|t| t.id == id || t.parent == Some(id)).map(|t| t.id).collect()
}

/// (done, total) subtasks of a task, or None if it has none.
pub fn progress(list: &[Todo], id: u32) -> Option<(usize, usize)> {
    let kids: Vec<&Todo> = list.iter().filter(|t| t.parent == Some(id)).collect();
    (!kids.is_empty()).then(|| (kids.iter().filter(|t| t.done).count(), kids.len()))
}

/// Tasks in display order: each top-level task followed by its subtasks. A subtask whose
/// parent is gone (or is itself a subtask) is promoted to top level.
fn grouped(list: &[Todo]) -> Vec<Todo> {
    let key = |t: &Todo| (t.done, t.due.unwrap_or(i64::MAX), t.id);
    let tops: Vec<u32> = list.iter().filter(|t| t.parent.is_none()).map(|t| t.id).collect();
    let mut all: Vec<Todo> = list.to_vec();
    for t in &mut all {
        if t.parent.map_or(false, |p| !tops.contains(&p)) {
            t.parent = None;
        }
    }
    all.sort_by_key(key);
    let mut out = Vec::with_capacity(all.len());
    for t in all.iter().filter(|t| t.parent.is_none()) {
        out.push(t.clone());
        out.extend(all.iter().filter(|c| c.parent == Some(t.id)).cloned());
    }
    out
}

/// Open items first (soonest due first, undated last), done items at the bottom;
/// subtasks sit right under their task, sorted the same way.
pub fn sort(list: &mut Vec<Todo>) {
    *list = grouped(list);
}

// ---------- persistence: "flags|due|text" per line ----------
// flags: "0"/"1" for done, an optional repeat digit ("02" = open, weekly), and a
// trailing "s" on a subtask, which belongs to the nearest top-level line above it.

pub fn load(path: &Path) -> Vec<Todo> {
    let Ok(txt) = std::fs::read_to_string(path) else { return Vec::new() };
    let mut out: Vec<Todo> = Vec::new();
    let mut last_top: Option<u32> = None;
    for l in txt.lines() {
        let mut p = l.splitn(3, '|');
        let (Some(flags), Some(due), Some(text)) = (p.next(), p.next(), p.next()) else { continue };
        let id = out.len() as u32 + 1;
        let sub = flags.contains('s');
        let parent = if sub { last_top } else { None };
        if parent.is_none() {
            last_top = Some(id);
        }
        out.push(Todo {
            id,
            text: text.to_string(),
            due: due.parse::<i64>().ok(),
            done: flags.starts_with('1'),
            fired: false,
            repeat: flags.chars().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(0).min(3) as u8,
            parent,
        });
    }
    out
}

pub fn save(path: &Path, list: &[Todo]) {
    let mut out = String::new();
    for t in grouped(list) {
        let due = t.due.map(|d| d.to_string()).unwrap_or_default();
        let rep = if t.repeat > 0 { t.repeat.to_string() } else { String::new() };
        let sub = if t.parent.is_some() { "s" } else { "" };
        out.push_str(&format!("{}{}{}|{}|{}\n", t.done as u8, rep, sub, due, t.text.replace('\n', " ")));
    }
    let _ = std::fs::write(path, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Wed 2026-11-04 10:00
    fn now() -> i64 {
        minutes_from_civil(2026, 11, 4, 10, 0)
    }

    fn todo(repeat: u8, due: i64) -> Todo {
        Todo { id: 1, text: "x".into(), due: Some(due), done: false, fired: true, repeat, parent: None }
    }

    fn item(id: u32, parent: Option<u32>, due: Option<i64>, done: bool) -> Todo {
        Todo { id, text: format!("t{id}"), due, done, fired: false, repeat: 0, parent }
    }

    #[test]
    fn subtasks_sit_under_their_task() {
        let mut l = vec![
            item(1, None, Some(500), false),
            item(2, Some(1), None, false),
            item(3, None, Some(100), false),
            item(4, Some(1), Some(50), false),
            item(5, Some(3), None, true),
            item(6, Some(99), None, false), // orphan: promoted
        ];
        sort(&mut l);
        let ids: Vec<u32> = l.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![3, 5, 1, 4, 2, 6]);
        assert_eq!(l[5].parent, None);
        assert_eq!(progress(&l, 1), Some((0, 2)));
        assert_eq!(progress(&l, 3), Some((1, 1)));
        assert_eq!(progress(&l, 2), None);
        assert_eq!(parent_for(&l, 4), Some(1));
        assert_eq!(parent_for(&l, 1), Some(1));
        let mut fam = family(&l, 1);
        fam.sort();
        assert_eq!(fam, vec![1, 2, 4]);
    }

    #[test]
    fn subtasks_survive_save_and_load() {
        let path = std::env::temp_dir().join("pixel-pet-test-subtodos.txt");
        // saved out of order on purpose: save groups children under their parent
        let mut weekly = item(7, Some(3), Some(10), true);
        weekly.repeat = 2;
        save(&path, &[item(3, None, None, false), item(9, None, Some(5), false), weekly, item(4, Some(9), None, false)]);
        let l = load(&path);
        let _ = std::fs::remove_file(path);
        let shape: Vec<(Option<u32>, bool, u8)> = l.iter().map(|t| (t.parent, t.done, t.repeat)).collect();
        // ids are renumbered on load: t9=1, t4=2, t3=3, t7=4
        assert_eq!(shape, vec![(None, false, 0), (Some(1), false, 0), (None, false, 0), (Some(3), true, 2)]);
        assert_eq!(l[1].text, "t4");
    }

    #[test]
    fn parses_repeats() {
        let (t, d, r) = parse_full("water plants every day 8am", now());
        assert_eq!((t.as_str(), r), ("Water plants (daily)", 1));
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 5, 8, 0)));
        let (t, d, r) = parse_full("team sync every monday 10am", now());
        assert_eq!((t.as_str(), r), ("Team sync (weekly)", 2));
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 9, 10, 0)));
        let (_, d, r) = parse_full("standup weekdays 9:30", now());
        assert_eq!(r, 3);
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 5, 9, 30)));
        let (_, d, r) = parse_full("stretch daily", now());
        assert_eq!((r, d), (1, Some(minutes_from_civil(2026, 11, 5, 9, 0)))); // default 9am
        let (t, _, r) = parse_full("call mom tomorrow 5pm", now());
        assert_eq!((t.as_str(), r), ("Call mom", 0));
    }

    #[test]
    fn completing_rolls_repeating_tasks_forward() {
        let mut d = todo(1, minutes_from_civil(2026, 11, 4, 9, 0));
        assert!(complete(&mut d, now()));
        assert!(!d.done && !d.fired);
        assert_eq!(d.due, Some(minutes_from_civil(2026, 11, 5, 9, 0)));
        let mut w = todo(2, minutes_from_civil(2026, 11, 2, 9, 0));
        complete(&mut w, now());
        assert_eq!(w.due, Some(minutes_from_civil(2026, 11, 9, 9, 0)));
        let mut once = todo(0, now());
        complete(&mut once, now());
        assert!(once.done);
    }

    #[test]
    fn weekday_repeat_skips_weekends() {
        let mut t = todo(3, minutes_from_civil(2026, 11, 6, 9, 0)); // Friday
        complete(&mut t, minutes_from_civil(2026, 11, 6, 10, 0));
        assert_eq!(t.due, Some(minutes_from_civil(2026, 11, 9, 9, 0))); // Monday
    }

    #[test]
    fn overdue_repeat_catches_up_to_the_future() {
        let mut t = todo(1, minutes_from_civil(2026, 10, 20, 9, 0));
        complete(&mut t, now());
        assert_eq!(t.due, Some(minutes_from_civil(2026, 11, 5, 9, 0)));
    }

    #[test]
    fn repeat_survives_save_and_load() {
        let path = std::env::temp_dir().join("pixel-pet-test-todos.txt");
        save(&path, &[todo(2, 100), todo(0, 200)]);
        let l = load(&path);
        assert_eq!((l[0].repeat, l[1].repeat, l[0].due), (2, 0, Some(100)));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn weekday_math() {
        assert_eq!(weekday_of_days(days_from_civil(2026, 11, 4)), 3);
    }

    #[test]
    fn relative() {
        let (t, d) = parse("stretch in 45m", now());
        assert_eq!(t, "Stretch");
        assert_eq!(d, Some(now() + 45));
        let (t, d) = parse("call dentist in 2 hours", now());
        assert_eq!(t, "Call dentist");
        assert_eq!(d, Some(now() + 120));
    }

    #[test]
    fn tomorrow_time() {
        let (t, d) = parse("call mom tomorrow 5pm", now());
        assert_eq!(t, "Call mom");
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 5, 17, 0)));
    }

    #[test]
    fn weekday_time() {
        let (t, d) = parse("gym on fri at 6:30pm", now());
        assert_eq!(t, "Gym");
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 6, 18, 30)));
    }

    #[test]
    fn time_only_rolls_over() {
        let (_, d) = parse("water plants at 9am", now());
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 5, 9, 0)));
        let (_, d) = parse("standup 14:15", now());
        assert_eq!(d, Some(minutes_from_civil(2026, 11, 4, 14, 15)));
    }

    #[test]
    fn plain_text_has_no_time() {
        let (t, d) = parse("buy 2 apples", now());
        assert_eq!(t, "Buy 2 apples");
        assert_eq!(d, None);
    }

    #[test]
    fn format() {
        assert_eq!(fmt_due(minutes_from_civil(2026, 11, 5, 9, 0), now()), "tomorrow 09:00");
        assert_eq!(fmt_due(now() + 25, now()), "in 25 min");
    }
}
