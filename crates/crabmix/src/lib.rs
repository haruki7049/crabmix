//! # crabmix
//!
//! `crabmix` is an audio processing library for reading, writing, and
//! manipulating audio waveforms in Rust.
//!
//! The main entry point is the [`wave`] module, which provides the [`wave::Wave`]
//! type for working with audio sample data, along with traits and helpers for
//! mixing, splitting, and serialising audio in various file formats (currently
//! WAV via the [`rustttwavvv`](rustttwavvv) crate).
//!
//! ## Quick start
//!
//! ```rust
//! use crabmix::wave::{Wave, Waveable};
//!
//! // Create a mono waveform at 44100 Hz.
//! let wave = Wave::new(&[0.0, 0.5, -0.5, 1.0], 44100, 1).unwrap();
//! assert_eq!(wave.sample_rate(), 44100);
//! assert_eq!(wave.channels(), 1);
//! ```

pub mod wave;
