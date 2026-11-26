//! Focus history: sessions and minutes per day, streaks, daily goal, weekly recap.
//! Days are "days since 1970-01-01" in local time. Pure logic, no Win32.

const KEEP_DAYS: u32 = 90;

pub struct Stats {
    pub days: Vec<(u32, u32, u32)>, // (day, sessions, minutes), ascending
    pub goal: u32,                  // sessions per day
    pub hours: [u32; 24],           // minutes focused in each hour of the day, all kept
    pub stopped: Vec<(u32, u32)>,   // (day, rounds stopped before the end), ascending
}

pub struct Week {
    pub sessions: u32,
    pub minutes: u32,
    pub active_days: u32,
    pub best: Option<(u32, u32)>, // (day, sessions)
}

impl Stats {
    pub const fn new() -> Stats {
        Stats { days: Vec::new(), goal: 4, hours: [0; 24], stopped: Vec::new() }
    }

    /// Count the minutes of a round that ended at `end` (minutes after midnight) in the hours they
    /// were in.
    pub fn at_hours(&mut self, end: u32, minutes: u32) {
        let mut left = minutes.min(24 * 60);
        let mut at = end as i64;
        while left > 0 {
            let hour = (at - 1).rem_euclid(24 * 60) / 60;
            let into = (at - 1).rem_euclid(60) + 1; // minutes of this hour up to `at`
            let n = (into as u32).min(left);
            self.hours[hour as usize] += n;
            left -= n;
            at -= n as i64;
        }
    }

    /// A focus round was stopped before its end.
    pub fn stop(&mut self, day: u32) {
        match self.stopped.iter_mut().find(|d| d.0 == day) {
            Some(d) => d.1 += 1,
            None => {
                self.stopped.push((day, 1));
                self.stopped.sort();
            }
        }
        self.stopped.retain(|d| d.0 + KEEP_DAYS > day);
    }

    /// (finished, stopped) rounds in the `n` days ending today.
    pub fn finished(&self, day: u32, n: u32) -> (u32, u32) {
        let within = |d: u32| d <= day && d + n > day;
        (self.days.iter().filter(|d| within(d.0)).map(|d| d.1).sum(), self.stopped.iter().filter(|d| within(d.0)).map(|d| d.1).sum())
    }

    /// The two hours of the day with the most focus in them: the first of them, if there is any.
    pub fn best_hours(&self) -> Option<u32> {
        (0..24u32).map(|h| (self.hours[h as usize] + self.hours[(h as usize + 1) % 24], h)).filter(|(m, _)| *m > 0).max_by_key(|(m, h)| (*m, self.hours[*h as usize], 24 - h)).map(|(_, h)| h)
    }

    /// Log a finished focus session. Returns true if this one just reached the daily goal.
    pub fn record(&mut self, day: u32, minutes: u32) -> bool {
        match self.days.iter_mut().find(|d| d.0 == day) {
            Some(d) => {
                d.1 += 1;
                d.2 += minutes;
            }
            None => {
                self.days.push((day, 1, minutes));
                self.days.sort(); // keeps history ordered even if the clock jumped back
            }
        }
        self.days.retain(|d| d.0 + KEEP_DAYS > day);
        self.today(day).0 == self.goal
    }

    /// (sessions, minutes) so far today.
    pub fn today(&self, day: u32) -> (u32, u32) {
        self.days.iter().find(|d| d.0 == day).map_or((0, 0), |d| (d.1, d.2))
    }

    /// Consecutive days with at least one session. Today not yet started doesn't break it.
    pub fn streak(&self, day: u32) -> u32 {
        let has = |d: u32| self.days.iter().any(|x| x.0 == d && x.1 > 0);
        let mut d = if has(day) { day } else { day.saturating_sub(1) };
        let mut n = 0;
        while has(d) {
            n += 1;
            if d == 0 {
                break;
            }
            d -= 1;
        }
        n
    }

