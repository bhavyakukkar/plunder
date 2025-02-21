use std::hash::DefaultHasher;

use anyhow::Context;
use log::{info, trace};
use mlua::prelude::*;

use libplunder::{combine_i32, prelude::instrument::*, Engine};

pub type EventStreamPair = (usize, EmittableUserData);

pub fn render_single_event_stream<I>(
    path: String,
    instruments: Vec<LuaUserDataRef<PackagedInstrument>>,
    sorted_event_stream: I,
    bitrate: u32,
    interval: usize,
    sample_bound: usize,
) where
    I: Iterator<Item = EventStreamPair>,
{
    use std::hash::{Hash, Hasher};

    let engine = Engine::new(
        instruments
            .iter()
            .map(|instrument| (*instrument).clone())
            .collect(),
        sorted_event_stream,
        interval,
        sample_bound,
    );

    let mut hasher = DefaultHasher::new();
    let mut samples = engine.map(|i| match i {
        Ok(i) => match combine_i32(&i) {
            Ok(Some(s)) => {
                // for si in &s {
                //     print!("{si} ");
                // }
                // println!();
                trace!("combined samples into `{s:?}`");
                s.hash(&mut hasher);
                Some(Ok(s))
            }
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        },
        Err(err) => Some(Err(anyhow::anyhow!("engine error: {err}"))),
    });

    let first_sample = samples
        .next()
        .unwrap()
        // We unwrap the Option that combine_i32 returns because we will have no information about
        // the number of channels if the first sample is empty
        .unwrap()
        .unwrap();

    let num_channels = first_sample.len();
    println!("num of channels: `{num_channels}`");
    let spec = hound::WavSpec {
        channels: num_channels as u16,
        sample_rate: bitrate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    first_sample
        .iter()
        .map(|s| writer.write_sample(*s))
        .collect::<Result<(), _>>()
        .unwrap();
    samples
        .map(|s| {
            s.transpose()?
                // because we received the number of channels from that first-sample, we can replace future empty samples with a collection of empty samples in each channel
                .unwrap_or(vec![0; num_channels])
                .iter()
                .map(|s| writer.write_sample(*s))
                .collect::<Result<(), _>>()
                .context("wav write error")
        })
        .collect::<Result<(), _>>()
        .unwrap();

    writer.finalize().unwrap();

    info!("hash: {}", hasher.finish());
}
