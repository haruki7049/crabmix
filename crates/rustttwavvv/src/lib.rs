//! # rustttwavvv

use riffy_chan::{Chunk, FourCC};
use std::{array::TryFromSliceError, io::Read};
use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wav {
    format_code: FormatCode,
    sample_rate: u32,
    channels: u16,
    bits: u16,
    samples: Vec<f64>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum FormatCode {
    #[default]
    PCM,
    IEEEFloat,
}

impl FormatCode {
    pub fn new(inner: u16) -> Result<FormatCode, FormatCodeError> {
        match inner {
            1 => Ok(FormatCode::PCM),
            3 => Ok(FormatCode::IEEEFloat),
            _ => Err(FormatCodeError::InvalidCode { actual: inner }),
        }
    }
}

#[derive(Debug, Error)]
pub enum FormatCodeError {
    #[error("Invalid Code for FormatCode. found {}", actual)]
    InvalidCode { actual: u16 },
}

impl Wav {
    pub fn read<R: Read>(read: R) -> Result<Wav, WavError> {
        // Parse buf to RIFF Chunk
        let root_chunk: Chunk = Chunk::read(read).map_err(WavError::ChunkError)?;

        // Unpack RIFF format's four_cc and chunks
        let (four_cc, chunks): (FourCC, Vec<Chunk>) = match root_chunk {
            Chunk::Riff { four_cc, chunks } => (four_cc, chunks),
            _ => return Err(WavError::InvalidRiffChunk { actual: root_chunk }),
        };

        if four_cc != FourCC::from(b"WAVE") {
            return Err(WavError::InvalidWave { actual: four_cc });
        }

        let wav: Wav = parse_chunk(chunks)?;
        Ok(wav)
    }
}

fn parse_chunk(chunks: Vec<Chunk>) -> Result<Wav, WavError> {
    let mut wav: Wav = Wav::default();

    for chunk in chunks {
        if let Chunk::Chunk { four_cc, data } = chunk {
            let four_cc_inner = Into::<[u8; 4]>::into(four_cc);

            if &four_cc_inner == b"fmt " {
                parse_format_code(&mut wav, &data)?;
                parse_sample_rate(&mut wav, &data)?;
                parse_bits(&mut wav, &data)?;
            } else if &four_cc_inner == b"data" {
            }
        }
    }

    Ok(wav)
}

fn parse_format_code(wav: &mut Wav, data: &Vec<u8>) -> Result<(), WavError> {
    let format_code_raw =
        u16::from_le_bytes(data[0..2].try_into().map_err(|err: TryFromSliceError| {
            WavError::InvalidFormatCode {
                actual: data[0..2].to_vec(),
                inner_error: err,
            }
        })?);

    wav.format_code = FormatCode::new(format_code_raw).map_err(WavError::FormatCodeError)?;
    Ok(())
}

fn parse_sample_rate(wav: &mut Wav, data: &Vec<u8>) -> Result<(), WavError> {
    let sample_rate =
        u32::from_le_bytes(data[4..8].try_into().map_err(|err: TryFromSliceError| {
            WavError::InvalidSampleRate {
                actual: data[4..8].to_vec(),
                inner_error: err,
            }
        })?);

    wav.sample_rate = sample_rate;
    Ok(())
}

fn parse_bits(wav: &mut Wav, data: &Vec<u8>) -> Result<(), WavError> {
    const SUPPORTED_BITS: [u16; 5] = [8, 16, 24, 32, 64];
    let bits = u16::from_le_bytes(data[14..16].try_into().map_err(|err: TryFromSliceError| {
        WavError::InvalidBits {
            actual: data[14..16].to_vec(),
            inner_error: err,
        }
    })?);

    if SUPPORTED_BITS.contains(&bits) {
        wav.bits = bits;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum WavError {
    #[error("RIFF parse error from riffy_chan")]
    ChunkError(riffy_chan::ChunkError),

    #[error("FormatCode parse error")]
    FormatCodeError(FormatCodeError),

    #[error(
        "Invalid chunk is detected. expected RIFF Chunk but found {:?}",
        actual
    )]
    InvalidRiffChunk { actual: Chunk },

    #[error(
        "Invalid chunk is detected. expected WAVE FourCC but found {:?}",
        actual
    )]
    InvalidWave { actual: FourCC },

    #[error(
        "Invalid FormatCode in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidFormatCode {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    #[error(
        "Invalid sample_rate in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidSampleRate {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    #[error(
        "Invalid bits in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidBits {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },
}

#[cfg(test)]
mod wav_tests {
    use super::Wav;

    #[test]
    fn read() -> Result<(), Box<dyn std::error::Error>> {
        let test_files = ["./assets/10-samples.wav"];

        for path in test_files {
            let file = std::fs::File::open(path)?;

            // Test:
            let wav: Wav = Wav::read(file)?;

            dbg!(wav);
        }

        Ok(())
    }
}
