//! # wave module
//!
//! This module contains the core audio types and traits for the `crabmix`
//! library. It provides [`Wave`] — an in-memory representation of audio
//! waveform data — together with the [`Waveable`] trait that defines
//! operations such as mixing, splitting, reading, and writing audio.
//!
//! ## File format support
//!
//! Audio files are read and written through the [`Waveable`] trait methods
//! [`read`](Waveable::read) and [`write`](Waveable::write). The output
//! format is controlled by a [`WriteOptions`] implementation (see
//! [`WaveWriteOptions`] and [`FileFormat`]).
//!
//! Currently the only supported file format is WAV, powered by the
//! [`rustttwavvv`] crate.

pub use rustttwavvv;
pub use rustttwavvv::FormatCode;

use std::io::{Read, Write};
use thiserror::Error;

/// An in-memory representation of an audio waveform.
///
/// `Wave` stores normalised `f64` sample data together with the sample rate
/// and channel count. Samples are interleaved when there are multiple
/// channels.
///
/// Use [`Wave::new`] to construct a waveform from raw samples, or
/// [`Waveable::read`] to parse one from a file.
///
/// # Examples
///
/// ```rust
/// use crabmix::wave::Wave;
///
/// let wave = Wave::new(&[0.0, 0.5, -0.5], 44100, 1).unwrap();
/// assert_eq!(wave.samples.len(), 3);
/// ```
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Wave {
    /// The normalised audio sample data, stored as `f64` values.
    pub samples: Vec<f64>,
    /// The number of samples per second (e.g. 44100).
    pub sample_rate: u32,
    /// The number of audio channels (e.g. 1 for mono, 2 for stereo).
    pub channels: u16,
}

/// Options that control how a [`Wave`] is written to a file.
///
/// Wraps a [`FileFormat`] describing the target encoding. Pass an instance
/// of this type to [`Waveable::write`].
#[derive(Debug, PartialEq, Clone, Default)]
pub struct WaveWriteOptions {
    file_format: FileFormat,
}

impl WaveWriteOptions {
    /// Creates a new `WaveWriteOptions` with the given file format.
    pub fn new(file_format: FileFormat) -> Self {
        Self { file_format }
    }
}

impl WriteOptions for WaveWriteOptions {
    fn file_format(&self) -> FileFormat {
        self.file_format
    }
}

/// Describes the target file format and encoding parameters for writing audio.
///
/// Currently only the WAV format is supported.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileFormat {
    /// WAV file format with a specific format code and bit depth.
    Wav {
        /// The audio encoding format (e.g. PCM or IEEE Float).
        format_code: rustttwavvv::FormatCode,
        /// The bit depth per sample (e.g. 8, 16, 24, 32, or 64).
        bits: u16,
    },
}

impl FileFormat {
    /// Creates a [`FileFormat::Wav`] variant with the given format code and
    /// bit depth.
    pub fn wav(format_code: rustttwavvv::FormatCode, bits: u16) -> Self {
        Self::Wav { format_code, bits }
    }
}

impl Default for FileFormat {
    /// Returns WAV with PCM encoding at 16-bit depth.
    fn default() -> Self {
        Self::Wav {
            format_code: rustttwavvv::FormatCode::PCM,
            bits: 16,
        }
    }
}

/// Core trait for audio waveform types.
///
/// `Waveable` defines the common interface for reading, writing, mixing,
/// and splitting audio data. The [`Wave`] struct implements this trait.
pub trait Waveable {
    /// The error type returned by trait methods.
    type Error: std::error::Error;

    /// Returns a copy of the sample data.
    fn samples(&self) -> Vec<f64>;
    /// Returns the sample rate in samples per second.
    fn sample_rate(&self) -> u32;
    /// Returns the number of audio channels.
    fn channels(&self) -> u16;

    /// Mixes two waveforms element-wise using the provided function.
    ///
    /// Both waveforms must have the same sample rate, channel count, and
    /// sample length. The `mixer_fn` receives corresponding sample pairs
    /// and returns the mixed value.
    fn mix<F>(&self, other: &Self, mixer_fn: F) -> Result<Self, Self::Error>
    where
        Self: Waveable + Sized,
        F: Fn(f64, f64) -> f64;

    /// Splits the waveform at the given sample index.
    ///
    /// Returns a tuple of two waveforms: the first contains samples
    /// `0..separate_point` and the second contains the remainder.
    fn separate(&self, separate_point: usize) -> Result<(Self, Self), Self::Error>
    where
        Self: Waveable + Sized;

