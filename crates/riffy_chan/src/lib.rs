//! # riffy_chan

use thiserror::Error;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FourCC {
    inner: [u8; 4],
}

impl From<FourCC> for Vec<u8> {
    fn from(value: FourCC) -> Self {
        value.inner.to_vec()
    }
}

impl From<[u8; 4]> for FourCC {
    fn from(from: [u8; 4]) -> Self {
        Self { inner: from }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Chunk {
    /// A basic RIFF chunk with a FourCC identifier and data payload.
    Chunk { four_cc: FourCC, data: Vec<u8> },

    /// A LIST chunk containing a list of sub-chunks.
    List { chunks: Vec<Chunk> },

    /// A RIFF chunk representing the root container of a RIFF file.
    Riff { four_cc: FourCC, chunks: Vec<Chunk> },
}

impl TryFrom<Vec<u8>> for Chunk {
    type Error = ChunkTryFromError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let riff: FourCC = FourCC::from([b'R', b'I', b'F', b'F']);
        let list: FourCC = FourCC::from([b'L', b'I', b'S', b'T']);

        let four_cc_raw = value
            .first_chunk::<4>()
            .ok_or(ChunkTryFromError::NoFourCC)?;
        let four_cc: FourCC = FourCC::from(*four_cc_raw);
        let rest: Vec<u8> = value.iter().skip(4).cloned().collect();

        match four_cc {
            r if r == riff => Self::create_riff_chunk(rest),
            l if l == list => Self::create_list_chunk(rest),
            _ => Self::create_chunk(four_cc, rest),
        }
    }
}

impl Chunk {
    fn create_chunk(four_cc: FourCC, data: Vec<u8>) -> Result<Self, ChunkTryFromError> {
        let v = Chunk::Chunk { four_cc, data };
        Ok(v)
    }

    fn create_list_chunk(value: Vec<u8>) -> Result<Self, ChunkTryFromError> {
        let chunks = Self::to_chunk_list(value)?;
        Ok(Self::List { chunks })
    }

    fn create_riff_chunk(value: Vec<u8>) -> Result<Self, ChunkTryFromError> {
        if value.len() < 8 {
            return Err(ChunkTryFromError::InvalidFormat);
        }

        let four_cc_raw = value
            .first_chunk::<4>()
            .ok_or(ChunkTryFromError::NoFourCC)?;
        let four_cc = FourCC::from(*four_cc_raw);
        let rest: Vec<u8> = value.iter().skip(4).cloned().collect();
        let chunks = Self::to_chunk_list(rest)?;

        Ok(Chunk::Riff { four_cc, chunks })
    }

    fn to_chunk_list(bytes: Vec<u8>) -> Result<Vec<Self>, ChunkTryFromError> {
        let mut result: Vec<Chunk> = vec![];

        let mut pos: usize = 0;
        while pos < bytes.len() {
            // Need at least 8 bytes for chunk header (FourCC + size)
            if pos + 8 > bytes.len() {
                // If we have leftover bytes that can't form a valid chunk header,
                // this is not necessarily an error - it could be padding
                // But we should check if there are any non-zero bytes
                let mut has_data = false;
                for &b in &bytes[pos..] {
                    if b != 0 {
                        has_data = true;
                        break;
                    }
                }
                if has_data {
                    return Err(ChunkTryFromError::InvalidFormat);
                }
                break;
            }

            let id = *bytes
                .first_chunk::<4>()
                .ok_or(ChunkTryFromError::InvalidId)?;
            let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            let next_pos = pos + 8 + size;

            if next_pos > bytes.len() {
                return Err(ChunkTryFromError::SizeMismatch);
            }

            let four_cc = FourCC::from(id);
            let chunk_data = bytes[pos + 8..next_pos].to_vec();
            let chunk = Self::create_chunk(four_cc, chunk_data)?;
            result.push(chunk);

            pos = next_pos;
        }

        Ok(result)
    }
}

#[derive(Debug, Error)]
pub enum ChunkTryFromError {
    #[error("There is not a FourCC data")]
    NoFourCC,

    #[error("Invalid format")]
    InvalidFormat,

    #[error("Invalid Id")]
    InvalidId,

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

    #[test]
    fn load_chunk() {
        let expected = b"fmt \x0c\x00\x00\x00EXAMPLE_DATA";
        let bytes = include_bytes!("./assets/chunk.riff");
        assert_eq!(bytes, expected);

        let expected = Chunk::Chunk {
            four_cc: FourCC::from(*b"fmt "),
            data: b"\x0c\x00\x00\x00EXAMPLE_DATA".to_vec(),
        };
        let actual = Chunk::try_from(bytes.to_vec()).expect("Failed to parse bytes into Chunk");

        assert_eq!(expected, actual);
    }

    #[test]
    fn load_list_chunk() {
        let expected =
            b"LIST\x28\x00\x00\x00fmt \x0c\x00\x00\x00EXAMPLE_DATAfmt \x0c\x00\x00\x00EXAMPLE_DATA";
        let bytes = include_bytes!("./assets/list_chunk.riff");
        assert_eq!(bytes, expected);

        let expected = Chunk::Chunk {
            four_cc: FourCC::from(*b"fmt "),
            data: b"\x0c\x00\x00\x00EXAMPLE_DATA".to_vec(),
        };
        let actual = Chunk::try_from(bytes.to_vec()).expect("Failed to parse bytes into Chunk");

        assert_eq!(expected, actual);
    }
}
