//! The pet's inner life: happiness, energy, sleep, and what it says about them.
//! Pure logic (no Win32) so it can be unit tested. One `tick` = 500 ms.

const TPM: f32 = 120.0; // ticks per minute
const NAP_TICKS: u32 = 120; // a 60 s nap
const NAP_GAIN_PER_MIN: f32 = 45.0; // energy per minute while napping
const HAPPY_FLOOR: f32 = 15.0; // never fully miserable: gentle, not guilt-tripping

/// What the pet is visibly doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Idle,
    Sleep,
    Cheer,
    Love,
    Stretch,
    Tired,
    Sad,
}

/// Things worth saying out loud.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ev {
    Wake,
    Morning,
    Focus,
    Task,
    Pet,
    Late,
    Tired,
    Sad,
    Back,
}

pub struct Mood {
    pub happy: f32,  // 0..100
    pub energy: f32, // 0..100
    pub last_run: i64, // local minutes, for "welcome back"
    tick: u32,
    cheer: u32,
    love: u32,
    stretch: u32,
    pet_cd: u32,
    pet_n: u32,
    asleep: bool,
    asleep_ticks: u32,
    nap: u32,
    tired_said: bool,
    sad_said: bool,
    late_said: bool,
}

impl Mood {
    pub const fn new() -> Mood {
        Mood {
            happy: 70.0,
            energy: 80.0,
            last_run: 0,
            tick: 0,
            cheer: 0,
            love: 0,
            stretch: 0,
            pet_cd: 0,
            pet_n: 0,
            asleep: false,
            asleep_ticks: 0,
            nap: 0,
            tired_said: false,
            sad_said: false,
            late_said: false,
        }
    }

    /// Call once at startup. `now` is local minutes; returns a greeting if we were away a while.
    pub fn on_launch(&mut self, now: i64) -> Option<Ev> {
        let gap = if self.last_run > 0 { now - self.last_run } else { 0 };
        self.last_run = now;
        if gap < 0 {
            return None;
        }
        // away overnight: well rested, a little lonely
        if gap >= 360 {
            self.energy = self.energy.max(85.0);
            self.happy = (self.happy - (gap as f32 / 720.0).min(20.0)).max(HAPPY_FLOOR);
            self.stretch = 6;
            return Some(Ev::Back);
        }
        None
    }

    /// Advance half a second. `idle` = seconds since the user last touched mouse/keyboard.
    pub fn tick(&mut self, idle: u64, hour: u32, focusing: bool) -> Option<Ev> {
        self.tick += 1;
        for t in [&mut self.cheer, &mut self.love, &mut self.stretch, &mut self.pet_cd] {
            *t = t.saturating_sub(1);
        }
        let night = hour >= 23 || hour < 6;
        let napping = self.nap > 0;
        self.nap = self.nap.saturating_sub(1);
        if self.energy >= 100.0 {
            self.nap = 0; // fully rested: wake up early
        }
        let sleepy = napping || (!focusing && (idle >= 300 || (night && idle >= 60)));
        let mut ev = None;

        if sleepy {
            if !self.asleep {
                self.asleep = true;
                self.asleep_ticks = 0;
            }
            self.asleep_ticks += 1;
            // a nap recharges ~30x faster than ordinary idle sleep
            let per_min = if napping { NAP_GAIN_PER_MIN } else { 1.5 };
            self.energy = (self.energy + per_min / TPM).min(100.0);
        } else {
            if self.asleep {
                self.asleep = false;
                if self.asleep_ticks >= 120 {
                    self.stretch = 6;
                    ev = Some(if (5..12).contains(&hour) { Ev::Morning } else { Ev::Wake });
                }
            }
            self.energy = (self.energy - 0.25 / TPM).max(0.0);
            self.happy = (self.happy - 0.1 / TPM).max(HAPPY_FLOOR);
        }

        if self.energy >= 40.0 {
            self.tired_said = false;
        } else if self.energy < 25.0 && !self.tired_said && !sleepy {
            self.tired_said = true;
            ev = ev.or(Some(Ev::Tired));
        }
        if self.happy >= 45.0 {
            self.sad_said = false;
        } else if self.happy < 30.0 && !self.sad_said && !sleepy {
            self.sad_said = true;
            ev = ev.or(Some(Ev::Sad));
        }
        if (6..12).contains(&hour) {
            self.late_said = false;
        } else if hour < 5 && !self.late_said && !sleepy && idle < 60 {
            self.late_said = true;
            ev = ev.or(Some(Ev::Late));
        }
        ev
    }

