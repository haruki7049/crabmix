use crabmix::wave::{FileFormat, Wave, WaveWriteOptions, Waveable};
use std::{f64::consts::PI, fs::File};

fn main() -> anyhow::Result<()> {
    let sinewave: Wave = generate_sinewave()?;
    let mut file = File::create("result-sinewave.wav")?;
    let file_format: FileFormat = FileFormat::wav(rustttwavvv::FormatCode::PCM, 16);
    let write_options: WaveWriteOptions = WaveWriteOptions::new(file_format);
    sinewave.write(&mut file, write_options)?;

    Ok(())
}

const SINEWAVE_SAMPLES_LEN: usize = 88200;
const SINEWAVE_SAMPLE_RATE: u32 = 44100;
const SINEWAVE_CHANNELS: u16 = 1;
const SINEWAVE_FREQUENCY: f64 = 440.0;
const RADIANS_PER_SEC: f64 = SINEWAVE_FREQUENCY * 2.0 * PI;

fn generate_sinewave() -> anyhow::Result<Wave> {
    let sinewave_samples = generate_sinewave_samples()?;

    let result: Wave = Wave::new(&sinewave_samples, SINEWAVE_SAMPLE_RATE, SINEWAVE_CHANNELS)?;
    Ok(result)
}

fn generate_sinewave_samples() -> anyhow::Result<Vec<f64>> {
    let mut samples: Vec<f64> = Vec::new();
    for i in 0..SINEWAVE_SAMPLES_LEN {
        let t = i as f64 / SINEWAVE_SAMPLE_RATE as f64;
        let value = (RADIANS_PER_SEC * t).sin();

        samples.push(value);
    }

    Ok(samples)
}
