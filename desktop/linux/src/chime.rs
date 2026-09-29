//! The timer chime: the web's three rising notes (`chime` in `web/src/lib/timers.svelte.ts`),
//! synthesised to a WAV once and played with PipeWire's `pw-play` (or `paplay`, `aplay`), so
//! there's no audio file to ship and no QtMultimedia to link. `Sound.chime()` from QML.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const RATE: u32 = 44_100;
/// The notes (Hz), each starting 0.18 s after the last, as the web plays them.
const NOTES: [f64; 3] = [880.0, 1175.0, 1568.0];
const STEP: f64 = 0.18;
const LENGTH: f64 = 0.55;

/// The web's gain envelope: 0.0001 → 0.3 in 20 ms, then down to 0.0001 by 0.5 s, both
/// exponential.
fn gain(t: f64) -> f64 {
    let (low, peak) = (0.0001_f64, 0.3_f64);
    if t < 0.0 || t > LENGTH {
        0.0
    } else if t < 0.02 {
        low * (peak / low).powf(t / 0.02)
    } else if t < 0.5 {
        peak * (low / peak).powf((t - 0.02) / 0.48)
    } else {
        low
    }
}

/// The chime as a 16-bit mono WAV.
pub fn wav() -> Vec<u8> {
    let seconds = STEP * (NOTES.len() - 1) as f64 + LENGTH;
    let samples = (seconds * f64::from(RATE)) as usize;
    let mut pcm = Vec::with_capacity(samples * 2);
    for n in 0..samples {
        let t = n as f64 / f64::from(RATE);
        let value: f64 = NOTES
            .iter()
            .enumerate()
            .map(|(i, freq)| {
                let local = t - STEP * i as f64;
                gain(local) * (std::f64::consts::TAU * freq * local).sin()
            })
            .sum();
        let sample = (value.clamp(-1.0, 1.0) * f64::from(i16::MAX)) as i16;
        pcm.extend_from_slice(&sample.to_le_bytes());
    }
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(&pcm);
    out
}

/// The WAV on disk, written once per run in the runtime folder.
fn file() -> Option<&'static PathBuf> {
    static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
    FILE.get_or_init(|| {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
        let path = dir.join("crumb-desktop-chime.wav");
        std::fs::write(&path, wav()).ok()?;
        Some(path)
    })
    .as_ref()
}

/// Plays the chime with the first player that's installed; silent when there's none.
pub fn play() {
    let Some(path) = file() else { return };
    for player in ["pw-play", "paplay", "aplay"] {
        let spawned = Command::new(player)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = spawned {
            // Reap it off the Qt thread
            std::thread::spawn(move || child.wait());
            return;
        }
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Sound = super::SoundRust;

        /// The timer chime.
        #[qinvokable]
        fn chime(self: &Sound);
    }
}

#[derive(Default)]
pub struct SoundRust;

impl qobject::Sound {
    pub fn chime(&self) {
        play();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chime_is_a_short_wav() {
        let wav = wav();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        // 0.91 s of 16-bit mono at 44.1 kHz
        assert_eq!(wav.len(), 44 + (0.91 * 44_100.0) as usize * 2);
        assert!(gain(0.02) > 0.29 && gain(0.6) == 0.0);
    }
}