    /// Put the pet down for a short nap that restores energy fast. Returns false (and does
    /// nothing) if it is already napping or already full of energy.
    pub fn start_nap(&mut self) -> bool {
        if self.nap > 0 || self.energy >= 100.0 {
            return false;
        }
        self.nap = NAP_TICKS;
        true
    }

    #[cfg(test)]
    pub fn napping(&self) -> bool {
        self.nap > 0
    }

    /// Mouse stroked across the pet. Returns an event only now and then, so it never nags.
    pub fn on_petted(&mut self) -> Option<Ev> {
        if self.pet_cd > 0 {
            return None;
        }
        self.pet_cd = 10;
        self.love = 10;
        self.pet_n += 1;
        self.happy = (self.happy + 4.0).min(100.0);
        (self.pet_n % 4 == 1).then_some(Ev::Pet)
    }

    pub fn on_focus_done(&mut self) -> Ev {
        self.happy = (self.happy + 12.0).min(100.0);
        self.energy = (self.energy - 3.0).max(0.0);
        self.cheer = 10;
        Ev::Focus
    }

    pub fn on_task_done(&mut self) -> Ev {
        self.happy = (self.happy + 6.0).min(100.0);
        self.cheer = 6;
        Ev::Task
    }

    pub fn act(&self) -> Act {
        if self.asleep {
            Act::Sleep
        } else if self.cheer > 0 {
            Act::Cheer
        } else if self.love > 0 {
            Act::Love
        } else if self.stretch > 0 {
            Act::Stretch
        } else if self.energy < 25.0 {
            Act::Tired
        } else if self.happy < 30.0 {
            Act::Sad
        } else {
            Act::Idle
        }
    }

    pub fn label(&self) -> &'static str {
        match self.happy {
            h if h >= 75.0 => "Happy",
            h if h >= 45.0 => "Content",
            h if h >= 30.0 => "Meh",
            _ => "Lonely",
        }
    }
}

