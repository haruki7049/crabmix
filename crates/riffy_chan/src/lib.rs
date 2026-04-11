//! # riffy_chan
//!
//! `riffy_chan` is a library for decoding and encoding
//! [RIFF](https://en.wikipedia.org/wiki/Resource_Interchange_File_Format)
//! (Resource Interchange File Format) files in Rust.
//!
//! RIFF is a generic container format used by many well-known file types such
//! as WAV audio and WebP images. A RIFF file is built from a tree of *chunks*,
//! each identified by a four-character code ([`FourCC`]) and carrying either
//! raw byte data or a list of nested chunks.
//!
//! ## Core types
//!
//! - [`FourCC`] – a four-byte identifier used to label every chunk.
//! - [`Chunk`] – an enum representing the three kinds of RIFF chunk: a plain
//!   data chunk, a `LIST` chunk (containing sub-chunks), and the root `RIFF`
//!   chunk.
//!
//! ## Parsing example
//!
//! ```rust
//! use riffy_chan::Chunk;
//!
//! // Raw bytes of a minimal RIFF file (e.g. read from disk with std::fs::read).
//! let bytes: &[u8] =
//!     b"RIFF\x14\x00\x00\x00TESTfmt \x00\x00\x00\x00data\x00\x00\x00\x00";
//!
//! let chunk = Chunk::try_from(bytes).expect("valid RIFF data");
//! ```

use std::fmt;
use std::io::{BufReader, BufWriter, Read, Write};
use thiserror::Error;

// ---------------------------------------------------------------------------
// FourCC
// ---------------------------------------------------------------------------

/// A four-character code (FourCC) used to identify RIFF chunks.
///
/// A `FourCC` is exactly four bytes long and is typically written as an ASCII
/// string such as `"RIFF"`, `"LIST"`, `"fmt "`, or `"data"`.  It acts as the
/// type tag for every chunk in a RIFF file.
///
/// # Examples
///
/// ```rust
/// use riffy_chan::FourCC;
///
/// let cc = FourCC::from(*b"fmt ");
/// assert_eq!(cc.as_bytes(), b"fmt ");
/// assert_eq!(format!("{cc}"), "fmt ");
/// ```
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub struct FourCC {
    inner: [u8; 4],
}

impl FourCC {
    /// Creates a new `FourCC` from a four-byte array.
    ///
    /// This is a `const` alternative to [`From<[u8; 4]>`].
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self { inner: bytes }
    }

    /// Returns a reference to the underlying four bytes.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.inner
    }
}

/// Displays the FourCC as an ASCII string.
///
/// Non-ASCII bytes are replaced with `.` for readability.
impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &b in &self.inner {
            if b.is_ascii_graphic() || b == b' ' {
                write!(f, "{}", b as char)?;
            } else {
                write!(f, ".")?;
            }
        }
        Ok(())
    }
}

/// Returns a reference to the underlying four-byte array.
impl AsRef<[u8]> for FourCC {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

/// Converts a [`FourCC`] into its raw four-byte representation.
impl From<FourCC> for Vec<u8> {
    fn from(value: FourCC) -> Self {
        value.inner.to_vec()
    }
}

/// Converts a [`FourCC`] into its raw four-byte array.
impl From<FourCC> for [u8; 4] {
    fn from(value: FourCC) -> Self {
        value.inner
    }
}

/// Creates a [`FourCC`] from a fixed-size four-byte array.
impl From<[u8; 4]> for FourCC {
    fn from(from: [u8; 4]) -> Self {
        Self { inner: from }
    }
}

/// Creates a [`FourCC`] from a reference to a fixed-size four-byte array.
impl From<&[u8; 4]> for FourCC {
    fn from(from: &[u8; 4]) -> Self {
        Self { inner: *from }
    }
}

/// Attempts to create a [`FourCC`] from a byte slice.
///
/// # Errors
///
/// Returns [`FourCCError::InvalidLength`] when the slice does not contain
/// exactly four bytes.
impl TryFrom<&[u8]> for FourCC {
    type Error = FourCCError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let slice: [u8; 4] = value
            .try_into()
            .map_err(|_| FourCCError::InvalidLength {
                actual: value.len(),
            })?;
        Ok(Self::from(slice))
    }
}

