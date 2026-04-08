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

impl From<&[u8; 4]> for FourCC {
    fn from(from: &[u8; 4]) -> Self {
        Self { inner: *from }
    }
}

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

#[derive(Debug, Error)]
pub enum FourCCTryFromError {
    #[error("Invalid length of Vec<u8>, expected 4 but actually {}", actual)]
    InvalidLength { actual: usize },

    #[error("Invalid slice. The received Vec<u8> is: {:?}", actual)]
    InvalidSlice { actual: Vec<u8> },
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

impl Chunk {
    fn size(&self) -> usize {
        match self {
            Self::Chunk { data, .. } => Self::chunk_size(data),
            Self::Riff { chunks, .. } => Self::riff_chunk_size(chunks),
            Self::List { chunks } => Self::list_chunk_size(chunks),
        }
    }

    fn chunk_size(data: &[u8]) -> usize {
        const FOUR_CC_BYTES: usize = 4;
        const SIZE_BYTES: usize = 4;
        let data_bytes: usize = data.len();

        data_bytes + FOUR_CC_BYTES + SIZE_BYTES
    }

    fn riff_chunk_size(chunks: &[Chunk]) -> usize {
        const RIFF_BYTES: usize = 4;
        const FOUR_CC_BYTES: usize = 4;
        const SIZE_BYTES: usize = 4;
        let chunks_bytes = chunks.len();

        chunks_bytes + RIFF_BYTES + FOUR_CC_BYTES + SIZE_BYTES
    }

    fn list_chunk_size(chunks: &[Chunk]) -> usize {
        const LIST_BYTES: usize = 4;
        const FOUR_CC_BYTES: usize = 4;
        const SIZE_BYTES: usize = 4;
        let chunks_bytes = chunks.iter().map(Self::size).count();

        chunks_bytes + LIST_BYTES + FOUR_CC_BYTES + SIZE_BYTES
    }
}

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

impl Chunk {
    fn parse_chunk(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let four_cc_raw = buffer[0..4].to_vec();
        let four_cc = FourCC::try_from(four_cc_raw)?;

        let size = u32::from_le_bytes(buffer[4..8].try_into()?) as usize;
        let data = buffer[8..8 + size].to_vec();

        if size < 8 {
            return Err(Box::new(ChunkTryFromError::SizeMismatch));
        }

        Ok(Chunk::Chunk { four_cc, data })
    }

    fn parse_list(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let size = u32::from_le_bytes(buffer[4..8].try_into()?) as usize;
        dbg!("List size: {}", size);
        let mut chunks = Vec::new();
        let mut offset = 8;

        while offset < size {
            dbg!("Current offset: {}", offset);

            let chunk = Chunk::parse_chunk(&buffer[offset..])?;
            let chunk_size = Chunk::size(&chunk);
            dbg!("Parsed chunk size: {}", chunk_size);

            chunks.push(chunk);
            offset += chunk_size;
            dbg!("New offset: {}", offset);
        }

        Ok(Chunk::List { chunks })
    }

    fn parse_riff(buffer: &[u8]) -> Result<Chunk, Box<dyn std::error::Error>> {
        let size = u32::from_le_bytes(buffer[4..8].try_into()?) as usize;
        let mut chunks = Vec::new();

        let four_cc_raw = buffer[8..12].to_vec();
        let four_cc = FourCC::try_from(four_cc_raw)?;
        let mut offset = 0;

        while offset < size {
            let chunk = Chunk::parse_chunk(&buffer[offset..])?;
            let chunk_size = Chunk::size(&chunk);
            chunks.push(chunk);
            offset += chunk_size;
        }

        Ok(Chunk::Riff { four_cc, chunks })
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

    // #[test]
    // fn load_riff_chunk() -> Result<(), Box<dyn std::error::Error>> {
    //     {
    //         let expected = b"RIFF\x14\x00\x00\x00TESTfmt \x00\x00\x00\x00data\x00\x00\x00\x00";
    //         let bytes = include_bytes!("./assets/riff_chunk.riff");
    //         assert_eq!(bytes, expected);
    //     }

    //     {
    //         let bytes = include_bytes!("./assets/riff_chunk.riff");
    //         let expected = Chunk::Chunk {
    //             four_cc: FourCC::from(*b"fmt "),
    //             data: b"EXAMPLE_DATA".to_vec(),
    //         };
    //         let actual = Chunk::try_from(bytes.to_vec())?;
    //         assert_eq!(expected, actual);
    //     }

    //     Ok(())
    // }
}
