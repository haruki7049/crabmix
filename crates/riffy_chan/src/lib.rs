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
//! let bytes: Vec<u8> =
//!     b"RIFF\x14\x00\x00\x00TESTfmt \x00\x00\x00\x00data\x00\x00\x00\x00"
//!         .to_vec();
//!
//! let chunk = Chunk::try_from(bytes).expect("valid RIFF data");
//! ```

use std::io::{BufReader, BufWriter, Read, Write};
use thiserror::Error;

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
/// let bytes: Vec<u8> = cc.into();
/// assert_eq!(bytes, b"fmt ");
/// ```
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct FourCC {
    inner: [u8; 4],
}

/// Converts a [`FourCC`] into its raw four-byte representation.
impl From<FourCC> for Vec<u8> {
    fn from(value: FourCC) -> Self {
        value.inner.to_vec()
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

/// Attempts to create a [`FourCC`] from a `Vec<u8>`.
///
/// # Errors
///
/// Returns [`FourCCTryFromError::InvalidLength`] when the vector does not
/// contain exactly four bytes.
impl TryFrom<Vec<u8>> for FourCC {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let actual_len = value.len();
        if actual_len != 4 {
            return Err(Box::new(FourCCTryFromError::InvalidLength {
                actual: actual_len,
            }));
        }

        let slice: [u8; 4] = value
            .try_into()
            .map_err(|actual| FourCCTryFromError::InvalidSlice { actual })?;
        Ok(Self::from(slice))
    }
}

/// Errors that can occur when converting a `Vec<u8>` into a [`FourCC`].
#[derive(Debug, Error)]
pub enum FourCCTryFromError {
    /// The source vector did not contain exactly four bytes.
    #[error("Invalid length of Vec<u8>, expected 4 but actually {}", actual)]
    InvalidLength { actual: usize },

    /// The source vector could not be converted into a `[u8; 4]` slice.
    #[error("Invalid slice. The received Vec<u8> is: {:?}", actual)]
    InvalidSlice { actual: Vec<u8> },
}

/// A single node in a RIFF chunk tree.
///
/// RIFF files are hierarchical: the file starts with a root [`Chunk::Riff`]
/// that contains zero or more child chunks, which may themselves be
/// [`Chunk::List`] chunks with further children.
///
/// # Parsing
///
/// Use the [`TryFrom<Vec<u8>>`] implementation to parse raw bytes:
///
/// ```rust
/// use riffy_chan::{Chunk, FourCC};
///
/// let bytes = b"fmt \x0c\x00\x00\x00EXAMPLE_DATA".to_vec();
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

impl Chunk {
    /// Read bytes from a reader, and create a Chunk data.
    pub fn from_reader<R: Read>(reader: R) -> Result<Chunk, Box<dyn std::error::Error>> {
        let mut buf_reader = BufReader::new(reader);
        let mut bytes: Vec<u8> = Vec::new();
        buf_reader.read_to_end(&mut bytes)?;

        let result: Chunk = Chunk::try_from(bytes)?;
        Ok(result)
    }
}

impl Chunk {
    pub fn to_write<W: Write>(self, write: &mut W) -> Result<(), Box<dyn std::error::Error>> {
        let result: Vec<u8> = self.try_into()?;
        let mut w = BufWriter::new(write);
        w.write_all(&result)?;
        w.flush()?;

        Ok(())
    }
}

impl Chunk {
    /// Returns the total serialised byte size of this chunk, including the
    /// FourCC, size field, and payload.
    pub fn size(&self) -> Result<u32, Box<dyn std::error::Error>> {
        match self {
            Self::Chunk { data, .. } => Self::chunk_size(data),
            Self::Riff { chunks, .. } => Self::riff_chunk_size(chunks),
            Self::List { chunks } => Self::list_chunk_size(chunks),
        }
    }

    /// Calculates the byte size of a plain data chunk.
    ///
    /// Layout: `[FourCC (4)] [size (4)] [data (n)]`
    fn chunk_size(data: &[u8]) -> Result<u32, Box<dyn std::error::Error>> {
        let data_bytes: u32 = data.len().try_into()?;
        Ok(data_bytes)
    }