/// Errors that can occur when creating a [`FourCC`] from raw bytes.
#[derive(Debug, Error)]
pub enum FourCCError {
    /// The source data did not contain exactly four bytes.
    #[error("expected exactly 4 bytes for FourCC, got {actual}")]
    InvalidLength {
        /// The actual number of bytes provided.
        actual: usize,
    },
}

// ---------------------------------------------------------------------------
// ChunkError
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing or writing RIFF [`Chunk`] data.
#[derive(Debug, Error)]
pub enum ChunkError {
    /// The input buffer is too short to contain the expected data.
    #[error("buffer too short: need at least {needed} bytes, got {actual}")]
    BufferTooShort {
        /// Minimum number of bytes required.
        needed: usize,
        /// Actual number of bytes available.
        actual: usize,
    },

    /// A FourCC value could not be parsed.
    #[error("invalid FourCC: {0}")]
    FourCC(#[from] FourCCError),

    /// An I/O error occurred while reading or writing.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// A single node in a RIFF chunk tree.
///
/// RIFF files are hierarchical: the file starts with a root [`Chunk::Riff`]
/// that contains zero or more child chunks, which may themselves be
/// [`Chunk::List`] chunks with further children.
///
/// # Parsing
///
/// Use the [`TryFrom<&[u8]>`] implementation to parse raw bytes:
///
/// ```rust
/// use riffy_chan::{Chunk, FourCC};
///
/// let bytes: &[u8] = b"fmt \x0c\x00\x00\x00EXAMPLE_DATA";
/// let chunk = Chunk::try_from(bytes).unwrap();
///
/// assert_eq!(
///     chunk,
///     Chunk::Chunk {
///         four_cc: FourCC::from(*b"fmt "),
///         data: b"EXAMPLE_DATA".to_vec(),
///     }
/// );
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Chunk {
    /// A basic RIFF chunk with a FourCC identifier and data payload.
    Chunk { four_cc: FourCC, data: Vec<u8> },

    /// A LIST chunk containing a list of sub-chunks.
    List { chunks: Vec<Chunk> },

    /// A RIFF chunk representing the root container of a RIFF file.
    Riff { four_cc: FourCC, chunks: Vec<Chunk> },
}

// -- Reading ----------------------------------------------------------------

impl Chunk {
    /// Reads all bytes from a reader and parses a [`Chunk`].
    ///
    /// # Errors
    ///
    /// Returns [`ChunkError`] if the I/O read fails or the bytes are malformed.
    pub fn from_reader<R: Read>(reader: R) -> Result<Chunk, ChunkError> {
        let mut buf_reader = BufReader::new(reader);
        let mut bytes: Vec<u8> = Vec::new();
        buf_reader.read_to_end(&mut bytes)?;

        Chunk::try_from(bytes.as_slice())
    }
}

// -- Writing ----------------------------------------------------------------

impl Chunk {
    /// Serialises this chunk and writes all bytes to `writer`.
    ///
    /// The chunk is converted to its RIFF byte representation via
    /// [`From<&Chunk> for Vec<u8>`] and then written in full to the provided
    /// writer.  The writer is wrapped in a [`BufWriter`] internally, so
    /// calling this on an unbuffered sink (such as a [`std::fs::File`]) is
    /// efficient.
    ///
    /// # Errors
    ///
    /// Returns [`ChunkError::Io`] if the write operation fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use riffy_chan::{Chunk, FourCC};
    ///
    /// let chunk = Chunk::Chunk {
    ///     four_cc: FourCC::from(*b"data"),
    ///     data: vec![1, 2, 3, 4],
    /// };
    ///
    /// let mut buf: Vec<u8> = Vec::new();
    /// chunk.to_write(&mut buf).expect("write succeeded");
    /// assert_eq!(&buf[..4], b"data");
    /// ```
    pub fn to_write<W: Write>(&self, writer: &mut W) -> Result<(), ChunkError> {
        let result: Vec<u8> = self.into();
        let mut w = BufWriter::new(writer);
        w.write_all(&result)?;
        w.flush()?;
        Ok(())
    }
}

// -- Size -------------------------------------------------------------------

impl Chunk {
    /// Returns the data size of this chunk (excluding the FourCC and 4-byte
    /// size header).
    ///
    /// For a plain [`Chunk::Chunk`] this is the length of the payload. For
    /// [`Chunk::Riff`] it is the form-type FourCC (4 bytes) plus all
    /// children's full sizes. For [`Chunk::List`] it is the sum of all
    /// children's full sizes.
    pub fn size(&self) -> u32 {
        match self {
            Self::Chunk { data, .. } => data.len() as u32,
            Self::Riff { chunks, .. } => {
                const FOUR_CC_LEN: u32 = 4;
                let chunks_bytes: u32 = chunks.iter().map(|c| c.full_size()).sum();
                chunks_bytes + FOUR_CC_LEN
            }
            Self::List { chunks } => {
                chunks.iter().map(|c| c.full_size()).sum()
            }
        }
    }

    /// Returns the total serialised byte size including the 8-byte header
    /// (FourCC + size field).
    fn full_size(&self) -> u32 {
        self.size() + 8
    }
}

// -- Parsing (TryFrom) -----------------------------------------------------

/// Parses a [`Chunk`] from a byte slice.
///
/// The first four bytes are inspected to determine the chunk type:
/// - `b"RIFF"` → [`Chunk::Riff`]
/// - `b"LIST"` → [`Chunk::List`]
/// - anything else → [`Chunk::Chunk`]
///
/// # Errors
///
/// Returns [`ChunkError`] if the bytes are malformed (e.g. truncated or
/// invalid FourCC data).
impl TryFrom<&[u8]> for Chunk {
    type Error = ChunkError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 8 {
            return Err(ChunkError::BufferTooShort {
                needed: 8,
                actual: value.len(),
            });
        }

        match &value[0..4] {
            b"RIFF" => Self::parse_riff(value),
            b"LIST" => Self::parse_list(value),
            _ => Self::parse_chunk(value),
        }
    }
}

/// Parses a [`Chunk`] from a `Vec<u8>`.
///
/// Delegates to [`TryFrom<&[u8]>`].
impl TryFrom<Vec<u8>> for Chunk {
    type Error = ChunkError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_slice())
    }
}

