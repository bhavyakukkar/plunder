#![warn(missing_debug_implementations)]

use std::{
    fmt,
    hash::Hash,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use instrument::{EmittableUserData, PackagedInstrument, SourceError};

pub mod instrument;

pub mod prelude {
    pub mod instrument {
        pub use crate::{
            instrument::{
                package_instrument, /* Emittable, */ EmittableUserData, Instrument,
                PackagedInstrument, Source, SourceError, State,
            },
            is_event, Sample,
        };
    }

    pub mod parser {}
}

// mod wav_instrument;

// pub struct Sample<const S: usize>([u8; S]);

#[derive(Debug, Clone)]
pub enum Sample {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U24(Vec<u32>),
    U32(Vec<u32>),
    S8(Vec<i8>),
    S16(Vec<i16>),
    S24(Vec<i32>),
    S32(Vec<i32>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    Empty,
}

fn f64_to_i32(input: f64) -> i32 {
    let f64_bits = input.to_bits();
    let mut i32_bits: u32 = 0;
    for i in 0..32 {
        let odd_bit = (f64_bits >> (2 * i + 1)) & 1;
        i32_bits |= (odd_bit as u32) << i;
    }
    (i32::MIN as i64 + i32_bits as i64) as i32
}

// fn f32_to_i32(input: f32) -> i32 {
//     // Extract the bits of the f64
//     let f32_bits = input.to_bits();

//     // Collect only the odd bits from the f64
//     let mut i32_bits: u32 = 0;
//     for i in 0..32 {
//         // Shift the f64 bits right by (2 * i + 1) to get the odd bit, then mask it
//         let odd_bit = (f32_bits >> (2 * i + 1)) & 1;
//         // Place the odd bit into the corresponding position in the f32 bits
//         i32_bits |= (odd_bit as u32) << i;
//     }

//     // Convert the u32 to f32
//     (i32::MIN as i64 + i32_bits as i64) as i32
// }

// TODO allow samples of differing number of channels
pub fn combine_i32(samples: &[Sample]) -> anyhow::Result<Option<Vec<i32>>> {
    fn set_vec_i32<T, F>(
        sum: &mut Option<Vec<i32>>,
        samples: &Vec<T>,
        mut f: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(&T) -> i32,
    {
        match sum {
            Some(sum) => {
                samples
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        println!("adding {} to channel {i}", f(c));
                        sum.get_mut(i)
                            .map(|si| *si = si.saturating_add(f(c)))
                            .ok_or(anyhow!("channel inconsistency"))
                    })
                    .collect::<Result<(), _>>()?;
            }
            None => {
                *sum = Some(
                    samples
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            println!("adding {} to channel {i}", f(c));
                            f(c)
                        })
                        .collect(),
                );
            }
        }
        Ok(())
    }

    if samples.len() == 0 {
        return Ok(None);
    }
    let mut sum = None;
    for sample in samples {
        match sample {
            // (c / u8::MAX) * u32::MAX
            Sample::U8(cs) => set_vec_i32(&mut sum, cs, |c| {
                (i64::MIN + ((*c as i64) * (u32::MAX as i64 / u8::MAX as i64))) as i32
            }),
            Sample::U16(cs) => set_vec_i32(&mut sum, cs, |c| {
                (i64::MIN + ((*c as i64) * (u32::MAX as i64 / u16::MAX as i64))) as i32
            }),
            Sample::U24(cs) => set_vec_i32(&mut sum, cs, |c| {
                (i64::MIN + ((*c as i64) * (u32::MAX as i64 / (3 * u8::MAX as i64)))) as i32
            }),
            Sample::U32(cs) => set_vec_i32(&mut sum, cs, |c| (i64::MIN + *c as i64) as i32),
            Sample::S8(cs) => set_vec_i32(&mut sum, cs, |c| {
                (*c as i32) * (u32::MAX / u8::MAX as u32) as i32
            }),
            Sample::S16(cs) => set_vec_i32(&mut sum, cs, |c| {
                (*c as i32) * (u32::MAX / u16::MAX as u32) as i32
            }),
            Sample::S24(cs) => set_vec_i32(&mut sum, cs, |c| {
                (*c as i32) * (u32::MAX / (3 * u8::MAX as u32)) as i32
            }),
            Sample::S32(cs) => set_vec_i32(&mut sum, cs, |c| *c),
            Sample::F32(cs) => {
                set_vec_i32(&mut sum, cs, |c| (i64::MIN + c.to_bits() as i64) as i32)
            }
            Sample::F64(cs) => set_vec_i32(&mut sum, cs, |c| f64_to_i32(*c)),
            Sample::Empty => Ok(()),
        }?;
    }
    Ok(sum)
}

// impl Add<Sample> for Sample {
//     type Output = Sample;

