//! # rustttwavvv

use i24::I24;
use num_traits::cast::ToPrimitive;
use riffy_chan::{Chunk, FourCC};
use std::{
    array::TryFromSliceError,
    io::{Read, Write},
};
use thiserror::Error;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wav {
    format_code: FormatCode,
    sample_rate: SampleRate,
    channels: Channels,
    bits: u16,
    samples: Vec<f64>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SampleRate(u32);

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

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Channels(u16);

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

const SUPPORTED_BITS: [u16; 5] = [8, 16, 24, 32, 64];

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

    pub fn write<W: Write>(&self, write: W) -> Result<Wav, WavError> {
        todo!()
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

    wav.format_code = FormatCode::new(format_code_raw).map_err(WavError::FormatCodeError)?;
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

    if SUPPORTED_BITS.contains(&bits) {
        wav.bits = bits;
    } else {
        return Err(WavError::UnsupportedBits { found_bits: bits });
    }

    Ok(())
}

fn parse_samples(wav: &mut Wav, data: &[u8]) -> Result<(), WavError> {
    let sample_count = match wav.bits {
        8 => data.len(),
        16 => data.len() / 2,
        24 => data.len() / 3,
        32 => data.len() / 4,
        64 => data.len() / 8,
        _ => {
            return Err(WavError::UnsupportedBits {
                found_bits: wav.bits,
            });
        }
    };

    let mut samples: Vec<f64> = Vec::new();
    for i in 0..sample_count {
        match wav.bits {
            8 => match wav.format_code {
                FormatCode::PCM => {
                    let value: f64 = Into::<f64>::into(data[i]) / Into::<f64>::into(u8::MAX);
                    samples.push(value)
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code.clone(),
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            16 => match wav.format_code {
                FormatCode::PCM => {
                    const BYTES_NUMBER: usize = 2; // A i16 wave data's sample takes 2
                    let indexes: Vec<usize> = (i * BYTES_NUMBER..(i + 1) * BYTES_NUMBER).collect();
                    let values: Vec<u8> = indexes.into_iter().map(|v| data[v]).collect();
                    let value_raw =
                        i16::from_le_bytes(values[0..BYTES_NUMBER].try_into().map_err(|err| {
                            WavError::InvalidSample {
                                actual: values,
                                inner_error: err,
                            }
                        })?);

                    let value: f64 = Into::<f64>::into(value_raw) / Into::<f64>::into(i16::MAX);
                    samples.push(value);
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code.clone(),
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            24 => match wav.format_code {
                FormatCode::PCM => {
                    const BYTES_NUMBER: usize = 3; // A i24 wave data's sample takes 3
                    let indexes: Vec<usize> = (i * BYTES_NUMBER..(i + 1) * BYTES_NUMBER).collect();
                    let values: Vec<u8> = indexes.into_iter().map(|v| data[v]).collect();
                    let value_raw =
                        I24::from_le_bytes(values[0..BYTES_NUMBER].try_into().map_err(|err| {
                            WavError::InvalidSample {
                                actual: values,
                                inner_error: err,
                            }
                        })?);

                    let value: f64 = value_raw.to_f64().ok_or(WavError::I24Error(value_raw))?
                        / I24::MAX.to_f64().ok_or(WavError::I24Error(I24::MAX))?;
                    samples.push(value)
                }
                _ => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code.clone(),
                        expected: vec![FormatCode::PCM],
                    });
                }
            },
            32 => match wav.format_code {
                FormatCode::PCM => {
                    const BYTES_NUMBER: usize = 4; // A i24 wave data's sample takes 3
                    let indexes: Vec<usize> = (i * BYTES_NUMBER..(i + 1) * BYTES_NUMBER).collect();
                    let values: Vec<u8> = indexes.into_iter().map(|v| data[v]).collect();
                    let value_raw =
                        i32::from_le_bytes(values[0..BYTES_NUMBER].try_into().map_err(|err| {
                            WavError::InvalidSample {
                                actual: values,
                                inner_error: err,
                            }
                        })?);

                    let value: f64 = Into::<f64>::into(value_raw) / Into::<f64>::into(i32::MAX);
                    samples.push(value);
                }
                FormatCode::IEEEFloat => {
                    const BYTES_NUMBER: usize = 4;
                    let indexes: Vec<usize> = (i * BYTES_NUMBER..(i + 1) * BYTES_NUMBER).collect();
                    let values: Vec<u8> = indexes.into_iter().map(|v| data[v]).collect();
                    let value_raw =
                        f32::from_le_bytes(values[0..BYTES_NUMBER].try_into().map_err(|err| {
                            WavError::InvalidSample {
                                actual: values,
                                inner_error: err,
                            }
                        })?);

                    let value: f64 = value_raw.into();
                    samples.push(value);
                }
            },
            64 => match wav.format_code {
                FormatCode::PCM => {
                    return Err(WavError::UnsupportedFormatCode {
                        bits: wav.bits,
                        format_code: wav.format_code.clone(),
                        expected: vec![FormatCode::IEEEFloat],
                    });
                }
                FormatCode::IEEEFloat => {
                    const BYTES_NUMBER: usize = 8;
                    let indexes: Vec<usize> = (i * BYTES_NUMBER..(i + 1) * BYTES_NUMBER).collect();
                    let values: Vec<u8> = indexes.into_iter().map(|v| data[v]).collect();
                    let value =
                        f64::from_le_bytes(values[0..BYTES_NUMBER].try_into().map_err(|err| {
                            WavError::InvalidSample {
                                actual: values,
                                inner_error: err,
                            }
                        })?);

                    samples.push(value);
                }
            },
            _ => {
                return Err(WavError::UnsupportedBits {
                    found_bits: wav.bits,
                });
            }
        }
    }

    wav.samples = samples;
    Ok(())
}

#[derive(Debug, Error)]
pub enum WavError {
    #[error("RIFF parse error from riffy_chan")]
    ChunkError(riffy_chan::ChunkError),

    #[error("FormatCode parse error")]
    FormatCodeError(FormatCodeError),

    #[error("I24 parse error from i24 crate when parsing from I24 to f64")]
    I24Error(I24),

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
        "Invalid channels in fmt chunk is detected. Found {:?}, and caused {}",
        actual,
        inner_error
    )]
    InvalidChannels {
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

    #[error("Invalid sample in data chunk is detected. Found {actual:?}, and caused {inner_error}")]
    InvalidSample {
        actual: Vec<u8>,
        inner_error: TryFromSliceError,
    },

    #[error(
        "Unsupported FormatCode is detected. It must be in range of {expected:?}, found {format_code:?} and bits: {bits}"
    )]
    UnsupportedFormatCode {
        bits: u16,
        format_code: FormatCode,
        expected: Vec<FormatCode>,
    },

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
        let mut file = std::fs::File::open(filepath)?;
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
            bits: 8,
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
        // write(&expected, b"")?;
        Ok(())
    }

    #[test]
    fn _10_samples_16bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-16bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 16,
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
        // write(&expected, b"")?;
        Ok(())
    }

    #[test]
    fn _10_samples_24bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-24bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 24,
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
        // write(&expected, b"")?;
        Ok(())
    }

    #[test]
    fn _10_samples_32bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-32bit-PCM.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::PCM,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 32,
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
        // write(&expected, b"")?;
        Ok(())
    }

    #[test]
    fn _10_samples_32bit_ieee_float() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-32bit-IEEEFloat.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::IEEEFloat,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 32,
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
        // write(&expected, b"")?;
        Ok(())
    }

    #[test]
    fn _10_samples_64bit_ieee_float() -> Result<(), Box<dyn std::error::Error>> {
        const FILEPATH: &str = "./assets/10-samples-64bit-IEEEFloat.wav";
        let expected: Wav = Wav {
            format_code: FormatCode::IEEEFloat,
            sample_rate: 44100.into(),
            channels: 1.into(),
            bits: 64,
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
        // write(&expected, b"")?;
        Ok(())
    }
}
