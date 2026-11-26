//! Looking after the user: break reminders and system alerts. Pure logic, no Win32.
//! One `tick` = 500 ms.

const TPM: u32 = 120;
const QUIET_GAP: u32 = 5 * TPM; // minimum gap between two reminders
const RESET_IDLE: u64 = 300; // a 5 minute break counts as a real break
/// Ticks of looking into the distance.
pub const EYE_REST: u32 = 40;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Care {
    Eyes,
    Water,
    Stretch,
    Posture,
}

/// In the order of the settings.
pub const CARES: [Care; 4] = [Care::Eyes, Care::Water, Care::Stretch, Care::Posture];
pub const NAMES: [&str; 4] = ["REST YOUR EYES", "DRINK WATER", "STAND UP", "SIT UP STRAIGHT"];
/// Minutes between two, 0 = never.
pub const EVERY: [[u32; 5]; 4] = [[0, 10, 20, 30, 45], [0, 30, 45, 60, 90], [0, 30, 45, 60, 90], [0, 15, 30, 45, 60]];
/// Which of them unless the user picks another.
pub const USUAL: [usize; 4] = [2, 2, 3, 3];

pub fn care_text(c: Care, n: u32) -> &'static str {
    let pick = |l: [&'static str; 3]| l[n as usize % 3];
    match c {
        Care::Eyes => "Eye break! Look at something 20 feet away for 20 seconds.",
        Care::Water => pick(["Water time. Grab a glass and have a few sips.", "Thirsty? A glass of water would do you good.", "Sip break! Your keyboard can wait."]),
        Care::Stretch => pick(["You've been sitting a while. Stand up and stretch for a minute.", "Legs! Remember those? Take them for a short walk.", "Up you get. Roll your shoulders, shake your hands."]),
        Care::Posture => pick(["Posture check: back straight, shoulders down, feet on the floor.", "Are you shrimping again? Sit up, chin back.", "Screen at eye level, shoulders loose. There you go."]),
    }
}

/// The chosen minutes as the settings file has them, and back.
pub fn to_text(every: &[usize; 4]) -> String {
    every.map(|e| e.to_string()).join(",")
}

pub fn from_text(t: &str) -> [usize; 4] {
    let mut out = USUAL;
    for (o, v) in out.iter_mut().zip(t.split(',')) {
        if let Ok(v) = v.trim().parse::<usize>() {
            *o = v.min(EVERY[0].len() - 1);
        }
    }
    out
}

/// "20260101:3": the day and the glasses. Another day starts from none.
pub fn glasses(t: &str, day: u32) -> u32 {
    t.split_once(':').filter(|(d, _)| d.parse() == Ok(day)).and_then(|(_, n)| n.parse().ok()).unwrap_or(0)
}

pub struct Coach {
    felt: [u32; 4], // ticks of work since the last one
    gap: u32,
    pub said: u32,
}

impl Coach {
    pub const fn new() -> Coach {
        Coach { felt: [0; 4], gap: QUIET_GAP, said: 0 }
    }

    /// `idle` = seconds since last input. `hold` = don't interrupt. `every` = minutes for each of `CARES`.
    pub fn tick(&mut self, idle: u64, hold: bool, every: [u32; 4]) -> Option<Care> {
        self.gap = self.gap.saturating_sub(1);
        if idle >= RESET_IDLE {
            *self = Coach { said: self.said, ..Coach::new() };
            return None;
        }
        if idle >= 60 {
            return None; // away from the keyboard: time doesn't count
        }
        for (f, e) in self.felt.iter_mut().zip(every) {
            *f = if e == 0 { 0 } else { *f + 1 };
        }
        if hold || self.gap > 0 {
            return None;
        }
        let due = |c: Care| {
            let i = CARES.iter().position(|x| *x == c).unwrap_or(0);
            every[i] > 0 && self.felt[i] >= every[i] * TPM
        };
        let c = [Care::Stretch, Care::Water, Care::Eyes, Care::Posture].into_iter().find(|c| due(*c))?;
        self.done(c);
        self.gap = QUIET_GAP;
        self.said += 1;
        Some(c)
    }