    /// Reads and parses an audio file from the given reader.
    fn read<R>(read: R) -> Result<Self, Self::Error>
    where
        Self: Waveable + Sized,
        R: Read;

    /// Writes the waveform to the given writer using the specified options.
    fn write<W, O>(&self, write: &mut W, options: O) -> Result<(), Self::Error>
    where
        Self: Waveable + Sized,
        W: Write,
        O: WriteOptions;
}

/// Trait for types that specify the output file format for writing audio.
pub trait WriteOptions {
    /// Returns the [`FileFormat`] to use when writing.
    fn file_format(&self) -> FileFormat;
}

impl Waveable for Wave {
    type Error = WaveError;

    fn samples(&self) -> Vec<f64> {
        self.samples.clone()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

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

    fn separate(&self, separate_point: usize) -> Result<(Self, Self), Self::Error>
    where
        Self: Waveable + Sized,
    {
        validate_not_empty(self)?;
        validate_samples_len_is_bigger_than_point(self, separate_point)?;
        validate_point_is_not_zero(separate_point)?;

        let initial_len = separate_point;
        let terminal_len = self.samples.len() - separate_point;
        let mut initial = Vec::new();
        let mut terminal = Vec::new();

        for i in 0..initial_len {
            initial.push(self.samples[i]);
        }
        for i in 0..terminal_len {
            terminal.push(self.samples[initial_len + i]);
        }

        let result = (
            Wave::new(&initial, self.sample_rate, self.channels)?,
            Wave::new(&terminal, self.sample_rate, self.channels)?,
        );
        Ok(result)
    }

    fn read<R>(read: R) -> Result<Self, Self::Error>
    where
        Self: Waveable + Sized,
        R: Read,
    {
        // Wav format
        if let Ok(wave) = wav_read(read) {
            return Ok(wave);
        }

        Err(WaveError::Creation(CreationError::UnsupportedFileFormat))
    }

    fn write<W, O>(&self, write: &mut W, options: O) -> Result<(), Self::Error>
    where
        Self: Waveable + Sized,
        W: Write,
        O: WriteOptions,
    {
        match options.file_format() {
            FileFormat::Wav { format_code, bits } => wav_write(
                self,
                write,
                format_code,
                self.sample_rate,
                self.channels,
                bits,
            ),
        }
    }
}

fn wav_read<R>(read: R) -> Result<Wave, WaveError>
where
    R: Read,
{
    let wav = rustttwavvv::Wav::read(read)?;
    let sample_rate: u32 = **wav.sample_rate();
    let channels: u16 = **wav.channels();
    let result = Wave::new(wav.samples(), sample_rate, channels)?;

    Ok(result)
}

fn wav_write<S, W>(
    wave: &S,
    write: &mut W,
    format_code: rustttwavvv::FormatCode,
    sample_rate: u32,
    channels: u16,
    bits: u16,
) -> Result<(), WaveError>
where
    S: Waveable + Sized,
    W: Write,
{
    let wav = rustttwavvv::Wav::new(
        format_code,
        sample_rate.into(),
        channels.into(),
        bits.try_into()?,
        wave.samples(),
    );
    wav.write(write)?;

    Ok(())
}

impl Wave {
    /// Creates a new `Wave` from a slice of samples, a sample rate, and a
    /// channel count.
    ///
    /// # Errors
    ///
    /// Returns [`WaveError`] if the sample rate is zero, the channel count
    /// is zero, or fewer samples are provided than the number of channels.
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

fn validate_point_is_not_zero(separate_point: usize) -> Result<(), WaveError> {
    if separate_point == 0 {
        return Err(WaveError::Data(DataError::TooShortSeparatePoint));
    }
    Ok(())
}

fn validate_samples_len_is_bigger_than_point(
    wave: &Wave,
    separate_point: usize,
) -> Result<(), WaveError> {
    if wave.samples.len() < separate_point {
        return Err(WaveError::Data(DataError::TooShortSamples));
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

/// Errors that can occur when creating or operating on a [`Wave`].
#[derive(Debug, Error)]
pub enum WaveError {
    /// An error occurred while constructing a [`Wave`].
    #[error("creation error: {0}")]
    Creation(#[from] CreationError),

    /// A data validation check failed before performing an operation.
    #[error("data validation error: {0}")]
    Data(#[from] DataError),

    /// An error propagated from the underlying WAV parser ([`rustttwavvv`]).
    #[error("Parsing error for Wav format (rustttwavvv crate): {0}")]
    Wav(#[from] rustttwavvv::WavError),
}

/// Errors that can occur when constructing a [`Wave`].
#[derive(Debug, Error, PartialEq)]
pub enum CreationError {
    /// The provided sample rate was zero.
    #[error("sample rate must be greater than 0, found {0}")]
    InvalidSampleRate(u32),

    /// The provided channel count was zero.
    #[error("channel count must be greater than 0, found {0}")]
    InvalidChannels(u16),

    /// Fewer samples were provided than the number of channels requires.
    #[error("insufficient samples provided: required {required}, found {actual}")]
    TooFewSamples { required: usize, actual: usize },

    /// The file format is not recognised or supported for reading.
    #[error("Unsupported file format to read")]
    UnsupportedFileFormat,
}

/// Errors related to data validation when performing operations on a [`Wave`].
#[derive(Debug, Error, PartialEq)]
pub enum DataError {
    /// The two waveforms have different sample rates.
    #[error("sample rate mismatch: left={left}Hz, right={right}Hz")]
    SampleRateMismatch { left: u32, right: u32 },

    /// The two waveforms have different sample lengths.
    #[error("sample length mismatch: left={left}, right={right}")]
    LengthMismatch { left: usize, right: usize },

    /// The two waveforms have different channel counts.
    #[error("channel count mismatch: left={left}, right={right}")]
    ChannelMismatch { left: u16, right: u16 },

    /// The waveform contains no samples.
    #[error("operation cannot be performed on empty samples")]
    EmptySamples,

    /// The waveform has fewer samples than the requested split point.
    #[error("operation cannot be performed on too short samples")]
    TooShortSamples,

    /// The separation point was zero, which is not allowed.
    #[error("operation cannot be performed on separate_point variable which is zero")]
    TooShortSeparatePoint,
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

    #[test]
    fn separate() -> Result<(), Box<dyn std::error::Error>> {
        {
            let original = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
            let expected = (
                Wave::new(&[0.5, 0.5, 0.5], 44100, 1)?,
                Wave::new(&[0.5, 0.5], 44100, 1)?,
            );
            let actual = original.separate(3)?;

            assert_eq!(expected, actual);
        }

        Ok(())
    }

    #[test]
    #[should_panic(expected = "The separate_point value must not be zero")]
    fn separate_failure() {
        let original =
            Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1).expect("Failed to create Wave");
        _ = original
            .separate(0)
            .expect("The separate_point value must not be zero");
    }

    mod filter_tests {
        use super::super::{Wave, Waveable};

        trait ToSilent: Waveable {
            fn to_silent(&self) -> Result<Self, Self::Error>
            where
                Self: Waveable + Sized;
        }

        impl ToSilent for Wave {
            fn to_silent(&self) -> Result<Self, Self::Error>
            where
                Self: Waveable + Sized,
            {
                let sample_rate: u32 = self.sample_rate();
                let channels: u16 = self.channels();
                let samples_len: usize = self.samples.len();
                let samples: Vec<f64> = vec![0.0; samples_len];

                let result: Wave = Wave::new(&samples, sample_rate, channels)?;
                Ok(result)
            }
        }

        #[test]
        fn to_silent_filter() -> Result<(), Box<dyn std::error::Error>> {
            let original: Wave = Wave::new(&[0.5, 0.5, 0.5, 0.5, 0.5], 44100, 1)?;
            let actual: Wave = original.to_silent()?;
            let expected: Wave = Wave::new(&[0.0, 0.0, 0.0, 0.0, 0.0], 44100, 1)?;

            assert_eq!(expected, actual);
            Ok(())
        }
    }

    mod read_write_tests {
        use crate::wave::FileFormat;

        use super::super::{Wave, WaveWriteOptions, Waveable, WriteOptions};
        use std::io::{Read, Seek};

        fn read(filepath: &str, expected: &Wave) -> Result<(), Box<dyn std::error::Error>> {
            let file = std::fs::File::open(filepath)?;
            let actual: &Wave = &Wave::read(file)?;

            assert_eq!(expected, actual);
            Ok(())
        }

        fn write<O: WriteOptions>(
            wave: &Wave,
            write_options: O,
            expected: &[u8],
        ) -> Result<(), Box<dyn std::error::Error>> {
            let mut file = tempfile::tempfile()?;
            wave.write(&mut file, write_options)?;
            file.rewind()?;
            let mut written_bytes: Vec<u8> = Vec::new();
            file.read_to_end(&mut written_bytes)?;

            assert_eq!(expected, written_bytes);
            Ok(())
        }

        #[test]
        fn _10_samples_8bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-8bit-PCM.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::PCM, 8);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 46, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1,
                    0, 1, 0, 68, 172, 0, 0, 68, 172, 0, 0, 1, 0, 8, 0, 100, 97, 116, 97, 10, 0, 0,
                    0, 128, 131, 134, 137, 140, 143, 146, 149, 152, 155,
                ],
            )?;
            Ok(())
        }

        #[test]
        fn _10_samples_16bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-16bit-PCM.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::PCM, 16);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 56, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1,
                    0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0, 100, 97, 116, 97, 20, 0, 0,
                    0, 0, 0, 53, 3, 103, 6, 147, 9, 179, 12, 204, 15, 204, 18, 195, 21, 156, 24,
                    97, 27,
                ],
            )?;
            Ok(())
        }

        #[test]
        fn _10_samples_24bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-24bit-PCM.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::PCM, 24);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 66, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1,
                    0, 1, 0, 68, 172, 0, 0, 204, 4, 2, 0, 3, 0, 24, 0, 100, 97, 116, 97, 30, 0, 0,
                    0, 0, 0, 0, 37, 53, 3, 15, 103, 6, 140, 146, 9, 99, 180, 12, 120, 201, 15, 171,
                    206, 18, 239, 192, 21, 91, 157, 24, 253, 96, 27,
                ],
            )?;
            Ok(())
        }

        #[test]
        fn _10_samples_32bit_pcm() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-32bit-PCM.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::PCM, 32);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 76, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1,
                    0, 1, 0, 68, 172, 0, 0, 16, 177, 2, 0, 4, 0, 32, 0, 100, 97, 116, 97, 40, 0, 0,
                    0, 0, 0, 0, 0, 228, 36, 53, 3, 240, 15, 103, 6, 128, 138, 146, 9, 112, 100,
                    180, 12, 96, 119, 201, 15, 160, 169, 206, 18, 128, 241, 192, 21, 128, 88, 157,
                    24, 32, 254, 96, 27,
                ],
            )?;
            Ok(())
        }

        #[test]
        fn _10_samples_32bit_ieeefloat() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-32bit-IEEEFloat.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::IEEEFloat, 32);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 116, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0,
                    3, 0, 1, 0, 68, 172, 0, 0, 16, 177, 2, 0, 4, 0, 32, 0, 100, 97, 116, 97, 80, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 39, 169, 153, 63, 0, 0, 0, 192, 63,
                    156, 169, 63, 0, 0, 0, 0, 21, 37, 179, 63, 0, 0, 0, 224, 200, 104, 185, 63, 0,
                    0, 0, 192, 238, 146, 191, 63, 0, 0, 0, 160, 169, 206, 194, 63, 0, 0, 0, 128,
                    241, 192, 197, 63, 0, 0, 0, 128, 88, 157, 200, 63, 0, 0, 0, 32, 254, 96, 203,
                    63,
                ],
            )?;
            Ok(())
        }

        #[test]
        fn _10_samples_64bit_ieeefloat() -> Result<(), Box<dyn std::error::Error>> {
            const FILEPATH: &str = "./assets/10-samples-64bit-IEEEFloat.wav";
            let file_format = FileFormat::wav(rustttwavvv::FormatCode::IEEEFloat, 64);
            let options = WaveWriteOptions::new(file_format);
            let expected = Wave::new(
                &[
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
                44100,
                1,
            )?;

            read(FILEPATH, &expected)?;
            write(
                &expected,
                options,
                &[
                    82, 73, 70, 70, 116, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0,
                    3, 0, 1, 0, 68, 172, 0, 0, 32, 98, 5, 0, 8, 0, 64, 0, 100, 97, 116, 97, 80, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 39, 169, 153, 63, 0, 0, 0, 192, 63,
                    156, 169, 63, 0, 0, 0, 0, 21, 37, 179, 63, 0, 0, 0, 224, 200, 104, 185, 63, 0,
                    0, 0, 192, 238, 146, 191, 63, 0, 0, 0, 160, 169, 206, 194, 63, 0, 0, 0, 128,
                    241, 192, 197, 63, 0, 0, 0, 128, 88, 157, 200, 63, 0, 0, 0, 32, 254, 96, 203,
                    63,
                ],
            )?;
            Ok(())
        }
    }
}
