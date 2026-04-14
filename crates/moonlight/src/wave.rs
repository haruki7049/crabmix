//! # wave module

use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wave {
    pub samples: Vec<f64>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait Waveable {
    type Error: std::error::Error;

    fn mix<F>(&self, other: &Self, mixer_fn: F) -> Result<Self, Self::Error>
    where
        Self: Sized,
        F: Fn(f64, f64) -> f64;
}

impl Waveable for Wave {
    type Error = WaveError;

    fn mix<F>(&self, other: &Self, mixer_fn: F) -> Result<Self, Self::Error>
    where
        F: Fn(f64, f64) -> f64,
    {
        validate_equal_sample_rates(self, other)?;
        validate_equal_channels(self, other)?;
        validate_equal_lengths(self, other)?;
        validate_not_empty(self)?;

        let sample_rate = self.sample_rate;
        let channels = self.channels;
        let mut samples = Vec::new();
        for i in 0..self.samples.len() {
            let left = self.samples[i];
            let right = other.samples[i];
            let result = mixer_fn(left, right);

            samples.push(result);
        }

        Ok(Wave {
            samples,
            sample_rate,
            channels,
        })
    }
}

impl Wave {
    pub fn new(samples: &[f64], sample_rate: u32, channels: u16) -> Result<Self, WaveError> {
        ensure_valid_sample_rate(sample_rate)?;
        ensure_valid_channels(channels)?;
        ensure_sufficient_samples(channels, samples)?;

        Ok(Self {
            samples: samples.to_vec(),
            sample_rate,
            channels,
        })
    }
}

fn ensure_valid_sample_rate(sample_rate: u32) -> Result<(), WaveError> {
    if sample_rate == 0 {
        return Err(WaveError::Creation(CreationError::InvalidSampleRate(
            sample_rate,
        )));
    }
    Ok(())
}

fn ensure_valid_channels(channels: u16) -> Result<(), WaveError> {
    if channels == 0 {
        return Err(WaveError::Creation(CreationError::InvalidChannels(
            channels,
        )));
    }
    Ok(())
}

fn ensure_sufficient_samples(channels: u16, samples: &[f64]) -> Result<(), WaveError> {
    if (channels as usize) > samples.len() {
        return Err(WaveError::Creation(CreationError::TooFewSamples {
            actual: samples.len(),
            required: channels as usize,
        }));
    }
    Ok(())
}

fn validate_not_empty(wave: &Wave) -> Result<(), WaveError> {
    if wave.samples.is_empty() {
        return Err(WaveError::Data(DataError::EmptySamples));
    }
    Ok(())
}

fn validate_equal_lengths(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.samples.len() != right.samples.len() {
        return Err(WaveError::Data(DataError::LengthMismatch {
            left: left.samples.len(),
            right: right.samples.len(),
        }));
    }
    Ok(())
}

fn validate_equal_sample_rates(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.sample_rate != right.sample_rate {
        return Err(WaveError::Data(DataError::SampleRateMismatch {
            left: left.sample_rate,
            right: right.sample_rate,
        }));
    }
    Ok(())
}

fn validate_equal_channels(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.channels != right.channels {
        return Err(WaveError::Data(DataError::ChannelMismatch {
            left: left.channels,
            right: right.channels,
        }));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum WaveError {
    #[error("creation error: {0}")]
    Creation(#[from] CreationError),

    #[error("data validation error: {0}")]
    Data(#[from] DataError),
}

#[derive(Debug, Error)]
pub enum CreationError {
    #[error("sample rate must be greater than 0, found {0}")]
    InvalidSampleRate(u32),

    #[error("channel count must be greater than 0, found {0}")]
    InvalidChannels(u16),

    #[error("insufficient samples provided: required {required}, found {actual}")]
    TooFewSamples { required: usize, actual: usize },
}

#[derive(Debug, Error)]
pub enum DataError {
    #[error("sample rate mismatch: left={left}Hz, right={right}Hz")]
    SampleRateMismatch { left: u32, right: u32 },

    #[error("sample length mismatch: left={left}, right={right}")]
    LengthMismatch { left: usize, right: usize },

    #[error("channel count mismatch: left={left}, right={right}")]
    ChannelMismatch { left: u16, right: u16 },

    #[error("operation cannot be performed on empty samples")]
    EmptySamples,
}

#[cfg(test)]
mod tests {
    use super::Wave;

    #[test]
    fn new() -> Result<(), Box<dyn std::error::Error>> {
        _ = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
        _ = Wave::new(&[1.0, 1.0, 1.0, 1.0, 1.0], 44100, 1)?;

        Ok(())
    }

    #[test]
    fn mix() -> Result<(), Box<dyn std::error::Error>> {
        use super::Waveable;

        let left = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
        let right = Wave::new(&[1.0, 1.0, 1.0, 1.0, 1.0], 44100, 1)?;
        let result = left.mix(&right, |l, r| l + r)?;

        assert_eq!(result, Wave::new(&[1.5, 1.5, 1.5, 1.5, 1.5], 44100, 1)?);

        Ok(())
    }
}
