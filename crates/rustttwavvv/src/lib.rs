//! # rustttwavvv
//!
//! `rustttwavvv` is a library for reading and writing
//! [WAV](https://en.wikipedia.org/wiki/WAV) audio files in Rust.
//!
//! WAV is one of the most common uncompressed audio formats, built on top of
//! the RIFF container format. This crate handles the parsing and construction
//! of the `fmt ` and `data` chunks, supporting multiple bit depths and format
//! codes.
//!
//! ## Supported formats
//!
//! | Bit depth | PCM | IEEE Float |
//! |-----------|-----|------------|
//! | 8-bit     | ✓   |            |
//! | 16-bit    | ✓   |            |
//! | 24-bit    | ✓   |            |
//! | 32-bit    | ✓   | ✓          |
//! | 64-bit    |     | ✓          |
//!
//! ## Core types
//!
//! - [`Wav`] – the main type representing a WAV audio file in memory.
//! - [`FormatCode`] – the audio encoding format (PCM or IEEE Float).
//! - [`SampleRate`] – the number of samples per second (e.g. 44100).
//! - [`Channels`] – the number of audio channels (e.g. 1 for mono, 2 for stereo).
//! - [`Bits`] – the bit depth per sample.
//! - [`WavError`] – errors that can occur during reading or writing.
//!
//! ## Reading example
//!
//! ```rust,no_run
//! use rustttwavvv::Wav;
//!
//! let file = std::fs::File::open("audio.wav").unwrap();
//! let wav = Wav::read(file).unwrap();
//! ```
//!
//! ## Writing example
//!
//! ```rust,no_run
//! use rustttwavvv::Wav;
//!
//! # let wav = Wav::default();
//! let mut file = std::fs::File::create("output.wav").unwrap();
//! wav.write(&mut file).unwrap();
//! ```

use i24::I24;
use num_traits::{FromPrimitive, cast::ToPrimitive};
use riffy_chan::{Chunk, FourCC};
use std::{
    array::TryFromSliceError,
    io::{Read, Write},
};
use thiserror::Error;

/// A WAV audio file represented in memory.
///
/// `Wav` stores the audio metadata (format code, sample rate, channels, bit
/// depth) together with the sample data normalised to `f64` values.
///
/// Use [`Wav::read`] to parse a WAV file from any [`Read`]
/// source, and [`Wav::write`] to serialise it to any
/// [`Write`] destination.
///
/// # Constructing a `Wav`
///
/// ```
/// use rustttwavvv::{Wav, FormatCode, SampleRate, Channels, Bits};
///
/// let wav = Wav::new(
///     FormatCode::PCM,
///     SampleRate::new(44100),
///     Channels::new(1),
///     Bits::_16Bit,
///     vec![0.0, 0.5, -0.5],
/// );
///
/// assert_eq!(wav.format_code(), FormatCode::PCM);
/// assert_eq!(wav.sample_rate().value(), 44100);
/// assert_eq!(wav.channels().value(), 1);
/// assert_eq!(wav.bits(), Bits::_16Bit);
/// assert_eq!(wav.samples().len(), 3);
/// ```
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wav {
    /// The audio encoding format (e.g. PCM or IEEE Float).
    format_code: FormatCode,
    /// The number of samples per second (e.g. 44100 Hz).
    sample_rate: SampleRate,
    /// The number of audio channels (e.g. 1 for mono, 2 for stereo).
    channels: Channels,
    /// The bit depth per sample (e.g. 16-bit, 24-bit).
    bits: Bits,
    /// The audio sample data, normalised to `f64` values.
    samples: Vec<f64>,
}

impl TryFrom<Wav> for Chunk {
    type Error = WavError;

    fn try_from(wav: Wav) -> Result<Self, Self::Error> {
        let wave_four_cc = FourCC::from(*b"WAVE");

        let fmt_chunk = construct_fmt_chunk(&wav);
        let data_chunk = construct_data_chunk(&wav)?;
        let result: Chunk = Chunk::Riff {
            four_cc: wave_four_cc,
            chunks: vec![fmt_chunk, data_chunk],
        };
        Ok(result)
    }
}

impl TryFrom<&Wav> for Chunk {
    type Error = WavError;

