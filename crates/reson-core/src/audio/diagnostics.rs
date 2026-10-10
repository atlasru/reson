//! Bounded PCM inspection for real native playback validation. Never persists audio.
use crate::error::{Error, Result};
use std::{fs::File, io::Read, path::Path};

pub struct PcmEvidence {
    pub samples: usize,
    pub rms: f64,
    pub peak: f32,
}
pub fn inspect_pcm(path: &Path) -> Result<PcmEvidence> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(16 * 1024 * 1024)
        .read_to_end(&mut bytes)?;
    let mut square_sum = 0.0;
    let mut peak = 0.0f32;
    let mut samples = 0;
    for chunk in bytes.as_chunks::<4>().0 {
        let value = f32::from_le_bytes(*chunk);
        if !value.is_finite() {
            return Err(Error::Audio("Invalid PCM samples".into()));
        }
        square_sum += f64::from(value) * f64::from(value);
        peak = peak.max(value.abs());
        samples += 1;
    }
    let rms = (square_sum / samples.max(1) as f64).sqrt();
    if samples < 1000 || rms < 0.00001 {
        return Err(Error::Audio(
            "Native output contained no audible PCM".into(),
        ));
    }
    Ok(PcmEvidence { samples, rms, peak })
}
