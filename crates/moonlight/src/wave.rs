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

    pub fn mix<F>(&self, other: &Self, mixer_fn: F) -> Result<Self, WaveError>
    where
        F: Fn(f64, f64) -> f64,
    {
        check_both_samples(self, other)?;
        check_both_sample_rate(self, other)?;
        check_both_channels(self, other)?;
        check_zero_samples(self)?;

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

fn check_zero_samples(wave: &Wave) -> Result<(), WaveError> {
    if wave.samples.len() == 0 {
        return Err(WaveError::MixError(MixError::ZeroSamples));
    }

    Ok(())
}

fn check_both_samples(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.samples.len() != right.samples.len() {
        return Err(WaveError::MixError(MixError::InvalidSamplesLen {
            left: left.samples.len(),
            right: right.samples.len(),
        }));
    }

    Ok(())
}

fn check_both_sample_rate(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.sample_rate != right.sample_rate {
        return Err(WaveError::MixError(MixError::InvalidSampleRate {
            left: left.sample_rate,
            right: right.sample_rate,
        }));
    }

    Ok(())
}

fn check_both_channels(left: &Wave, right: &Wave) -> Result<(), WaveError> {
    if left.channels != right.channels {
        return Err(WaveError::MixError(MixError::InvalidChannels {
            left: left.channels,
            right: right.channels,
        }));
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum WaveError {
    #[error("Invalid sample_rate, {0:?}")]
    InvalidSampleRate(u32),

    #[error("Invalid channels, {0:?}")]
    InvalidChannels(u16),

    #[error("MixError: {0:?}")]
    MixError(#[from] MixError),
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