    /// Sessions for each of the `n` days ending today (oldest first).
    pub fn last_n(&self, day: u32, n: u32) -> Vec<u32> {
        (0..n)
            .map(|i| {
                day.checked_sub(n - 1 - i)
                    .map_or(0, |d| self.days.iter().find(|x| x.0 == d).map_or(0, |x| x.1))
            })
            .collect()
    }

    /// Longest run of consecutive days with at least one session (within the kept history).
    pub fn best_streak(&self) -> u32 {
        let (mut best, mut cur, mut prev) = (0u32, 0u32, None::<u32>);
        for &(d, s, _) in &self.days {
            if s == 0 {
                continue;
            }
            cur = if prev.map_or(false, |p| p + 1 == d) { cur + 1 } else { 1 };
            prev = Some(d);
            best = best.max(cur);
        }
        best
    }

    /// (sessions, minutes, active days, best day) over everything kept.
    pub fn totals(&self) -> (u32, u32, u32, Option<(u32, u32)>) {
        let mut t = (0, 0, 0, None::<(u32, u32)>);
        for &(d, s, m) in &self.days {
            t.0 += s;
            t.1 += m;
            if s > 0 {
                t.2 += 1;
            }
            if s > 0 && t.3.map_or(true, |b| s > b.1) {
                t.3 = Some((d, s));
            }
        }
        t
    }

    /// The 7 days ending today.
    pub fn week(&self, day: u32) -> Week {
        let mut w = Week { sessions: 0, minutes: 0, active_days: 0, best: None };
        for &(d, s, m) in self.days.iter().filter(|d| d.0 <= day && d.0 + 7 > day) {
            w.sessions += s;
            w.minutes += m;
            if s > 0 {
                w.active_days += 1;
            }
            if w.best.map_or(true, |b| s > b.1) {
                w.best = Some((d, s));
            }
        }
        w
    }

    /// One day per line; older versions read the lines that start with a letter as nothing.
    pub fn to_text(&self) -> String {
        let mut t: String = self.days.iter().map(|d| format!("{} {} {}\n", d.0, d.1, d.2)).collect();
        for (h, m) in self.hours.iter().enumerate().filter(|(_, m)| **m > 0) {
            t.push_str(&format!("h {h} {m}\n"));
        }
        for (d, n) in &self.stopped {
            t.push_str(&format!("x {d} {n}\n"));
        }
        t
    }