/// Parses a [`Chunk`] from a reference to a `Vec<u8>`.
///
/// Delegates to [`TryFrom<&[u8]>`].
impl TryFrom<&Vec<u8>> for Chunk {
    type Error = ChunkError;

    fn try_from(value: &Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_slice())
    }
}

// -- Serialisation (From) ---------------------------------------------------

/// Serialises a [`Chunk`] into its RIFF byte representation.
///
/// The returned bytes are a complete, self-contained RIFF chunk including the
/// FourCC identifier, the little-endian 32-bit size field, and the payload.
/// For [`Chunk::Riff`] chunks an extra padding byte (`0x00`) is appended when
/// the payload size is odd, as required by the RIFF specification.
impl From<Chunk> for Vec<u8> {
    fn from(value: Chunk) -> Self {
        Vec::<u8>::from(&value)
    }
}

/// Serialises a [`Chunk`] reference into its RIFF byte representation.
impl From<&Chunk> for Vec<u8> {
    fn from(value: &Chunk) -> Self {
        let size = value.size();

        match value {
            Chunk::List { chunks } => Chunk::encode_list(chunks, size),
            Chunk::Riff { four_cc, chunks } => Chunk::encode_riff(four_cc, chunks, size),
            Chunk::Chunk { four_cc, data } => Chunk::encode_chunk(four_cc, data, size),
        }
    }
}

