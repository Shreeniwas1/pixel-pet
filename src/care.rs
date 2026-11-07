//! Looking after the user: break reminders and system alerts. Pure logic, no Win32.
//! One `tick` = 500 ms.

const TPM: u32 = 120;
const EYES_EVERY: u32 = 20 * TPM; // 20 minutes of active use
const WATER_EVERY: u32 = 45 * TPM;
const STRETCH_EVERY: u32 = 60 * TPM;
const QUIET_GAP: u32 = 5 * TPM; // minimum gap between two reminders
const RESET_IDLE: u64 = 300; // a 5 minute break counts as a real break

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Care {
    Eyes,
    Water,
    Stretch,
}

pub fn care_text(c: Care) -> &'static str {
    match c {
        Care::Eyes => "Eye break! Look at something 20 feet away for 20 seconds.",
        Care::Water => "Water time. Grab a glass and have a few sips.",
        Care::Stretch => "You've been sitting a while. Stand up and stretch for a minute.",
    }
}

pub struct Coach {
    eyes: u32,
    water: u32,
    stretch: u32,
    gap: u32,
}

impl Coach {
    pub const fn new() -> Coach {
        Coach { eyes: 0, water: 0, stretch: 0, gap: QUIET_GAP }
    }

    /// `idle` = seconds since last input. `hold` = don't interrupt (deep focus round running).
    pub fn tick(&mut self, idle: u64, hold: bool) -> Option<Care> {
        self.gap = self.gap.saturating_sub(1);
        if idle >= RESET_IDLE {
            *self = Coach::new();
            return None;
        }
        if idle >= 60 {
            return None; // away from the keyboard: time doesn't count
        }
        self.eyes += 1;
        self.water += 1;
        self.stretch += 1;
        if hold || self.gap > 0 {
            return None;
        }
        let due = if self.stretch >= STRETCH_EVERY {
            self.stretch = 0;
            Care::Stretch
        } else if self.water >= WATER_EVERY {
            self.water = 0;
            Care::Water
        } else if self.eyes >= EYES_EVERY {
            self.eyes = 0;
            Care::Eyes
        } else {
            return None;
        };
        self.gap = QUIET_GAP;
        Some(due)
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

    fn run(c: &mut Coach, ticks: u32, idle: u64) -> Vec<Care> {
        (0..ticks).filter_map(|_| c.tick(idle, false)).collect()
    }

    #[test]
    fn eye_break_after_20_active_minutes_only() {
        let mut c = Coach::new();
        assert!(run(&mut c, EYES_EVERY - 1, 0).is_empty());
        assert_eq!(run(&mut c, 1, 0), vec![Care::Eyes]);
    }

    #[test]
    fn idle_time_does_not_count_and_a_real_break_resets() {
        let mut c = Coach::new();
        run(&mut c, EYES_EVERY - 10, 0);
        run(&mut c, 5000, 120); // away, paused
        assert_eq!(run(&mut c, 10, 0), vec![Care::Eyes]);
        let mut d = Coach::new();
        run(&mut d, EYES_EVERY - 10, 0);
        run(&mut d, 1, 400); // 5+ min break: counters reset
        assert!(run(&mut d, EYES_EVERY - 1, 0).is_empty());
    }

    #[test]
    fn reminders_are_spaced_and_hold_during_deep_focus() {
        let mut c = Coach::new();
        let got = run(&mut c, WATER_EVERY + 10, 0);
        assert!(got.contains(&Care::Eyes) && got.contains(&Care::Water));
        let mut f = Coach::new();
        let held: Vec<Care> = (0..STRETCH_EVERY + 5).filter_map(|_| f.tick(0, true)).collect();
        assert!(held.is_empty());
        assert_eq!(f.tick(0, false), Some(Care::Stretch)); // due the moment focus ends
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
