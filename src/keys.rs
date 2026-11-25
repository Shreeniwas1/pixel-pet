//! The shortcut keys: Ctrl+Alt plus a letter or digit of the user's choice.

use crate::prelude::*;

pub(crate) struct Action {
    pub id: usize, // what WM_HOTKEY carries
    pub name: &'static str,
    pub key: u8, // unless the user picks another
}

const fn action(id: usize, name: &'static str, key: u8) -> Action {
    Action { id, name, key }
}

pub(crate) static ACTIONS: [Action; 11] = [
    action(1, "ADD A TASK", b'T'),
    action(2, "FOCUS MODE", b'F'),
    action(3, "STATS", b'S'),
    action(4, "MINIMAL MODE", b'M'),
    action(5, "HIDE OR SHOW", b'H'),
    action(6, "PET HUB", b'P'),
    action(7, "FEED", b'E'),
    action(9, "CALCULATOR", b'K'),
    action(10, "NOTES", b'N'),
    action(11, "VS CODE", b'D'),
    action(CLOSE, "CLOSE", b'Q'),
];

pub(crate) const CLOSE: usize = 8;

/// The chosen key of every action, in the order of `ACTIONS`; 0 = none.
pub(crate) type Keys = [u8; ACTIONS.len()];

pub(crate) const DEFAULT: Keys = {
    let mut k = [0; ACTIONS.len()];
    let mut i = 0;
    while i < k.len() {
        k[i] = ACTIONS[i].key;
        i += 1;
    }
    k
};

/// Keys that may be chosen.
pub(crate) fn allowed(k: u8) -> bool {
    k.is_ascii_uppercase() || k.is_ascii_digit()
}

/// Give action `i` this key. Whoever had it gets the old one in exchange.
pub(crate) fn choose(keys: &mut Keys, i: usize, k: u8) {
    if !allowed(k) {
        return;
    }
    if let Some(other) = (0..keys.len()).find(|o| *o != i && keys[*o] == k) {
        keys[other] = keys[i];
    }
    keys[i] = k;
}

pub(crate) fn to_text(keys: &Keys) -> String {
    keys.iter().map(|k| if allowed(*k) { *k as char } else { '-' }).collect()
}

pub(crate) fn from_text(t: &str) -> Keys {
    let mut keys = DEFAULT;
    for (k, c) in keys.iter_mut().zip(t.bytes()) {
        if c == b'-' || allowed(c) {
            *k = if c == b'-' { 0 } else { c };
        }
    }
    keys
}

/// How a key is shown.
pub(crate) fn label(k: u8) -> String {
    if allowed(k) { format!("CTL+ALT+{}", k as char) } else { "NONE".into() }
}

/// The key of the action with this id, as shown on its tile.
pub(crate) fn label_of(keys: &Keys, id: usize) -> String {
    ACTIONS.iter().position(|a| a.id == id).map_or(String::new(), |i| label(keys[i]))
}

pub(crate) unsafe fn unregister(hwnd: HWND) {
    for a in ACTIONS.iter() {
        UnregisterHotKey(hwnd, a.id as i32);
    }
}

/// Ask Windows for the keys; ones another program has are skipped. `shortcut`: the Start menu
/// shortcut listens for the close key.
pub(crate) unsafe fn register(hwnd: HWND, keys: &Keys, shortcut: bool) {
    unregister(hwnd);
    for (a, k) in ACTIONS.iter().zip(keys) {
        if allowed(*k) && !(a.id == CLOSE && shortcut) {
            RegisterHotKey(hwnd, a.id as i32, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, *k as u32);
        }
    }
}

/// The letter or digit being held down together with Ctrl and Alt.
pub(crate) unsafe fn pressed() -> Option<u8> {
    let down = |k: u16| GetAsyncKeyState(k as i32) < 0;
    if !down(VK_CONTROL) || !down(VK_MENU) {
        return None;
    }
    (b'0'..=b'Z').filter(|k| allowed(*k)).find(|k| down(*k as u16))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_actions_start_with_the_same_key_or_id() {
        for (i, a) in ACTIONS.iter().enumerate() {
            assert!(allowed(a.key) && a.name.len() <= 14);
            assert!(ACTIONS[..i].iter().all(|b| b.key != a.key && b.id != a.id), "{}", a.name);
        }
    }

    #[test]
    fn a_taken_key_is_exchanged() {
        let mut k = DEFAULT;
        choose(&mut k, 0, b'F'); // task takes the focus key
        assert_eq!((k[0], k[1]), (b'F', b'T'));
        choose(&mut k, 0, b'7');
        assert_eq!((k[0], k[1]), (b'7', b'T'));
        choose(&mut k, 0, b'!');
        assert_eq!(k[0], b'7');
    }

    #[test]
    fn saved_and_loaded_again_they_are_the_same() {
        let mut k = DEFAULT;
        choose(&mut k, 2, b'9');
        k[4] = 0;
        assert_eq!(from_text(&to_text(&k)), k);
        assert_eq!(from_text(""), DEFAULT);
        assert_eq!(from_text("??"), DEFAULT);
        assert_eq!(label(0), "NONE");
        assert_eq!(label_of(&DEFAULT, 9), "CTL+ALT+K");
    }
}
