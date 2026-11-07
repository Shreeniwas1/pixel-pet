//! Focus history: sessions and minutes per day, streaks, daily goal, weekly recap.
//! Days are "days since 1970-01-01" in local time. Pure logic, no Win32.

const KEEP_DAYS: u32 = 90;
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub struct Stats {
    pub days: Vec<(u32, u32, u32)>, // (day, sessions, minutes), ascending
    pub goal: u32,                  // sessions per day
}

pub struct Week {
    pub sessions: u32,
    pub minutes: u32,
    pub active_days: u32,
    pub best: Option<(u32, u32)>, // (day, sessions)
}

impl Stats {
    pub const fn new() -> Stats {
        Stats { days: Vec::new(), goal: 4 }
    }

    /// Log a finished focus session. Returns true if this one just reached the daily goal.
    pub fn record(&mut self, day: u32, minutes: u32) -> bool {
        match self.days.last_mut() {
            Some(d) if d.0 == day => {
                d.1 += 1;
                d.2 += minutes;
            }
            _ => self.days.push((day, 1, minutes)),
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

    pub fn recap(&self, day: u32) -> String {
        let w = self.week(day);
        let best = w.best.map_or("-".to_string(), |(d, s)| format!("{} ({} sessions)", WEEKDAYS[((d + 4) % 7) as usize], s));
        format!(
            "Last 7 days\n\nFocus sessions:  {}\nFocus time:  {}h {:02}m\nActive days:  {} of 7\nBest day:  {}\nCurrent streak:  {} day(s)\nDaily goal:  {} sessions",
            w.sessions,
            w.minutes / 60,
            w.minutes % 60,
            w.active_days,
            best,
            self.streak(day),
            self.goal
        )
    }

    pub fn to_text(&self) -> String {
        self.days.iter().map(|d| format!("{} {} {}\n", d.0, d.1, d.2)).collect()
    }

    pub fn load_text(&mut self, txt: &str) {
        self.days = txt
            .lines()
            .filter_map(|l| {
                let v: Vec<u32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.len() == 3).then(|| (v[0], v[1], v[2]))
            })
            .collect();
        self.days.sort();
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
    fn week_summary_and_recap() {
        let mut s = Stats::new();
        s.record(10, 25); // too old for the window ending day 20
        s.record(18, 25);
        s.record(18, 50);
        s.record(20, 25);
        let w = s.week(20);
        assert_eq!((w.sessions, w.minutes, w.active_days), (3, 100, 2));
        assert_eq!(w.best, Some((18, 2)));
        assert!(s.recap(20).contains("1h 40m"));
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
