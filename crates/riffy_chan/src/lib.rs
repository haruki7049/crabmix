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
        todo!()
    }

    fn create_riff_chunk(value: Vec<u8>) -> Result<Self, ChunkTryFromError> {
        todo!()
    }
}

#[derive(Debug, Error)]
pub enum ChunkTryFromError {
    #[error("There is not a FourCC data")]
    NoFourCC,
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