    /// It has been done, whoever asked for it. Standing up is good for the back and the eyes too.
    pub fn done(&mut self, c: Care) {
        self.felt[CARES.iter().position(|x| *x == c).unwrap_or(0)] = 0;
        if c == Care::Stretch {
            self.felt[0] = 0;
            self.felt[3] = 0;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sys {
    Cpu(u32),
    Ram(u32),
    Battery(u32),
}

pub fn sys_text(a: Sys) -> String {
    match a {
        Sys::Cpu(p) => format!("CPU has been at {p}% for a while. Something is working hard."),
        Sys::Ram(p) => format!("Memory is {p}% full. Closing a few apps or tabs would help."),
        Sys::Battery(p) if p <= 10 => format!("Battery at {p}%! Plug in now."),
        Sys::Battery(p) => format!("Battery at {p}%. Time to find a charger."),
    }
}

pub struct Watch {
    cpu_hi: u32,
    ram_hi: u32,
    cpu_cd: u32,
    ram_cd: u32,
    bat_level: u8, // 0 none said, 1 said low, 2 said critical
}

impl Watch {
    pub const fn new() -> Watch {
        Watch { cpu_hi: 0, ram_hi: 0, cpu_cd: 0, ram_cd: 0, bat_level: 0 }
    }

    /// `cpu`/`ram` in percent; `battery` = (percent, on AC power) or None on a desktop.
    pub fn tick(&mut self, cpu: u32, ram: u32, battery: Option<(u32, bool)>) -> Option<Sys> {
        self.cpu_cd = self.cpu_cd.saturating_sub(1);
        self.ram_cd = self.ram_cd.saturating_sub(1);
        self.cpu_hi = if cpu >= 90 { self.cpu_hi + 1 } else { 0 };
        self.ram_hi = if ram >= 90 { self.ram_hi + 1 } else { 0 };

        if let Some((pct, ac)) = battery {
            if ac || pct >= 30 {
                self.bat_level = 0;
            } else if pct <= 10 && self.bat_level < 2 {
                self.bat_level = 2;
                return Some(Sys::Battery(pct));
            } else if pct <= 20 && self.bat_level < 1 {
                self.bat_level = 1;
                return Some(Sys::Battery(pct));
            }
        }
        if self.cpu_hi >= 60 && self.cpu_cd == 0 {
            self.cpu_cd = 10 * TPM;
            return Some(Sys::Cpu(cpu));
        }
        if self.ram_hi >= 60 && self.ram_cd == 0 {
            self.ram_cd = 20 * TPM;
            return Some(Sys::Ram(ram));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [u32; 4] = [20, 45, 60, 30];
    const EYES: [u32; 4] = [20, 0, 0, 0];

    fn run(c: &mut Coach, ticks: u32, idle: u64, every: [u32; 4]) -> Vec<Care> {
        (0..ticks).filter_map(|_| c.tick(idle, false, every)).collect()
    }

    #[test]
    fn eye_break_after_20_active_minutes_only() {
        let mut c = Coach::new();
        assert!(run(&mut c, 20 * TPM - 1, 0, ALL).is_empty());
        assert_eq!(run(&mut c, 1, 0, ALL), vec![Care::Eyes]);
    }

    #[test]
    fn idle_time_does_not_count_and_a_real_break_resets() {
        let mut c = Coach::new();
        run(&mut c, 20 * TPM - 10, 0, EYES);
        run(&mut c, 5000, 120, EYES); // away, paused
        assert_eq!(run(&mut c, 10, 0, EYES), vec![Care::Eyes]);
        let mut d = Coach::new();
        run(&mut d, 20 * TPM - 10, 0, EYES);
        run(&mut d, 1, 400, EYES); // 5+ min break: counters reset
        assert!(run(&mut d, 20 * TPM - 1, 0, EYES).is_empty());
    }

    #[test]
    fn reminders_are_spaced_and_hold_during_deep_focus() {
        let mut c = Coach::new();
        let mut last = None;
        for t in 0..3 * 60 * TPM {
            if c.tick(0, false, ALL).is_some() {
                assert!(last.map_or(true, |l| t - l >= QUIET_GAP), "too close together");
                last = Some(t);
            }
        }
        let mut c = Coach::new();
        let got = run(&mut c, 3 * 60 * TPM, 0, ALL);
        for k in CARES {
            assert!(got.contains(&k), "{k:?} never came");
        }
        let mut f = Coach::new();
        assert!((0..60 * TPM + 5).all(|_| f.tick(0, true, ALL).is_none()));
        assert_eq!(f.tick(0, false, ALL), Some(Care::Stretch)); // due the moment focus ends
    }

    #[test]
    fn each_can_be_switched_off_and_has_its_own_time() {
        for (i, k) in CARES.iter().enumerate() {
            let mut only = [0; 4];
            only[i] = 15;
            let got = run(&mut Coach::new(), 40 * TPM, 0, only);
            assert_eq!(got, vec![*k, *k], "only {k:?}");
        }
        assert!(run(&mut Coach::new(), 600 * TPM, 0, [0; 4]).is_empty());
    }

    #[test]
    fn standing_up_rests_the_eyes_and_the_back_too() {
        let mut c = Coach::new();
        run(&mut c, 19 * TPM, 0, EYES);
        c.done(Care::Stretch);
        assert!(run(&mut c, 19 * TPM, 0, EYES).is_empty());
    }

    #[test]
    fn settings_and_glasses_are_read_back() {
        for e in [USUAL, [0; 4], [4, 3, 2, 1]] {
            assert_eq!(from_text(&to_text(&e)), e);
        }
        assert_eq!(from_text(""), USUAL);
        assert_eq!(from_text("9,x"), [4, USUAL[1], USUAL[2], USUAL[3]]);
        assert_eq!((glasses("20260101:3", 20260101), glasses("20260101:3", 20260102), glasses("junk", 1)), (3, 0, 0));
        for c in CARES {
            for n in 0..4 {
                assert!(care_text(c, n).chars().count() <= 90, "{}", care_text(c, n));
            }
        }
        assert!(EVERY.iter().zip(USUAL).all(|(e, u)| e[0] == 0 && e[u] > 0));
    }

    #[test]
    fn cpu_alert_needs_30s_and_then_cools_down() {
        let mut w = Watch::new();
        let hits: Vec<_> = (0..59).filter_map(|_| w.tick(95, 40, None)).collect();
        assert!(hits.is_empty());
        assert_eq!(w.tick(95, 40, None), Some(Sys::Cpu(95)));
        assert!((0..600).filter_map(|_| w.tick(95, 40, None)).next().is_none());
    }

    #[test]
    fn a_spike_resets_the_cpu_count() {
        let mut w = Watch::new();
        for _ in 0..50 {
            w.tick(99, 10, None);
        }
        w.tick(10, 10, None);
        assert!((0..50).filter_map(|_| w.tick(99, 10, None)).next().is_none());
    }

    #[test]
    fn battery_warns_once_per_level_and_rearms_on_charge() {
        let mut w = Watch::new();
        assert_eq!(w.tick(5, 5, Some((19, false))), Some(Sys::Battery(19)));
        assert_eq!(w.tick(5, 5, Some((18, false))), None);
        assert_eq!(w.tick(5, 5, Some((9, false))), Some(Sys::Battery(9)));
        assert_eq!(w.tick(5, 5, Some((50, true))), None);
        assert_eq!(w.tick(5, 5, Some((15, false))), Some(Sys::Battery(15)));
        assert_eq!(w.tick(5, 5, Some((15, true))), None); // charging: silent
    }
}
