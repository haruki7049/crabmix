//! # riffy_chan

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FourCC {
    inner: [u8; 4],
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

#[cfg(test)]
mod tests {
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