    /// Calculates the byte size of a RIFF root chunk.
    ///
    /// Layout: `[RIFF (4)] [size (4)] [FourCC (4)] [chunks…]`
    fn riff_chunk_size(chunks: &[Chunk]) -> Result<u32, Box<dyn std::error::Error>> {
        const RIFF_BYTES: u32 = 4;
        const SIZE_BYTES: u32 = 4;
        const FOUR_CC_BYTES: u32 = 4;
        let chunks_bytes: u32 = chunks
            .iter()
            .map(|chunk| chunk.size())
            .sum::<Result<u32, Box<dyn std::error::Error>>>()?;

        Ok(chunks_bytes + RIFF_BYTES + FOUR_CC_BYTES + SIZE_BYTES)
    }

    /// Calculates the byte size of a LIST chunk.
    ///
    /// Layout: `[LIST (4)] [size (4)] [FourCC (4)] [sub-chunks…]`
    fn list_chunk_size(chunks: &[Chunk]) -> Result<u32, Box<dyn std::error::Error>> {
        const LIST_BYTES: u32 = 4;
        const SIZE_BYTES: u32 = 4;
        const FOUR_CC_BYTES: u32 = 4;
        let chunks_bytes: u32 = chunks
            .iter()
            .map(|chunk| chunk.size())
            .sum::<Result<u32, Box<dyn std::error::Error>>>()?;

        Ok(chunks_bytes + LIST_BYTES + FOUR_CC_BYTES + SIZE_BYTES)
    }
}

/// Parses a [`Chunk`] from raw bytes.
///
/// The first four bytes are inspected to determine the chunk type:
/// - `b"RIFF"` → [`Chunk::Riff`]
/// - `b"LIST"` → [`Chunk::List`]
/// - anything else → [`Chunk::Chunk`]
///
/// # Errors
///
/// Returns an error if the bytes are malformed (e.g. truncated size field or
/// invalid FourCC data).
impl TryFrom<Vec<u8>> for Chunk {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let riff = b"RIFF";
        let list = b"LIST";
        let four_cc_raw = value[0..4].to_vec();

        match four_cc_raw {
            r if r == riff => Self::parse_riff(&value),
            l if l == list => Self::parse_list(&value),
            _ => Self::parse_chunk(&value),
        }
    }
}

impl TryFrom<&Vec<u8>> for Chunk {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Vec<u8>) -> Result<Self, Self::Error> {
        let riff = b"RIFF";
        let list = b"LIST";
        let four_cc_raw = value[0..4].to_vec();

        match four_cc_raw {
            r if r == riff => Self::parse_riff(value),
            l if l == list => Self::parse_list(value),
            _ => Self::parse_chunk(value),
        }
    }
}

impl TryFrom<Chunk> for Vec<u8> {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Chunk) -> Result<Self, Self::Error> {
        let size: u32 = value.size()?;

        match value {
            Chunk::List { chunks } => Chunk::try_from_list_chunk(chunks, size),
            Chunk::Riff { four_cc, chunks } => Chunk::try_from_riff_chunk(four_cc, chunks, size),
            Chunk::Chunk { four_cc, data } => Ok(Chunk::try_from_chunk(four_cc, data, size)),
        }
    }
}

impl Chunk {
    fn try_from_list_chunk(
        chunks: Vec<Chunk>,
        size: u32,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        const LIST_BYTES: &[u8; 4] = b"LIST";
        let size_raw: Vec<u8> = size.to_le_bytes().to_vec(); // RIFF chunk's size
        let children_chunks_raw: Vec<u8> = chunks
            .iter()
            .map(|chunk: &Chunk| {
                let bytes: Vec<u8> = chunk.clone().try_into()?;
                Ok(bytes)
            })
            .collect::<Result<Vec<Vec<u8>>, Box<dyn std::error::Error>>>()?
            .into_iter()
            .flatten()
            .collect();

        let result: Vec<u8> = [LIST_BYTES.to_vec(), size_raw, children_chunks_raw].concat();
        Ok(result)
    }