/// What each pet says. Index = pet species (Cat, Dog, Claude, Agent); a little personality each.
pub fn line(pet: usize, ev: Ev, n: u32) -> &'static str {
    let alt = n % 2 == 1;
    match (pet, ev) {
        (0, Ev::Wake) => "Mrrp. Fine. I was up anyway.",
        (1, Ev::Wake) => "You're back! You're back! You're back!",
        (2, Ev::Wake) => "Welcome back. Where were we?",
        (_, Ev::Wake) => "Systems online. Welcome back.",

        (0, Ev::Morning) => "Morning. Breakfast first, then work.",
        (1, Ev::Morning) => "Good morning! Today is going to be great!",
        (2, Ev::Morning) => "Good morning. What shall we make today?",
        (_, Ev::Morning) => "Good morning. Agenda loaded.",

        (0, Ev::Focus) => "Not bad, human. Not bad at all.",
        (1, Ev::Focus) => "You did it! Best focus ever!",
        (2, Ev::Focus) => "A clean round. Take a breath.",
        (_, Ev::Focus) => "Focus block complete. Efficiency up.",

        (0, Ev::Task) => "Checked off. I approve.",
        (1, Ev::Task) => "Task done! Good human!",
        (2, Ev::Task) => "One less thing to carry.",
        (_, Ev::Task) => "Task closed. Nice.",

        (0, Ev::Pet) => if alt { "Purr." } else { "Okay. A little more." },
        (1, Ev::Pet) => if alt { "Best day ever!" } else { "More pets! Please!" },
        (2, Ev::Pet) => if alt { "That's kind of you." } else { "Thank you. Genuinely." },
        (_, Ev::Pet) => if alt { "Affection logged." } else { "Morale improved." },

        (0, Ev::Late) => "It's late. Even cats sleep, you know.",
        (1, Ev::Late) => "It's so late! Sleepy time soon?",
        (2, Ev::Late) => "It's very late. The work will keep until morning.",
        (_, Ev::Late) => "Late hour detected. Rest is recommended.",

        (0, Ev::Tired) => "I need a nap. You need one too.",
        (1, Ev::Tired) => "I'm sooo tired. Walk break?",
        (2, Ev::Tired) => "We've been at it a while. A short break?",
        (_, Ev::Tired) => "Energy low. Break suggested.",

        (0, Ev::Sad) => "...Hello? I exist, you know.",
        (1, Ev::Sad) => "Psst... pet me?",
        (2, Ev::Sad) => "I missed you a little. Say hi?",
        (_, Ev::Sad) => "Social battery low. Interact?",

        (0, Ev::Back) => "You were gone. I did not miss you.",
        (1, Ev::Back) => "You came back! I waited all day!",
        (2, Ev::Back) => "Welcome back. I kept your seat warm.",
        (_, Ev::Back) => "Welcome back. Resuming normal operation.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &mut Mood, ticks: u32, idle: u64, hour: u32) -> Vec<Ev> {
        (0..ticks).filter_map(|_| m.tick(idle, hour, false)).collect()
    }

    #[test]
    fn sleeps_when_idle_and_wakes_with_greeting() {
        let mut m = Mood::new();
        run(&mut m, 5, 400, 14);
        assert_eq!(m.act(), Act::Sleep);
        run(&mut m, 200, 400, 14);
        let evs = run(&mut m, 1, 0, 14);
        assert_eq!(evs, vec![Ev::Wake]);
        assert_eq!(m.act(), Act::Stretch);
    }

    #[test]
    fn morning_greeting_in_the_morning() {
        let mut m = Mood::new();
        run(&mut m, 200, 400, 8);
        assert_eq!(run(&mut m, 1, 0, 8), vec![Ev::Morning]);
    }

    #[test]
    fn short_nap_is_silent() {
        let mut m = Mood::new();
        run(&mut m, 20, 400, 14);
        assert!(run(&mut m, 1, 0, 14).is_empty());
    }

    #[test]
    fn sleeps_sooner_at_night_but_never_while_focusing() {
        let mut m = Mood::new();
        run(&mut m, 2, 90, 2);
        assert_eq!(m.act(), Act::Sleep);
        let mut f = Mood::new();
        for _ in 0..4 {
            f.tick(900, 14, true);
        }
        assert_ne!(f.act(), Act::Sleep);
    }

    #[test]
    fn petting_has_cooldown_and_raises_happiness() {
        let mut m = Mood::new();
        let before = m.happy;
        assert_eq!(m.on_petted(), Some(Ev::Pet)); // first pet speaks
        assert_eq!(m.act(), Act::Love);
        assert_eq!(m.on_petted(), None); // cooldown
        assert!(m.happy > before);
        run(&mut m, 12, 0, 14);
        assert_eq!(m.on_petted(), None); // second pet is silent
    }

    #[test]
    fn nap_restores_energy_fast_even_while_active() {
        let mut m = Mood::new();
        m.energy = 20.0;
        assert!(m.start_nap());
        assert!(!m.start_nap()); // already napping
        run(&mut m, 4, 0, 14); // user is active (idle 0), yet it falls asleep
        assert_eq!(m.act(), Act::Sleep);
        run(&mut m, NAP_TICKS, 0, 14);
        assert!(m.energy > 60.0, "energy {}", m.energy);
        assert!(!m.napping());
        assert_ne!(m.act(), Act::Sleep); // awake again once the nap is over
    }

    #[test]
    fn no_nap_when_already_full() {
        let mut m = Mood::new();
        m.energy = 100.0;
        assert!(!m.start_nap());
    }

    #[test]
    fn happiness_decays_but_has_a_floor() {
        let mut m = Mood::new();
        run(&mut m, 120 * 60 * 24, 0, 14);
        assert!(m.happy >= HAPPY_FLOOR && m.happy < 30.0);
    }

    #[test]
    fn long_work_makes_it_tired_once() {
        let mut m = Mood::new();
        m.energy = 26.0;
        let evs = run(&mut m, 120 * 10, 0, 14);
        assert_eq!(evs.iter().filter(|e| **e == Ev::Tired).count(), 1);
    }

    #[test]
    fn rewards_for_focus_and_tasks() {
        let mut m = Mood::new();
        m.happy = 50.0;
        assert_eq!(m.on_focus_done(), Ev::Focus);
        assert_eq!(m.act(), Act::Cheer);
        assert!(m.happy >= 62.0);
        assert_eq!(m.on_task_done(), Ev::Task);
    }

    #[test]
    fn welcome_back_after_long_absence() {
        let mut m = Mood::new();
        m.last_run = 1000;
        assert_eq!(m.on_launch(1000 + 600), Some(Ev::Back));
        let mut n = Mood::new();
        n.last_run = 1000;
        assert_eq!(n.on_launch(1010), None);
    }
}
