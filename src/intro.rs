//! The first-run tour: a handful of speech bubbles shown one after another on first launch.

/// Marker meaning "the tour has been shown".
pub const DONE: u8 = 255;

pub const STEPS: [&str; 5] = [
    "Hi! I'm your desk pet. Click me to open my hub.",
    "Stroke me with your mouse and I'll purr. Leave me alone and I'll nap.",
    "The hub has tasks, a focus timer, stats and other pets to choose from.",
    "Hotkeys: Ctrl+Alt+T adds a task, Ctrl+Alt+F starts focus, Ctrl+Alt+S shows stats.",
    "Drag me wherever you like. I hide when a game or video goes fullscreen. Enjoy!",
];

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
}