    pub fn load_text(&mut self, txt: &str) {
        self.days.clear();
        self.stopped.clear();
        self.hours = [0; 24];
        for l in txt.lines() {
            let (kind, rest) = match l.trim_start().split_once(' ') {
                Some((k @ ("h" | "x"), r)) => (k, r),
                _ => ("", l),
            };
            let v: Vec<u32> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            match (kind, &v[..]) {
                ("", &[d, s, m]) => self.days.push((d, s, m)),
                ("h", &[h, m]) if h < 24 => self.hours[h as usize] = m,
                ("x", &[d, n]) => self.stopped.push((d, n)),
                _ => {}
            }
        }
        self.days.sort();
        self.stopped.sort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_hits_goal_once() {
        let mut s = Stats::new();
        s.goal = 2;
        assert!(!s.record(100, 25));
        assert!(s.record(100, 25));
        assert!(!s.record(100, 25));
        assert_eq!(s.today(100), (3, 75));
        assert_eq!(s.today(101), (0, 0));
    }

    #[test]
    fn streak_counts_back_and_forgives_today() {
        let mut s = Stats::new();
        for d in [97, 98, 99] {
            s.record(d, 25);
        }
        assert_eq!(s.streak(99), 3);
        assert_eq!(s.streak(100), 3); // today not started yet
        assert_eq!(s.streak(101), 0); // missed a full day
        s.record(101, 25);
        assert_eq!(s.streak(101), 1);
    }

    #[test]
    fn week_summary() {
        let mut s = Stats::new();
        s.record(10, 25); // too old for the window ending day 20
        s.record(18, 25);
        s.record(18, 50);
        s.record(20, 25);
        let w = s.week(20);
        assert_eq!((w.sessions, w.minutes, w.active_days), (3, 100, 2));
        assert_eq!(w.best, Some((18, 2)));
    }

    #[test]
    fn last_n_lines_up_oldest_first() {
        let mut s = Stats::new();
        s.record(18, 25);
        s.record(18, 25);
        s.record(20, 25);
        assert_eq!(s.last_n(20, 7), vec![0, 0, 0, 0, 2, 0, 1]);
        assert_eq!(s.last_n(20, 3), vec![2, 0, 1]);
        assert_eq!(Stats::new().last_n(2, 7), vec![0; 7]); // near the epoch: no underflow
    }

    #[test]
    fn best_streak_and_totals() {
        let mut s = Stats::new();
        for d in [10, 11, 12, 20, 21] {
            s.record(d, 25);
        }
        s.record(11, 50); // out of order on purpose: must merge into day 11, not duplicate it
        assert_eq!(s.days.len(), 5);
        assert_eq!(s.best_streak(), 3);
        assert_eq!(Stats::new().best_streak(), 0);
        let (sessions, minutes, active, best) = s.totals();
        assert_eq!((sessions, minutes, active, best), (6, 175, 5, Some((11, 2))));
    }

    #[test]
    fn minutes_go_to_the_hours_they_were_in() {
        let mut s = Stats::new();
        s.at_hours(9 * 60 + 10, 25); // 8:45 to 9:10
        assert_eq!((s.hours[8], s.hours[9]), (15, 10));
        s.at_hours(30, 50); // 23:40 to 0:30, over midnight
        assert_eq!((s.hours[23], s.hours[0]), (20, 30));
        s.at_hours(14 * 60, 60);
        assert_eq!((s.hours[13], s.hours[14]), (60, 0));
        assert_eq!(s.hours.iter().sum::<u32>(), 135);
        assert_eq!(s.best_hours(), Some(13)); // 13:00 to 15:00 has 60, 8:00 to 10:00 has 25
        assert_eq!(Stats::new().best_hours(), None);
    }

    #[test]
    fn rounds_finished_and_rounds_stopped() {
        let mut s = Stats::new();
        s.record(20, 25);
        s.record(20, 25);
        s.record(18, 25);
        s.stop(20);
        s.stop(19);
        s.stop(5);
        assert_eq!(s.finished(20, 7), (3, 2));
        assert_eq!(s.finished(20, 1), (2, 1));
        assert_eq!(s.stopped.len(), 3);
        s.stop(200); // trims what is too old
        assert_eq!(s.stopped, vec![(200, 1)]);
    }

    #[test]
    fn the_new_lines_round_trip_and_old_files_still_load() {
        let mut s = Stats::new();
        s.record(100, 25);
        s.at_hours(600, 25);
        s.stop(100);
        let mut t = Stats::new();
        t.load_text(&s.to_text());
        assert_eq!((t.days.clone(), t.hours, t.stopped.clone()), (s.days.clone(), s.hours, s.stopped.clone()));
        let mut old = Stats::new();
        old.load_text("100 2 50\n101 1 25\n");
        assert_eq!(old.days, vec![(100, 2, 50), (101, 1, 25)]);
        assert_eq!(old.best_hours(), None);
        // what an older version reads of a new file: the days, and nothing of the rest
        let older: Vec<(u32, u32, u32)> = s.to_text().lines().filter_map(|l| {
            let v: Vec<u32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| (v[0], v[1], v[2]))
        }).collect();
        assert_eq!(older, s.days);
    }

    #[test]
    fn round_trips_and_trims() {
        let mut s = Stats::new();
        s.record(1, 25);
        s.record(500, 25);
        assert_eq!(s.days.len(), 1); // day 1 is older than 90 days
        let mut t = Stats::new();
        t.load_text(&s.to_text());
        assert_eq!(t.days, s.days);
    }
}
