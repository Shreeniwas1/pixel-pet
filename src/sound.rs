//! Little chip-tune sounds, made up from scratch when they are first needed. Off unless switched on.

use crate::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

/// Mirrors the setting, so anyone can ask for a sound without the state lock.
pub(crate) static ON: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sound {
    Hit,
    Boing,
    Eat,
    Whoosh,
    Land,
    Happy,
    Level,
    Ball,
    Done,
    Ask,
    Click,
}

const ALL: [Sound; 11] = [Sound::Hit, Sound::Boing, Sound::Eat, Sound::Whoosh, Sound::Land, Sound::Happy, Sound::Level, Sound::Ball, Sound::Done, Sound::Ask, Sound::Click];
const RATE: u32 = 22_050;

/// One note: pitch at its start and end (Hz), seconds, loudness, and how much of it is hiss.
#[derive(Clone, Copy)]
struct Note(f32, f32, f32, f32, f32);

fn score(s: Sound) -> &'static [Note] {
    match s {
        Sound::Hit => &[Note(240.0, 70.0, 0.11, 0.5, 0.6)],
        Sound::Boing => &[Note(180.0, 520.0, 0.09, 0.4, 0.0), Note(520.0, 240.0, 0.16, 0.35, 0.0)],
        Sound::Eat => &[Note(300.0, 210.0, 0.05, 0.35, 0.5), Note(1.0, 1.0, 0.07, 0.0, 0.0), Note(320.0, 220.0, 0.05, 0.35, 0.5), Note(1.0, 1.0, 0.07, 0.0, 0.0), Note(280.0, 180.0, 0.06, 0.35, 0.5)],
        Sound::Whoosh => &[Note(900.0, 200.0, 0.22, 0.3, 0.95)],
        Sound::Land => &[Note(130.0, 60.0, 0.12, 0.5, 0.3)],
        Sound::Happy => &[Note(660.0, 700.0, 0.07, 0.3, 0.0), Note(880.0, 930.0, 0.1, 0.3, 0.0)],
        Sound::Level => &[Note(523.0, 523.0, 0.09, 0.35, 0.0), Note(659.0, 659.0, 0.09, 0.35, 0.0), Note(784.0, 784.0, 0.09, 0.35, 0.0), Note(1047.0, 1047.0, 0.24, 0.35, 0.0)],
        Sound::Ball => &[Note(420.0, 180.0, 0.07, 0.4, 0.1)],
        Sound::Done => &[Note(784.0, 784.0, 0.1, 0.32, 0.0), Note(988.0, 988.0, 0.1, 0.32, 0.0), Note(1175.0, 1175.0, 0.2, 0.32, 0.0)],
        Sound::Ask => &[Note(880.0, 880.0, 0.1, 0.32, 0.0), Note(1.0, 1.0, 0.06, 0.0, 0.0), Note(880.0, 1100.0, 0.16, 0.32, 0.0)],
        Sound::Click => &[Note(700.0, 500.0, 0.03, 0.2, 0.0)],
    }
}

/// The sound as a WAV file in memory.
fn render(s: Sound) -> Vec<u8> {
    let mut pcm: Vec<i16> = Vec::new();
    let (mut phase, mut noise) = (0.0f32, 0x1234_5678u32);
    for Note(from, to, secs, loud, hiss) in score(s) {
        let n = (secs * RATE as f32) as u32;
        for i in 0..n {
            let k = i as f32 / n as f32;
            phase = (phase + (from + (to - from) * k) / RATE as f32).fract();
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let tone = if phase < 0.5 { 4.0 * phase - 1.0 } else { 3.0 - 4.0 * phase }; // triangle
            let rough = (noise >> 16) as f32 / 32768.0 - 1.0;
            // quick attack, soft end, so nothing clicks
            let shape = (i as f32 / 60.0).min(1.0) * (1.0 - k).powf(0.7);
            pcm.push(((tone * (1.0 - hiss) + rough * hiss) * shape * loud * 11_000.0) as i16);
        }
    }
    let bytes = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + bytes as usize);
    out.extend(b"RIFF");
    out.extend((36 + bytes).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // plain samples
    out.extend(1u16.to_le_bytes()); // one channel
    out.extend(RATE.to_le_bytes());
    out.extend((RATE * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(bytes.to_le_bytes());
    for s in pcm {
        out.extend(s.to_le_bytes());
    }
    out
}

/// Play it, if sounds are on. A new sound cuts off the last one.
pub(crate) fn play(s: Sound) {
    static QUEUE: OnceLock<Mutex<std::sync::mpsc::Sender<Sound>>> = OnceLock::new();
    if !ON.load(Relaxed) || std::env::var_os("PIXEL_PET_TEST").is_some() {
        return;
    }
    // on a thread of its own: callers hold locks, and stopping the last sound can take a moment
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Sound>();
        std::thread::spawn(move || {
            let made: Vec<Vec<u8>> = ALL.iter().map(|s| render(*s)).collect();
            while let Ok(mut s) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    s = newer;
                }
                let i = ALL.iter().position(|a| *a == s).unwrap_or(0);
                unsafe { PlaySoundW(made[i].as_ptr() as *const u16, null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
            }
        });
        Mutex::new(tx)
    });
    let _ = queue.lock().unwrap().send(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_a_short_quiet_valid_wav() {
        for s in ALL {
            let w = render(s);
            assert_eq!(&w[..4], b"RIFF");
            assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8, "{s:?}");
            let samples: Vec<i32> = w[44..].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as i32).collect();
            let secs = samples.len() as f32 / RATE as f32;
            assert!((0.02..1.0).contains(&secs), "{s:?} lasts {secs}");
            let peak = samples.iter().map(|v| v.abs()).max().unwrap();
            assert!((500..9000).contains(&peak), "{s:?} peaks at {peak}");
            assert!(samples[0].abs() < 200 && samples.last().unwrap().abs() < 600, "{s:?} clicks");
        }
    }
}