// -- Encoding helpers -------------------------------------------------------

impl Chunk {
    /// Serialises a `LIST` chunk into its RIFF byte representation.
    fn encode_list(chunks: &[Chunk], size: u32) -> Vec<u8> {
        let mut result = Vec::with_capacity(8 + size as usize);
        result.extend_from_slice(b"LIST");
        result.extend_from_slice(&size.to_le_bytes());

        for chunk in chunks {
            let b: Vec<u8> = chunk.into();
            result.extend(b);
        }

        result
    }

    /// Serialises a `RIFF` root chunk into its RIFF byte representation.
    fn encode_riff(four_cc: &FourCC, chunks: &[Chunk], size: u32) -> Vec<u8> {
        let is_odd_size = size % 2 == 1;
        let written_size = if is_odd_size { size + 1 } else { size };

        let mut result = Vec::with_capacity(8 + written_size as usize);
        result.extend_from_slice(b"RIFF");
        result.extend_from_slice(&written_size.to_le_bytes());
        result.extend_from_slice(four_cc.as_bytes());

        for chunk in chunks {
            let b: Vec<u8> = chunk.into();
            result.extend(b);
        }

        if is_odd_size {
            result.push(0x00);
        }
        result
    }

    /// Serialises a plain data chunk into its RIFF byte representation.
    fn encode_chunk(four_cc: &FourCC, data: &[u8], size: u32) -> Vec<u8> {
        let mut result = Vec::with_capacity(8 + size as usize);
        result.extend_from_slice(four_cc.as_bytes());
        result.extend_from_slice(&size.to_le_bytes());
        result.extend_from_slice(data);
        result
    }
}

// -- Parsing helpers --------------------------------------------------------

impl Chunk {
    /// Parses a plain data chunk from a byte buffer.
    fn parse_chunk(buffer: &[u8]) -> Result<Chunk, ChunkError> {
        if buffer.len() < 8 {
            return Err(ChunkError::BufferTooShort {
                needed: 8,
                actual: buffer.len(),
            });
        }

        let four_cc = FourCC::try_from(&buffer[0..4])?;
        let size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;

        if buffer.len() < 8 + size {
            return Err(ChunkError::BufferTooShort {
                needed: 8 + size,
                actual: buffer.len(),
            });
        }

        let data = buffer[8..8 + size].to_vec();
        Ok(Chunk::Chunk { four_cc, data })
    }

    /// Parses a `LIST` chunk from a byte buffer.
    fn parse_list(buffer: &[u8]) -> Result<Chunk, ChunkError> {
        if buffer.len() < 8 {
            return Err(ChunkError::BufferTooShort {
                needed: 8,
                actual: buffer.len(),
            });
        }

        let list_size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

        let mut chunks = Vec::new();
        let mut offset: u32 = 8;

        while list_size >= offset {
            let chunk = Chunk::parse_chunk(&buffer[offset as usize..])?;
            let chunk_size = chunk.full_size();

            chunks.push(chunk);
            offset += chunk_size;
        }

        Ok(Chunk::List { chunks })
    }

    /// Parses a `RIFF` root chunk from a byte buffer.
    fn parse_riff(buffer: &[u8]) -> Result<Chunk, ChunkError> {
        if buffer.len() < 12 {
            return Err(ChunkError::BufferTooShort {
                needed: 12,
                actual: buffer.len(),
            });
        }

        let riff_size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
        let four_cc = FourCC::try_from(&buffer[8..12])?;

        let mut chunks = Vec::new();
        let mut offset: u32 = 12;

        while riff_size >= offset {
            let chunk = Chunk::parse_chunk(&buffer[offset as usize..])?;
            let chunk_size = chunk.full_size();
            chunks.push(chunk);
            offset += chunk_size;
        }

        Ok(Chunk::Riff { four_cc, chunks })
    }
}

#[cfg(test)]
mod four_cc_tests {
    use super::FourCC;

