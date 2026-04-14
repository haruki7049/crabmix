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
        check_both_samples(self, other)?;
        check_both_sample_rate(self, other)?;
        check_both_channels(self, other)?;
        check_is_empty_samples(self)?;

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
        whether_sample_rate_is_not_zero(sample_rate)?;
        whether_channels_is_not_zero(channels)?;
        whether_samples_is_too_short_than_channels(channels, samples)?;

        Ok(Self {
            samples: samples.to_vec(),
            sample_rate,
            channels,
        })
    }
}

fn whether_sample_rate_is_not_zero(sample_rate: u32) -> Result<(), WaveError> {
    if sample_rate == 0 {
        return Err(WaveError::New(NewError::InvalidSampleRate(sample_rate)));
    }

    Ok(())
}

fn whether_channels_is_not_zero(channels: u16) -> Result<(), WaveError> {
    if channels == 0 {
        return Err(WaveError::New(NewError::InvalidChannels(channels)));
    }

    Ok(())
}

fn whether_samples_is_too_short_than_channels(
    channels: u16,
    samples: &[f64],
) -> Result<(), WaveError> {
    if channels as usize > samples.len() {
        return Err(WaveError::New(NewError::TooShortSamples {
            samples_len: samples.len(),
            channels,
        }));
    }

    Ok(())
}

fn check_is_empty_samples(wave: &Wave) -> Result<(), WaveError> {
    if wave.samples.is_empty() {
        return Err(WaveError::Mix(MixError::ZeroSamples));
    }

    Ok(())
}

fn check_both_samples(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.samples.len() != right.samples.len() {
        return Err(WaveError::Mix(MixError::InvalidSamplesLen {
            left: left.samples.len(),
            right: right.samples.len(),
        }));
    }

    Ok(())
}

fn check_both_sample_rate(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.sample_rate != right.sample_rate {
        return Err(WaveError::Mix(MixError::InvalidSampleRate {
            left: left.sample_rate,
            right: right.sample_rate,
        }));
    }

    Ok(())
}

fn check_both_channels(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.channels != right.channels {
        return Err(WaveError::Mix(MixError::InvalidChannels {
            left: left.channels,
            right: right.channels,
        }));
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum WaveError {
    #[error("NewError: {0:?}")]
    New(#[from] NewError),

    #[error("MixError: {0:?}")]
    Mix(#[from] MixError),
}

#[derive(Debug, Error)]
pub enum NewError {
    #[error("Invalid sample_rate, {0:?}")]
    InvalidSampleRate(u32),

    #[error("Invalid channels, {0:?}")]
    InvalidChannels(u16),

    #[error(
        "Too short samples. The channels value is {channels}, and the samples.len() is {samples_len}"
    )]
    TooShortSamples { channels: u16, samples_len: usize },
}

#[derive(Debug, Error)]
pub enum MixError {
    #[error("The self's sample_rate, {left:?} and the other's smaple_rate, {right:?} is different")]
    InvalidSampleRate { left: u32, right: u32 },

    #[error(
        "The samples' len by self, {left:?} and the samples' len by other, {right:?} is different"
    )]
    InvalidSamplesLen { left: usize, right: usize },

    #[error("The self's channels, {left:?} and the other's channels, {right:?} is different")]
    InvalidChannels { left: u16, right: u16 },

    #[error("Both samples have zero samples")]
    ZeroSamples,
}

#[cfg(test)]
mod tests {
    use super::{Wave, Waveable};

    #[test]
    fn new() -> Result<(), Box<dyn std::error::Error>> {
        _ = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
        _ = Wave::new(&[1.0, 1.0, 1.0, 1.0, 1.0], 44100, 1)?;

        Ok(())
    }

    #[test]
    fn mix() -> Result<(), Box<dyn std::error::Error>> {
        let left = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
        let right = Wave::new(&[1.0, 1.0, 1.0, 1.0, 1.0], 44100, 1)?;
        let result = left.mix(&right, |l, r| l + r)?;

        assert_eq!(result, Wave::new(&[1.5, 1.5, 1.5, 1.5, 1.5], 44100, 1)?);

        Ok(())
    }
}
