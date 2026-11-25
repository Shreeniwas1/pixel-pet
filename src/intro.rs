//! The first-run tour: a handful of speech bubbles shown one after another on first launch.

/// Marker meaning "the tour has been shown".
pub const DONE: u8 = 255;

pub const STEPS: [&str; 7] = [
    "Hi! I'm your desk pet. Click me to open my hub.",
    "Stroke me with your mouse and I'll purr. Leave me alone and I'll nap.",
    "The hub has tasks, a focus timer, stats and other pets to choose from.",
    "Options changes my theme and size, and what my info bubble shows.",
    "Hotkeys: Ctrl+Alt+T adds a task, Ctrl+Alt+F starts focus, Ctrl+Alt+S shows stats.",
    "Too much of me? Ctrl+Alt+M makes me small and quiet, Ctrl+Alt+H hides me.",
    "Drag me wherever you like. I hide when a game or video goes fullscreen. Enjoy!",
];

/// Half-seconds a step stays on screen.
pub const STEP_TICKS: u32 = 34;
/// Half-seconds between two steps.
const GAP: u32 = 3;
/// Half-seconds of quiet after the tour (no random facts straight after "Enjoy!").
pub const COOLDOWN: u32 = 240;

/// One half-second of the tour. `step` is the next step to show (or `DONE`), `wait` counts the gap
/// before it and, once the tour is over, the quiet time before facts may start.
/// `can_speak` is false while something else owns the screen (reminder, menu, fullscreen game,
/// another message). Returns the step to say now, if any.
pub fn advance(step: &mut u8, wait: &mut u32, can_speak: bool) -> Option<usize> {
    if *step == DONE {
        *wait = wait.saturating_sub(1);
        return None;
    }
    if !can_speak {
        return None; // the tour simply holds its place
    }
    if *wait > 0 {
        *wait -= 1;
        return None;
    }
    let i = *step as usize;
    *step += 1;
    *wait = GAP;
    if *step as usize >= STEPS.len() {
        *step = DONE;
        *wait = COOLDOWN;
    }
    Some(i)
}

/// May a random fact appear? Not during the tour, and not right after it.
pub fn facts_allowed(step: u8, wait: u32) -> bool {
    step == DONE && wait == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_fit_in_a_speech_bubble() {
        assert!(STEPS.len() < DONE as usize);
        for t in STEPS {
            assert!(t.chars().count() <= 90, "too long for the bubble: {t}");
            assert!(!t.is_empty());
        }
    }

    /// Run the tour for `n` half-seconds, saying each step for STEP_TICKS like the real app does.
    fn run(step: &mut u8, wait: &mut u32, n: u32, speaking: &mut u32, said: &mut Vec<usize>) {
        for _ in 0..n {
            *speaking = speaking.saturating_sub(1);
            if let Some(i) = advance(step, wait, *speaking == 0) {
                said.push(i);
                *speaking = STEP_TICKS;
            }
        }
    }

    #[test]
    fn plays_every_step_once_in_order_then_finishes() {
        let (mut step, mut wait, mut speaking, mut said) = (0u8, 6u32, 0u32, Vec::new());
        run(&mut step, &mut wait, 2000, &mut speaking, &mut said);
        assert_eq!(said, (0..STEPS.len()).collect::<Vec<_>>());
        assert_eq!(step, DONE);
    }

    #[test]
    fn holds_its_place_while_something_else_is_speaking() {
        let (mut step, mut wait) = (0u8, 0u32);
        for _ in 0..100 {
            assert_eq!(advance(&mut step, &mut wait, false), None);
        }
        assert_eq!((step, wait), (0, 0), "nothing consumed while blocked");
        assert_eq!(advance(&mut step, &mut wait, true), Some(0));
    }

    #[test]
    fn facts_stay_quiet_during_the_tour_and_for_a_while_after_it() {
        let (mut step, mut wait) = (0u8, 6u32);
        let mut quiet_ticks_after = 0;
        let mut finished = false;
        for _ in 0..3000 {
            if advance(&mut step, &mut wait, true).is_some() && step == DONE {
                finished = true;
            }
            if !finished {
                assert!(!facts_allowed(step, wait), "a fact could pop up mid-tour");
            } else if !facts_allowed(step, wait) {
                quiet_ticks_after += 1;
            }
        }
        assert!(finished);
        assert!(quiet_ticks_after >= COOLDOWN as usize - 1, "cooldown after the last step: {quiet_ticks_after}");
        assert!(facts_allowed(step, wait), "facts come back eventually");
    }

    #[test]
    fn replaying_restarts_from_the_first_step() {
        let (mut step, mut wait) = (0u8, 2u32); // what "Replay intro" sets
        assert_eq!(advance(&mut step, &mut wait, true), None);
        assert_eq!(advance(&mut step, &mut wait, true), None);
        assert_eq!(advance(&mut step, &mut wait, true), Some(0));
    }
}