    #[test]
    fn four_cc_default() {
        let expected = FourCC::default();
        let actual = FourCC::from([0, 0, 0, 0]);

        assert_eq!(expected, actual);
    }

    #[test]
    fn four_cc_from() {
        let expected = FourCC {
            inner: [4, 4, 4, 4],
        };
        let actual = FourCC::from([4, 4, 4, 4]);

        assert_eq!(expected, actual);
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::{Chunk, FourCC};
    use std::{
        fs::File,
        io::{BufReader, Read, Seek},
    };

    #[test]
    fn load_chunk() -> Result<(), Box<dyn std::error::Error>> {
        {
            let expected = b"fmt \x0c\x00\x00\x00EXAMPLE_DATA";
            let actual = include_bytes!("./assets/chunk.riff");
            assert_eq!(expected, actual);
        }

        {
            let bytes = include_bytes!("./assets/chunk.riff");
            let expected = Chunk::Chunk {
                four_cc: FourCC::from(*b"fmt "),
                data: b"EXAMPLE_DATA".to_vec(),
            };
            let actual = Chunk::try_from(bytes.to_vec())?;
            assert_eq!(expected, actual);
        }

        Ok(())
    }

    #[test]
    fn load_list_chunk() -> Result<(), Box<dyn std::error::Error>> {
        {
            let expected =
                b"LIST\x28\x00\x00\x00fmt \x0c\x00\x00\x00EXAMPLE_DATAfmt \x0c\x00\x00\x00EXAMPLE_DATA";
            let bytes = include_bytes!("./assets/list_chunk.riff");
            assert_eq!(bytes, expected);
        }

        {
            let bytes = include_bytes!("./assets/list_chunk.riff");
            let expected = Chunk::List {
                chunks: vec![
                    Chunk::Chunk {
                        four_cc: FourCC::from(b"fmt "),
                        data: b"EXAMPLE_DATA".to_vec(),
                    },
                    Chunk::Chunk {
                        four_cc: FourCC::from(b"fmt "),
                        data: b"EXAMPLE_DATA".to_vec(),
                    },
                ],
            };
            let actual = Chunk::try_from(bytes.to_vec())?;
            assert_eq!(expected, actual);
        }

        Ok(())
    }

    #[test]
    fn load_riff_chunk() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = include_bytes!("./assets/riff_chunk.riff");

        let expected = b"RIFF\x14\x00\x00\x00TESTfmt \x00\x00\x00\x00data\x00\x00\x00\x00";
        assert_eq!(bytes, expected);

        let expected = Chunk::Riff {
            four_cc: FourCC::from(b"TEST"),
            chunks: vec![
                Chunk::Chunk {
                    four_cc: FourCC::from(b"fmt "),
                    data: vec![],
                },
                Chunk::Chunk {
                    four_cc: FourCC::from(b"data"),
                    data: vec![],
                },
            ],
        };
        let actual = Chunk::try_from(bytes.to_vec())?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn load_webp() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = include_bytes!("./assets/test_DJ.webp");
        _ = Chunk::try_from(bytes.to_vec())?;
        Ok(())
    }

