use std::path::Path;

pub fn export_wav<P>(path: P, samples: &[f32])
where
    P: AsRef<Path>,
{
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path.as_ref(), spec).unwrap();
    let mut index = 0;
    while let Some(sample) = samples.get(index) {
        writer.write_sample(*sample * 2.).unwrap();
        index += 1;
    }
    writer.finalize().unwrap();
}