//     fn add(self, rhs: Sample) -> Self::Output {
//         match (self, rhs) {
//             (Sample::U8(items), Sample::U8(_items)) => todo!(),
//             (Sample::U16(items), Sample::U8(_items)) => todo!(),
//             (Sample::U24(items), Sample::U8(_items)) => todo!(),
//             (Sample::U32(items), Sample::U8(_items)) => todo!(),
//             (Sample::S8(items), Sample::U8(_items)) => todo!(),
//             (Sample::S16(items), Sample::U8(_items)) => todo!(),
//             (Sample::S24(items), Sample::U8(_items)) => todo!(),
//             (Sample::S32(items), Sample::U8(_items)) => todo!(),
//             (Sample::F32(items), Sample::U8(_items)) => todo!(),
//             (Sample::F64(items), Sample::U8(_items)) => todo!(),
//             (Sample::Empty, Sample::U8(items)) => todo!(),
//         }
//     }
// }

impl Hash for Sample {
    #[rustfmt::skip]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Sample::U8(items) =>  { 0u8.hash(state); items.hash(state) }
            Sample::U16(items) => { 1u8.hash(state); items.hash(state) }
            Sample::U24(items) => { 2u8.hash(state); items.hash(state) }
            Sample::U32(items) => { 3u8.hash(state); items.hash(state) }
            Sample::S8(items) =>  { 4u8.hash(state); items.hash(state) }
            Sample::S16(items) => { 5u8.hash(state); items.hash(state) }
            Sample::S24(items) => { 6u8.hash(state); items.hash(state) }
            Sample::S32(items) => { 7u8.hash(state); items.hash(state) }
            Sample::F32(items) => {
                8u8.hash(state);
                for item in items {
                    item.to_bits().hash(state);
                }
            }
            Sample::F64(items) => {
                9u8.hash(state);
                for item in items {
                    item.to_bits().hash(state);
                }
            }
            Sample::Empty => { 10u8.hash(state); }
        }
    }
}

// impl Display for Sample {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "{}",
//             match self {
//                 Sample::U8(items) => "U",
//                 Sample::U16(items) => "",
//                 Sample::U24(items) => "",
//                 Sample::U32(items) => "",
//                 Sample::S8(items) => "",
//                 Sample::S16(items) => "",
//                 Sample::S24(items) => "",
//                 Sample::S32(items) => "",
//                 Sample::F32(items) => "",
//                 Sample::F64(items) => "",
//                 Sample::Empty => "",
//             }
//         )
//     }
// }

// pub trait SampleDepth {}
// impl<S: hound::Sample> SampleDepth for S {}
// impl<S: symphonia::core::sample::Sample> SampleDepth for S {}

// pub enum SampleDepth {
//     B8(i8),
//     B16(i16),
//     B24(i8, i8, i8),
//     B32(i32),
// }

type SharedPtr<T> = Arc<RwLock<T>>;

#[derive(Debug)]
pub struct Engine<I> {
    instruments: Vec<PackagedInstrument>,
    event_stream: I,
    next_event: Option<(usize, EmittableUserData)>,
    index: usize,
    interval: usize,
    max_interval: usize,
    duration: usize,
}

impl<I> Engine<I>
where
    I: Iterator<Item = mlua::Result<(usize, EmittableUserData)>>,
{
    fn _next_write(&mut self, _sample: &mut Sample) -> Result<(), EngineError> {
        todo!()
    }

    // TODO over here
    // this iterator can "end" in 3 different ways:
    //   1. event-stream runs out
    //   2. instruments run out
    //   3. duration is finished
    // in the first 2 cases, sometimes we don't even care if it ends: we might still want sample to
    // play till its end, or for other instruments to play for the rest of the duration
    // find a better way to address this
    //
    /// Lots of operations in an iteration can cause errors, so it's idiomatic to return a
    /// Return<Option<_>> and just transpose it in the Iterator implementation
    /// It is the job of the caller to stop iterating when empty samples are being returned. This only returns None once all instruments have been exhausted
    fn next_inner(&mut self) -> Result<Option<Vec<Sample>>, EngineError> {
        if (self.index as u128) * (self.max_interval as u128) >= self.duration as u128 {
            return Ok(None);
        }
        if self.interval >= self.max_interval - 1 {
            // Emit all events set to the current index and then proceed generating samples
            loop {
                match self.next_event {
                    Some(ref mut next_event) => {
                        if next_event.0 == self.index {
                            // We've reached the next unit where an event is to be emitted
                            //
                            // There might be more events at the same unit so don't advance to the
                            // next unit yet
                            println!(">> Reached next-event at i:`{}`", self.index);
                            next_event
                                .1
                                 .0
                                .write()
                                .unwrap() // TODO don't unwrap
                                .emit()
                                .map_err(EngineError::Emit)?;
                            // Empty `next_event` so another event can be popped from the
                            // `event_stream`
                            self.next_event = None;
                        } else {
                            // Unit of `next_event` still not reached, advance 1 unit and try again
                            break;
                        }
                    }
                    None => {
                        // Pop the next event in `event_stream` into `next_event`
                        self.next_event = self
                            .event_stream
                            .next()
                            .transpose()
                            .map_err(EngineError::EventStream)?;
                        if self.next_event.is_none() {
                            // `event_stream` returned None, i.e. it has been exhausted
                            // println!(">> Event-stream exhausted");
                            break;
                        }
                        println!(
                            ">> Popped next next-event with i:`{}`",
                            self.next_event.as_ref().unwrap().0
                        );
                    }
                }
            }
            self.interval = 0;
            // Advance to the next unit
            self.index += 1;
        } else {
            self.interval += 1;
        }

        println!(">> At index {}", self.index);
        let mut samples = None;
        for instrument in &self.instruments {
            let sample = instrument
                .factory
                .write()
                .unwrap() // TODO don't unwrap here
                .next_sample()
                .map_err(EngineError::Source)?;

            samples = match (samples, sample) {
                // only finished instruments so far and one more encountered
                //
                // the only way for the accumulator to result in None is if all instruments
                // return None
                (None, None) => None,
                // first unfinished instrument encountered
                (None, Some(s)) => Some(vec![s]),
                // have some unfinished instruments so ignore an empty one encountered
                (Some(acc), None) => Some(acc),
                // non-first unfinished instrument encountered
                (Some(mut acc), Some(s)) => {
                    acc.push(s);
                    Some(acc)
                }
            };
        }

        Ok(samples)
    }
}