    fn try_from(wav: &Wav) -> Result<Self, Self::Error> {
        let wave_four_cc = FourCC::from(*b"WAVE");

        let fmt_chunk = construct_fmt_chunk(wav);
        let data_chunk = construct_data_chunk(wav)?;
        let result: Chunk = Chunk::Riff {
            four_cc: wave_four_cc,
            chunks: vec![fmt_chunk, data_chunk],
        };
        Ok(result)
    }
}

fn construct_fmt_chunk(wav: &Wav) -> Chunk {
    let format_code_raw = (wav.format_code as u16).to_le_bytes().to_vec();
    let channels_raw = wav.channels.value().to_le_bytes().to_vec();
    let sample_rate_raw = wav.sample_rate.value().to_le_bytes().to_vec();

    let bits_per_sample = wav.bits as u16;
    let block_align = wav.channels.value() * (bits_per_sample / 8);
    let bytes_per_sec = wav.sample_rate.value() * u32::from(block_align);

    let bits_per_sample_raw = bits_per_sample.to_le_bytes().to_vec();
    let block_align_raw = block_align.to_le_bytes().to_vec();
    let bytes_per_sec_raw = bytes_per_sec.to_le_bytes().to_vec();

    let fmt_four_cc = FourCC::from(*b"fmt ");
    let fmt_data = [
        format_code_raw,
        channels_raw,
        sample_rate_raw,
        bytes_per_sec_raw,
        block_align_raw,
        bits_per_sample_raw,
    ]
    .concat();

    Chunk::Chunk {
        four_cc: fmt_four_cc,
        data: fmt_data,
    }
}