    #[test]
    fn load_wave() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = include_bytes!("./assets/sinewave.wav");
        _ = Chunk::try_from(bytes.to_vec())?;
        Ok(())
    }

    #[test]
    fn load_10_samples_wave() -> Result<(), Box<dyn std::error::Error>> {
        {
            let expected =
                    b"RIFF\x38\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x14\x00\x00\x00\x01\x00\x33\x03\x69\x06\x91\x09\xb7\x0c\xc6\x0f\xd3\x12\xbc\x15\xa1\x18\x60\x1b";
            let bytes = include_bytes!("./assets/10-samples.wav");
            assert_eq!(bytes, expected);
        }

        {
            let bytes = include_bytes!("./assets/10-samples.wav");
            let expected = Chunk::Riff {
                four_cc: FourCC::from(b"WAVE"),
                chunks: vec![
                    Chunk::Chunk {
                        four_cc: FourCC::from(b"fmt "),
                        data: vec![1, 0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0],
                    },
                    Chunk::Chunk {
                        four_cc: FourCC::from(b"data"),
                        data: vec![
                            1, 0, 51, 3, 105, 6, 145, 9, 183, 12, 198, 15, 211, 18, 188, 21, 161,
                            24, 96, 27,
                        ],
                    },
                ],
            };
            let actual = Chunk::try_from(bytes.to_vec())?;
            assert_eq!(expected, actual);
        }

        Ok(())
    }

    #[test]
    fn from_reader() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::current_dir()?;
        eprintln!("The current directory is: {:?}", path);

        let expected = Chunk::Riff {
            four_cc: FourCC::from(b"WAVE"),
            chunks: vec![
                Chunk::Chunk {
                    four_cc: FourCC::from(b"fmt "),
                    data: vec![1, 0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0],
                },
                Chunk::Chunk {
                    four_cc: FourCC::from(b"data"),
                    data: vec![
                        1, 0, 51, 3, 105, 6, 145, 9, 183, 12, 198, 15, 211, 18, 188, 21, 161, 24,
                        96, 27,
                    ],
                },
            ],
        };
        let f = File::open("src/assets/10-samples.wav")?;
        let actual = Chunk::from_reader(f)?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn to_write() -> Result<(), Box<dyn std::error::Error>> {
        let expected = include_bytes!("./assets/10-samples.wav").to_vec();

        // Writing to the file
        let mut f: File = tempfile::tempfile()?;
        let chunk = Chunk::try_from(&expected)?;
        chunk.to_write(&mut f)?;

        // Seek back to the start of the file before reading
        f.rewind()?;

        let mut reader = BufReader::new(f);
        let mut buf: Vec<u8> = Vec::new();
        reader.read_to_end(&mut buf)?;

        assert_eq!(expected.len(), buf.len());
        for i in 0..buf.len() {
            assert_eq!(expected[i], buf[i], "i = {}", i);
        }

        Ok(())
    }

    #[test]
    fn chunk_file_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let test_files = [
            "./src/assets/chunk.riff",
            "./src/assets/list_chunk.riff",
            "./src/assets/riff_chunk.riff",
            "./src/assets/10-samples.wav",
            "./src/assets/test_DJ.webp",
        ];

        for path in test_files {
            // Test: Read from file (to_reader)
            let file = File::open(path)?;
            let chunk = Chunk::from_reader(file)?;

            // Test: Write to temp file (to_write)
            let mut temp_file = tempfile::tempfile()?;
            chunk.to_write(&mut temp_file)?;

            // Verify: Written content matches original asset
            temp_file.rewind()?;
            let mut written_bytes = Vec::new();
            temp_file.read_to_end(&mut written_bytes)?;

            let original_bytes = std::fs::read(path)?;

            assert_eq!(original_bytes.len(), written_bytes.len());
            assert_eq!(
                original_bytes, written_bytes,
                "Roundtrip failed for file: {}",
                path
            );
        }

        Ok(())
    }

    #[test]
    fn chunk_try_from_vec_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let test_files = [
            "./src/assets/chunk.riff",
            "./src/assets/list_chunk.riff",
            "./src/assets/riff_chunk.riff",
            "./src/assets/10-samples.wav",
            "./src/assets/test_DJ.webp",
        ];

        for path in test_files {
            let original_bytes = std::fs::read(path)?;

            // Test: TryFrom<Vec<u8>>
            let chunk = Chunk::try_from(original_bytes.clone())?;

            // Test: From<Chunk> for Vec<u8>
            let converted_bytes: Vec<u8> = chunk.into();

            assert_eq!(original_bytes.len(), converted_bytes.len());
            assert_eq!(
                original_bytes, converted_bytes,
                "Vec conversion failed for file: {}",
                path
            );
        }

        Ok(())
    }
}