#[derive(Debug)]
pub enum EngineError {
    EventStream(mlua::Error),
    Emit(String),
    Source(SourceError<String>),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "engine error")
    }
}

impl<I> Iterator for Engine<I>
where
    I: Iterator<Item = mlua::Result<(usize, EmittableUserData)>>,
{
    type Item = Result<Vec<Sample>, EngineError>;

    fn next(&mut self) -> Option<Result<Vec<Sample>, EngineError>> {
        self.next_inner().transpose()
    }
}

impl<I> Engine<I>
where
    I: Iterator<Item = mlua::Result<(usize, EmittableUserData)>>,
{
    pub fn new(
        instruments: Vec<PackagedInstrument>,
        event_stream: I,
        max_interval: usize,
        sample_bound: usize,
    ) -> Self {
        println!("max-interval:`{max_interval}`");
        Engine {
            instruments,
            event_stream,
            next_event: None,
            index: 0,
            interval: max_interval,
            max_interval,
            duration: sample_bound,
        }
    }
}

pub fn is_event(value: &mlua::Value) -> bool {
    // TODO checks whether a LuaValue is an event *by convention* instead of just checking that its
    // userdata
    matches!(value, mlua::Value::UserData(_))
}

mod encoder {
    // use crate::Sample;
    // use hound::SampleFormat;
    // use std::io::{Seek, Write};

    // use hound::Sample;

    // fn encode(samples: &[Vec<Sample>]) {
    //     let spec = hound::WavSpec {
    //         channels: samples[0].len() as u16,
    //         sample_rate: 44100,
    //         bits_per_sample: 16,
    //         sample_format: hound::SampleFormat::Int,
    //     };
    //     let mut writer = hound::WavWriter::create("sine.wav", spec).unwrap();
    //     samples
    //         .iter()
    //         .map(|sample| writer.write_sample(sample).unwrap());
    // }

    // impl hound::Sample for Sample {
    //     fn write<W: std::io::Write>(self, writer: &mut W, bits: u16) -> hound::Result<()> {
    //         match self {
    //             Sample::U8(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::U16(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i16).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::U24(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::U32(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::S8(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::S16(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::S24(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::S32(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::F32(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::F64(samples) => samples
    //                 .iter()
    //                 .map(|sample| (*sample as i8).write(writer, bits))
    //                 .collect::<hound::Result<_>>(),
    //             Sample::Empty => {
    //                 todo!();
    //                 Ok(())
    //             }
    //         }
    //     }

    //     fn write_padded<W: std::io::Write>(
    //         self,
    //         writer: &mut W,
    //         bits: u16,
    //         byte_width: u16,
    //     ) -> hound::Result<()> {
    //         todo!()
    //     }

    //     fn read<R: std::io::Read>(
    //         reader: &mut R,
    //         fmt: SampleFormat,
    //         bytes: u16,
    //         bits: u16,
    //     ) -> hound::Result<Self> {
    //         todo!()
    //     }

    //     fn as_i16(self) -> i16 {
    //         todo!()
    //     }
    // }

    /* trait Encoder<S: Sample, W: Write + Seek> {
        fn write_samples(&mut self, writer: W, samples: &[S]);
    }

    struct WavEncoder;

    impl<S: Sample, W: Write + Seek> Encoder<S, W> for WavEncoder {
        fn write_samples(&mut self, writer: W, samples: &[S]) {
            // let wav_writer()
        }
    } */
}