    fn try_from_riff_chunk(
        four_cc: FourCC,
        chunks: Vec<Chunk>,
        size: u32,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        const RIFF_BYTES: &[u8; 4] = b"RIFF";
        let four_cc_raw: Vec<u8> = four_cc.into();
        let size_raw: Vec<u8> = size.to_le_bytes().to_vec(); // RIFF chunk's size
        let children_chunks_raw: Vec<u8> = chunks
            .iter()
            .map(|chunk: &Chunk| {
                let bytes: Vec<u8> = chunk.clone().try_into()?;
                Ok(bytes)
            })
            .collect::<Result<Vec<Vec<u8>>, Box<dyn std::error::Error>>>()?
            .into_iter()
            .flatten()
            .collect();

        let result: Vec<u8> = [
            RIFF_BYTES.to_vec(),
            size_raw,
            four_cc_raw,
            children_chunks_raw,
        ]
        .concat();

        Ok(result)
    }

    fn try_from_chunk(four_cc: FourCC, data: Vec<u8>, size: u32) -> Vec<u8> {
        let four_cc_raw: Vec<u8> = four_cc.into();
        let size_raw: Vec<u8> = size.to_le_bytes().to_vec();
        let result: Vec<u8> = [four_cc_raw, size_raw, data].concat();

        result
    }
}

impl Chunk {
    /// Parses a plain data chunk from a byte buffer.
    ///
    /// Reads the FourCC (bytes 0–3), the little-endian size (bytes 4–7), and
    /// then `size` bytes of payload data.
    fn parse_chunk(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let four_cc_raw = buffer[0..4].to_vec();
        let four_cc = FourCC::try_from(four_cc_raw)?;

        let size = u32::from_le_bytes(buffer[4..8].try_into()?) as usize;
        let data = buffer[8..8 + size].to_vec();

        Ok(Chunk::Chunk { four_cc, data })
    }

    /// Parses a `LIST` chunk from a byte buffer.
    ///
    /// The LIST header occupies 8 bytes (`LIST` + size); the remaining bytes
    /// up to `size` are parsed as a sequence of plain data chunks.
    fn parse_list(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let size: u32 = u32::from_le_bytes(buffer[4..8].try_into()?);
        let mut chunks = Vec::new();
        let mut offset: u32 = 8;

        while offset < size {
            let chunk = Chunk::parse_chunk(&buffer[offset as usize..])?;
            let chunk_size = Chunk::size(&chunk)?;

            chunks.push(chunk);
            offset += chunk_size;
        }

        Ok(Chunk::List { chunks })
    }

    /// Parses a `RIFF` root chunk from a byte buffer.
    ///
    /// The RIFF header occupies 12 bytes (`RIFF` + size + form-type FourCC);
    /// the remaining bytes up to `size + 4` are parsed as a sequence of nested
    /// chunks.
    fn parse_riff(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let size = u32::from_le_bytes(buffer[4..8].try_into()?);
        let mut chunks = Vec::new();

        let four_cc_raw = buffer[8..12].to_vec();
        let four_cc = FourCC::try_from(four_cc_raw)?;
        let mut offset: u32 = 12;

        while offset < size + 4 {
            let chunk = Chunk::parse_chunk(&buffer[offset as usize..])?;
            let chunk_size = Chunk::size(&chunk)?;
            chunks.push(chunk);
            offset += chunk_size;
        }

        Ok(Chunk::Riff { four_cc, chunks })
    }
}

/// Errors that can occur when converting raw bytes into a [`Chunk`].
#[derive(Debug, Error)]
pub enum ChunkTryFromError {
    /// The buffer did not contain a FourCC identifier.
    #[error("There is not a FourCC data")]
    NoFourCC,

    /// The buffer did not conform to the expected RIFF chunk layout.
    #[error("Invalid format")]
    InvalidFormat,

    /// The FourCC identifier was not recognised.
    #[error("Invalid Id")]
    InvalidId,

    /// The size field in the chunk header does not match the actual data length.
    #[error("Size mismatch between size signature and actual data")]
    SizeMismatch,
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
        {
            let expected = b"RIFF\x14\x00\x00\x00TESTfmt \x00\x00\x00\x00data\x00\x00\x00\x00";
            let bytes = include_bytes!("./assets/riff_chunk.riff");
            assert_eq!(bytes, expected);
        }

        {
            let bytes = include_bytes!("./assets/riff_chunk.riff");
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
        }

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

        dbg!(&expected);
        dbg!(&buf);

        assert_eq!(expected.len(), buf.len());
        for i in 0..buf.len() {
            assert_eq!(expected[i], buf[i]);
        }

        Ok(())
    }
}
