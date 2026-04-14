//! # wave module

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wave {
    pub samples: Vec<f64>,
    sample_rate: u32,
    channels: u16,
}

impl Wave {
    pub fn new(samples: Vec<f64>, sample_rate: u32, channels: u16) -> Result<Self, WaveError> {
        if sample_rate == 0 {
            return Err();
        }
    }
}