fn construct_data_chunk(wav: &Wav) -> Result<Chunk, WavError> {
    let data_four_cc = FourCC::from(*b"data");
    let mut data_data: Vec<u8> = Vec::new();

    for sample in &wav.samples {
        match wav.bits {
            Bits::_8Bit => match wav.format_code {
                FormatCode::PCM => {
                    let clamped_v = ((*sample as u8) * u8::MAX).clamp(u8::MIN, u8::MAX);
                    data_data.push(clamped_v);
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_16Bit => match wav.format_code {
                FormatCode::PCM => {
                    let clamped_v = ((*sample as i16) * i16::MAX).clamp(i16::MIN, i16::MAX);
                    data_data.extend(clamped_v.to_le_bytes());
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_24Bit => match wav.format_code {
                FormatCode::PCM => {
                    let i24_max = I24::MAX.to_f64().ok_or(WavError::FromI24Error(I24::MAX))?;
                    let original_v =
                        I24::from_f64(*sample * i24_max).ok_or(WavError::ToI24Error(*sample))?;
                    let clamped_v = original_v.clamp(I24::MIN, I24::MAX);
                    data_data.extend(clamped_v.to_le_bytes());
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_32Bit => match wav.format_code {
                FormatCode::PCM => {
                    let clamped_v = ((*sample as i32) * i32::MAX).clamp(i32::MIN, i32::MAX);
                    data_data.extend(clamped_v.to_le_bytes());
                }
                FormatCode::IEEEFloat => {
                    data_data.extend(sample.to_le_bytes());
                }
            },
            Bits::_64Bit => match wav.format_code {
                FormatCode::IEEEFloat => {
                    data_data.extend(sample.to_le_bytes());
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::IEEEFloat],
                    });
                }
            },
        }
    }

    Ok(Chunk::Chunk {
        four_cc: data_four_cc,
        data: data_data,
    })
}

/// The sample rate of a WAV file, in samples per second.
///
/// Common values include 44100 (CD quality), 48000 (DVD/broadcast), and
/// 96000 (high-resolution audio).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SampleRate(u32);

impl SampleRate {
    /// Creates a new `SampleRate` from the given value.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the inner sample rate value.
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl From<u32> for SampleRate {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for SampleRate {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The number of audio channels in a WAV file.
///
/// Typical values are 1 (mono) and 2 (stereo).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Channels(u16);

impl Channels {
    /// Creates a new `Channels` from the given value.
    pub fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the inner channel count value.
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl From<u16> for Channels {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for Channels {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The bit depth of each audio sample.
///
/// The bit depth determines the resolution of each sample and, together with
/// the [`FormatCode`], determines how samples are encoded in the `data` chunk.
///
/// # Variants
///
/// | Variant   | Supported format codes |
/// |-----------|------------------------|
/// | `_8Bit`   | PCM                    |
/// | `_16Bit`  | PCM                    |
/// | `_24Bit`  | PCM                    |
/// | `_32Bit`  | PCM, IEEE Float        |
/// | `_64Bit`  | IEEE Float             |
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum Bits {
    /// 8-bit samples (unsigned, PCM only).
    _8Bit = 8,

    /// 16-bit samples (signed, PCM only). This is the default.
    #[default]
    _16Bit = 16,

    /// 24-bit samples (signed, PCM only).
    _24Bit = 24,
    /// 32-bit samples (PCM or IEEE Float).
    _32Bit = 32,
    /// 64-bit samples (IEEE Float only).
    _64Bit = 64,
}

/// All supported bit depths.
const SUPPORTED_BITS: [Bits; 5] = [
    Bits::_8Bit,
    Bits::_16Bit,
    Bits::_24Bit,
    Bits::_32Bit,
    Bits::_64Bit,
];

impl Bits {
    /// Returns the number of bytes per sample for this bit depth.
    pub fn byte_count(self) -> usize {
        (self as usize) / 8
    }
}

impl TryFrom<u16> for Bits {
    type Error = WavError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            8 => Ok(Self::_8Bit),
            16 => Ok(Self::_16Bit),
            24 => Ok(Self::_24Bit),
            32 => Ok(Self::_32Bit),
            64 => Ok(Self::_64Bit),
            _ => Err(WavError::UnsupportedBits { found_bits: value }),
        }
    }
}

/// The audio encoding format stored in the WAV `fmt ` chunk.
///
/// WAV files support several format codes, but this crate currently handles
/// the two most common ones: uncompressed PCM and IEEE 754 floating-point.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum FormatCode {
    /// Pulse-Code Modulation – uncompressed integer samples.
    #[default]
    PCM = 1,
    /// IEEE 754 floating-point samples.
    IEEEFloat = 3,
}

impl TryFrom<u16> for FormatCode {
    type Error = FormatCodeError;

    fn try_from(inner: u16) -> Result<Self, Self::Error> {
        match inner {
            1 => Ok(FormatCode::PCM),
            3 => Ok(FormatCode::IEEEFloat),
            _ => Err(FormatCodeError::InvalidCode { actual: inner }),
        }
    }
}

/// Errors that can occur when converting a raw `u16` value to a [`FormatCode`].
#[derive(Debug, Error)]
pub enum FormatCodeError {
    /// The raw code does not correspond to any known format code.
    #[error("Invalid Code for FormatCode. found {}", actual)]
    InvalidCode { actual: u16 },
}

impl Wav {
    /// Creates a new `Wav` with the given metadata and sample data.
    pub fn new(
        format_code: FormatCode,
        sample_rate: SampleRate,
        channels: Channels,
        bits: Bits,
        samples: Vec<f64>,
    ) -> Self {
        Self {
            format_code,
            sample_rate,
            channels,
            bits,
            samples,
        }
    }

    /// Returns the audio encoding format.
    pub fn format_code(&self) -> FormatCode {
        self.format_code
    }

    /// Returns a reference to the sample rate.
    pub fn sample_rate(&self) -> &SampleRate {
        &self.sample_rate
    }

    /// Returns a reference to the channel count.
    pub fn channels(&self) -> &Channels {
        &self.channels
    }

    /// Returns the bit depth.
    pub fn bits(&self) -> Bits {
        self.bits
    }

    /// Returns a slice of the normalised sample data.
    pub fn samples(&self) -> &[f64] {
        &self.samples
    }

    /// Returns a mutable reference to the sample data.
    pub fn samples_mut(&mut self) -> &mut Vec<f64> {
        &mut self.samples
    }

    /// Consumes the `Wav` and returns the sample data.
    pub fn into_samples(self) -> Vec<f64> {
        self.samples
    }

    /// Reads and parses a WAV file from the given reader.
    ///
    /// The reader is consumed to parse the underlying RIFF structure and
    /// extract the `fmt ` and `data` chunks. Sample values are normalised to
    /// `f64`.
    ///
    /// # Errors
    ///
    /// Returns [`WavError`] if the data is not a valid RIFF/WAV file, or if
    /// the format code or bit depth is unsupported.
    pub fn read<R: Read>(read: R) -> Result<Wav, WavError> {
        // Parse buf to RIFF Chunk
        let root_chunk: Chunk = Chunk::read(read)?;

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

    /// Writes this WAV file to the given writer.
    ///
    /// The audio data is serialised as a valid RIFF/WAV byte stream that can
    /// be read back by [`Wav::read`] or any standard WAV player.
    ///
    /// # Errors
    ///
    /// Returns [`WavError`] if the combination of format code and bit depth
    /// is unsupported, or if an I/O error occurs while writing.
    pub fn write<W: Write>(&self, write: &mut W) -> Result<(), WavError> {
        let chunk: Chunk = self.try_into()?;
        chunk.write(write)?;
        Ok(())
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
                parse_channels(&mut wav, &data)?;
                parse_bits(&mut wav, &data)?;
            } else if &four_cc_inner == b"data" {
                parse_samples(&mut wav, &data)?;
            }
        }
    }

    Ok(wav)
}

fn parse_format_code(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let format_code_raw =
        u16::from_le_bytes(data[0..2].try_into().map_err(|err: TryFromSliceError| {
            WavError::InvalidFormatCode {
                actual: data[0..2].to_vec(),
                inner_error: err,
            }
        })?);

    wav.format_code = FormatCode::try_from(format_code_raw).map_err(WavError::FormatCodeError)?;
    Ok(())
}

fn parse_sample_rate(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let sample_rate =
        u32::from_le_bytes(data[4..8].try_into().map_err(|err: TryFromSliceError| {
            WavError::InvalidSampleRate {
                actual: data[4..8].to_vec(),
                inner_error: err,
            }
        })?);

    wav.sample_rate = sample_rate.into();
    Ok(())
}

fn parse_channels(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let channels =
        u16::from_le_bytes(data[2..4].try_into().map_err(|err: TryFromSliceError| {
            WavError::InvalidChannels {
                actual: data[2..4].to_vec(),
                inner_error: err,
            }
        })?);

    wav.channels = channels.into();
    Ok(())
}

fn parse_bits(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let bits = u16::from_le_bytes(data[14..16].try_into().map_err(|err: TryFromSliceError| {
        WavError::InvalidBits {
            actual: data[14..16].to_vec(),
            inner_error: err,
        }
    })?);

    wav.bits = bits.try_into()?;
    Ok(())
}

fn parse_samples(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let byte_count = wav.bits.byte_count();
    let sample_count = data.len() / byte_count;
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let offset = i * byte_count;
        let chunk = &data[offset..offset + byte_count];

        let value = match wav.bits {
            Bits::_8Bit => match wav.format_code {
                FormatCode::PCM => f64::from(chunk[0]) / f64::from(u8::MAX),
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_16Bit => match wav.format_code {
                FormatCode::PCM => {
                    let raw = i16::from_le_bytes(chunk.try_into().map_err(|err| {
                        WavError::InvalidSample {
                            actual: chunk.to_vec(),
                            inner_error: err,
                        }
                    })?);
                    f64::from(raw) / f64::from(i16::MAX)
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_24Bit => match wav.format_code {
                FormatCode::PCM => {
                    let raw = I24::from_le_bytes(chunk.try_into().map_err(|err| {
                        WavError::InvalidSample {
                            actual: chunk.to_vec(),
                            inner_error: err,
                        }
                    })?);
                    raw.to_f64().ok_or(WavError::FromI24Error(raw))?
                        / I24::MAX.to_f64().ok_or(WavError::FromI24Error(I24::MAX))?
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            Bits::_32Bit => match wav.format_code {
                FormatCode::PCM => {
                    let raw = i32::from_le_bytes(chunk.try_into().map_err(|err| {
                        WavError::InvalidSample {
                            actual: chunk.to_vec(),
                            inner_error: err,
                        }
                    })?);
                    f64::from(raw) / f64::from(i32::MAX)
                }
                FormatCode::IEEEFloat => {
                    let raw = f32::from_le_bytes(chunk.try_into().map_err(|err| {
                        WavError::InvalidSample {
                            actual: chunk.to_vec(),
                            inner_error: err,
                        }
                    })?);
                    f64::from(raw)
                }
            },
            Bits::_64Bit => match wav.format_code {
                FormatCode::IEEEFloat => {
                    f64::from_le_bytes(chunk.try_into().map_err(|err| {
                        WavError::InvalidSample {
                            actual: chunk.to_vec(),
                            inner_error: err,
                        }
                    })?)
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code,
                        expected: vec![FormatCode::IEEEFloat],
                    });
                }
            },
        };

        samples.push(value);
    }

    wav.samples = samples;
    Ok(())
}

/// Errors that can occur when reading or writing a WAV file.
#[derive(Debug, Error)]
pub enum WavError {
    /// An error propagated from the underlying RIFF parser ([`riffy_chan`]).
    #[error("RIFF parse error from riffy_chan: {0}")]
    Chunk(#[from] riffy_chan::ChunkError),

    /// The format code in the `fmt ` chunk could not be parsed.
    #[error("FormatCode parse error: {0}")]
    FormatCodeError(#[from] FormatCodeError),

    /// Failed to convert an [`I24`] value to `f64`.
    #[error("I24 parse error from i24 crate when parsing from I24 to f64. The value is: {0}")]
    FromI24Error(I24),

    /// Failed to convert an `f64` value to [`I24`].
    #[error("I24 parse error from i24 crate when parsing from f64 to I24. The value is: {0}")]
    ToI24Error(f64),

    /// The root chunk is not a RIFF chunk.
    #[error("Invalid chunk is detected. expected RIFF Chunk but found {actual:?}")]
    InvalidRiffChunk { actual: Chunk },

    /// The RIFF chunk's FourCC is not `WAVE`.
    #[error(
        "Invalid chunk is detected. expected WAVE FourCC but found {:?}",
        actual
    )]
    InvalidWave { actual: FourCC },

    /// The format code bytes in the `fmt ` chunk are malformed.
    #[error(
        "Invalid FormatCode in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidFormatCode {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    /// The sample rate bytes in the `fmt ` chunk are malformed.
    #[error(
        "Invalid sample_rate in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidSampleRate {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    /// The channel count bytes in the `fmt ` chunk are malformed.
    #[error(
        "Invalid channels in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidChannels {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    /// The bit depth bytes in the `fmt ` chunk are malformed.
    #[error(
        "Invalid bits in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidBits {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    /// A sample in the `data` chunk could not be decoded.
    #[error("Invalid sample in data chunk is detected. Found {actual:?}, and caused {inner_error}")]
    InvalidSample {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    /// The combination of bit depth and format code is not supported.
    #[error(
        "Unsupported FormatCode is detected. It must be in range of {expected:?}, found {format_code:?} and bits: {bits:?}"
    )]
    UnsupportedFormatCode {
        bits: Bits,
        format_code: FormatCode,
        expected: Vec<FormatCode>,
    },

    /// The bit depth is not one of the supported values.
    #[error(
        "Unsupported bits parameter is detected. It must be in range of {SUPPORTED_BITS:?}, found {found_bits}"
    )]
    UnsupportedBits { found_bits: u16 },
}

#[cfg(test)]
mod wav_tests {
    use super::{FormatCode, Wav};
    use std::io::{Read, Seek};

    fn read(filepath: &str, expected: &Wav) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::open(filepath)?;
        let actual: &Wav = &Wav::read(file)?;

        assert_eq!(expected, actual);
        Ok(())
    }

    fn write(wav: &Wav, expected: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = tempfile::tempfile()?;
        wav.write(&mut file)?;
        file.rewind()?;
        let mut written_bytes: Vec<u8> = Vec::new();
        file.read_to_end(&mut written_bytes)?;

        assert_eq!(expected, written_bytes);
        Ok(())
    }

    #[test]
    fn _10_samples_8bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-8bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 8.try_into()?,
            samples: vec![
                0.5019607843137255,
                0.5137254901960784,
                0.5254901960784314,
                0.5372549019607843,
                0.5490196078431373,
                0.5607843137254902,
                0.5725490196078431,
                0.5843137254901961,
                0.596078431372549,
                0.6078431372549019,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 46, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1, 0,
                1, 0, 68, 172, 0, 0, 68, 172, 0, 0, 1, 0, 8, 0, 100, 97, 116, 97, 10, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        )?;
        Ok(())
    }

    #[test]
    fn _10_samples_16bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-16bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 16.try_into()?,
            samples: vec![
                0.0,
                0.025055696279793694,
                0.0500198370311594,
                0.07480086672566912,
                0.09921567430646687,
                0.12341685232093265,
                0.14685506759849848,
                0.17001861629078036,
                0.19226660969878231,
                0.21390423291726432,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 56, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1, 0,
                1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0, 100, 97, 116, 97, 20, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        )?;
        Ok(())
    }

    #[test]
    fn _10_samples_24bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-24bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 24.try_into()?,
            samples: vec![
                0.0,
                0.02505934537164514,
                0.0500201046490794,
                0.07478476462182577,
                0.0992549776142809,
                0.1233358530206505,
                0.1469320233979253,
                0.16995038627986744,
                0.1923021307351745,
                0.21389737294881023,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 66, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1, 0,
                1, 0, 68, 172, 0, 0, 204, 4, 2, 0, 3, 0, 24, 0, 100, 97, 116, 97, 30, 0, 0, 0, 0,
                0, 0, 37, 53, 3, 15, 103, 6, 140, 146, 9, 99, 180, 12, 120, 201, 15, 171, 206, 18,
                239, 192, 21, 91, 157, 24, 253, 96, 27,
            ],
        )?;
        Ok(())
    }

    #[test]
    fn _10_samples_32bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-32bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 32.try_into()?,
            samples: vec![
                0.0,
                0.025059329357491493,
                0.050020210468219695,
                0.07478457692767707,
                0.09925513719173853,
                0.12333576386949782,
                0.14693184203791052,
                0.16995066412256596,
                0.19230180987729775,
                0.21389748166031086,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 76, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1, 0,
                1, 0, 68, 172, 0, 0, 16, 177, 2, 0, 4, 0, 32, 0, 100, 97, 116, 97, 40, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        )?;
        Ok(())
    }

    #[test]
    fn _10_samples_32bit_ieee_float() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-32bit-IEEEFloat.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::IEEEFloat,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 32.try_into()?,
            samples: vec![
                0.0,
                0.025059329345822334,
                0.050020210444927216,
                0.07478457689285278,
                0.09925513714551926,
                0.12333576381206512,
                0.14693184196949005,
                0.1699506640434265,
                0.19230180978775024,
                0.2138974815607071,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 116, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 3, 0,
                1, 0, 68, 172, 0, 0, 16, 177, 2, 0, 4, 0, 32, 0, 100, 97, 116, 97, 80, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 39, 169, 153, 63, 0, 0, 0, 192, 63, 156, 169, 63,
                0, 0, 0, 0, 21, 37, 179, 63, 0, 0, 0, 224, 200, 104, 185, 63, 0, 0, 0, 192, 238,
                146, 191, 63, 0, 0, 0, 160, 169, 206, 194, 63, 0, 0, 0, 128, 241, 192, 197, 63, 0,
                0, 0, 128, 88, 157, 200, 63, 0, 0, 0, 32, 254, 96, 203, 63,
            ],
        )?;
        Ok(())
    }

    #[test]
    fn _10_samples_64bit_ieee_float() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-64bit-IEEEFloat.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::IEEEFloat,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 64.try_into()?,
            samples: vec![
                0.0,
                0.025059329345822334,
                0.050020210444927216,
                0.07478457689285278,
                0.09925513714551926,
                0.12333576381206512,
                0.14693184196949005,
                0.1699506640434265,
                0.19230180978775024,
                0.2138974815607071,
            ],
        };

        read(FILEPATH, &expected)?;
        write(
            &expected,
            &[
                82, 73, 70, 70, 116, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 3, 0,
                1, 0, 68, 172, 0, 0, 32, 98, 5, 0, 8, 0, 64, 0, 100, 97, 116, 97, 80, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 39, 169, 153, 63, 0, 0, 0, 192, 63, 156, 169, 63,
                0, 0, 0, 0, 21, 37, 179, 63, 0, 0, 0, 224, 200, 104, 185, 63, 0, 0, 0, 192, 238,
                146, 191, 63, 0, 0, 0, 160, 169, 206, 194, 63, 0, 0, 0, 128, 241, 192, 197, 63, 0,
                0, 0, 128, 88, 157, 200, 63, 0, 0, 0, 32, 254, 96, 203, 63,
            ],
        )?;
        Ok(())
    }
}
