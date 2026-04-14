//! # wave module

use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wave {
    pub samples: Vec<f64>,
    sample_rate: u32,
    channels: u16,
}

impl Wave {
    pub fn new(samples: Vec<f64>, sample_rate: u32, channels: u16) -> Result<Self, WaveError> {
        if sample_rate == 0 {
            return Err(WaveError::InvalidSampleRate(sample_rate));
        }

        if channels == 0 {
            return Err(WaveError::InvalidChannels(channels));
        }

        Ok(Self {
            samples,
            sample_rate,
            channels,
        })
    }
}

#[derive(Debug, Error)]
pub enum WaveError {
    #[error("Invalid sample_rate, {0:?}")]
    InvalidSampleRate(u32),

    #[error("Invalid channels, {0:?}")]
    InvalidChannels(u16),
}
