use moonlight::wave::{Wave, Waveable};

fn main() -> anyhow::Result<()> {
    let original: Wave = Wave::new(&[1.0, 1.0, 1.0, 1.0, 1.0], 44100, 1)?;
    let actual: Wave = original.to_silent()?;
    let expected: Wave = Wave::new(&[0.0, 0.0, 0.0, 0.0, 0.0], 44100, 1)?;

    dbg!(&original);
    dbg!(&actual);
    dbg!(&expected);

    assert_eq!(expected, actual);

    Ok(())
}

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
