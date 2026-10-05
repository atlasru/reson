use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f64,
    pub cache_limit_mb: u64,
    pub discord_presence: bool,
    pub discord_application_id: String,
    pub telemetry: bool,
    pub audio_device: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.7,
            cache_limit_mb: 256,
            discord_presence: false,
            discord_application_id: String::new(),
            telemetry: false,
            audio_device: "auto".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !self.volume.is_finite() || !(0.0..=1.0).contains(&self.volume) {
            return Err(Error::Invalid("Volume must be between 0 and 100%".into()));
        }
        if !(32..=2048).contains(&self.cache_limit_mb) {
            return Err(Error::Invalid("Cache limit must be 32–2048 MB".into()));
        }
        if !self.discord_application_id.is_empty()
            && (self.discord_application_id.len() > 24
                || self.discord_application_id.len() < 16
                || !self
                    .discord_application_id
                    .bytes()
                    .all(|b| b.is_ascii_digit()))
        {
            return Err(Error::Invalid(
                "Enter a valid Discord application ID".into(),
            ));
        }
        if self.audio_device.len() > 256 || self.audio_device.contains('\0') {
            return Err(Error::Invalid("Invalid audio device".into()));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation() {
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.volume = f64::NAN;
        assert!(s.validate().is_err());
        s.volume = 0.5;
        s.cache_limit_mb = 1;
        assert!(s.validate().is_err());
    }
}
