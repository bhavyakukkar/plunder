use std::{
    collections::VecDeque,
    fs::File,
    io::{self},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use log::{info, trace};
use serde::Deserialize;
use symphonia::{
    core::{
        audio::{AudioBuffer, AudioBufferRef, Signal},
        codecs::Decoder,
        errors::Error as SymphoniaError,
        formats::{FormatReader, SeekMode, SeekTo},
        io::{MediaSource, MediaSourceStream},
        probe::Hint,
        sample::Sample as SymphoniaSample,
    },
    default::get_codecs,
};

use libplunder::prelude::instrument::*;

const ERR_PREFIX: &str = "<|SAMPLER|>::Err   |";
const _WARN_PREFIX: &str = "<|SAMPLER|>::Warn |";
const MANUAL: &str = "<|SAMPLER|>";

struct ToSample<'a>(AudioBufferRef<'a>);

type Channels<T> = Vec<T>;
fn yo<T: SymphoniaSample>(abr: &AudioBuffer<T>) -> Vec<Channels<T>> {
    let ns = abr.frames();
    let nc = abr.spec().channels.count();
    let mut out = Vec::with_capacity(ns * nc * size_of::<T>());
    let mut cur: &mut Vec<T>;
    for is in 0..ns {
        out.push(Vec::with_capacity(nc * size_of::<T>()));
        cur = &mut out[is];
        for ic in 0..nc {
            cur.push(abr.chan(ic)[is]);
        }
    }
    out
}

impl From<ToSample<'_>> for Vec<Sample> {
    fn from(ToSample(abr): ToSample) -> Self {
        use AudioBufferRef as ABR;

        match abr {
            ABR::U8(buf) => yo(&buf).into_iter().map(Sample::U8).collect(),
            ABR::U16(buf) => yo(&buf).into_iter().map(Sample::U16).collect(),
            ABR::U24(buf) => yo(&buf)
                .into_iter()
                .map(|ss| Sample::U24(ss.iter().map(|s| s.0).collect()))
                .collect(),
            ABR::U32(buf) => yo(&buf).into_iter().map(Sample::U32).collect(),
            ABR::S8(buf) => yo(&buf).into_iter().map(Sample::S8).collect(),
            ABR::S16(buf) => yo(&buf).into_iter().map(Sample::S16).collect(),
            ABR::S24(buf) => yo(&buf)
                .into_iter()
                .map(|ss| Sample::S24(ss.iter().map(|s| s.0).collect()))
                .collect(),
            ABR::S32(buf) => yo(&buf).into_iter().map(Sample::S32).collect(),
            ABR::F32(buf) => yo(&buf).into_iter().map(Sample::F32).collect(),
            ABR::F64(buf) => yo(&buf).into_iter().map(Sample::F64).collect(),
        }
    }
}

enum Reader {
    File {
        reader: Box<dyn FormatReader>,
        decoder: Box<dyn Decoder>,
        buffer: VecDeque<Sample>,
    },
    Mem {
        samples: Box<[Sample]>,
        cursor: usize,
    },
}

pub struct Sampler {
    outputting: bool,
    mute: bool,
    backward: bool,
    reader: Reader,
    path: PathBuf,
}

impl Sampler {
    pub fn load<P>(path: P, read_entire: bool) -> Result<Self, anyhow::Error>
    where
        P: AsRef<Path>,
    {
        let mut hint = Hint::new();
        if let Some(extension) = path.as_ref().extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        let boxed_source: Box<dyn MediaSource> = Box::new(File::open(&path)?);
        let mss = MediaSourceStream::new(boxed_source, Default::default());
        let mut probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &Default::default(),
            &Default::default(),
        )?;
        let track = probed.format.default_track().unwrap();
        let mut decoder = get_codecs().make(&track.codec_params, &Default::default())?;

        // dont care about errors in printing info
        _ = debug::print_tracks(probed.format.tracks());

        if read_entire {
            let mut samples = Vec::new();
            loop {
                match probed.format.next_packet() {
                    Ok(packet) => match decoder.decode(&packet) {
                        Ok(abr) => samples.extend(Vec::from(ToSample(abr))),
                        Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => {
                            samples.push(Sample::Empty)
                        }
                        // TODO reset, whatever that means
                        Err(SymphoniaError::ResetRequired) => (),
                        Err(err) => return Err(anyhow!("error decoding packet: {}", err)),
                    },
                    // TODO reset, whatever that means
                    Err(SymphoniaError::ResetRequired) => (),
                    Err(SymphoniaError::IoError(err))
                        if err.kind() == io::ErrorKind::UnexpectedEof =>
                    {
                        break;
                    }
                    Err(err) => return Err(anyhow!("error getting next packet: {}", err)),
                }
            }
            Ok(Sampler {
                reader: Reader::Mem {
                    samples: Box::from(samples.as_slice()),
                    cursor: 0,
                },
                outputting: false,
                mute: false,
                backward: false,
                path: path.as_ref().to_path_buf(),
            })
        } else {
            Ok(Sampler {
                reader: Reader::File {
                    decoder,
                    reader: probed.format,
                    buffer: VecDeque::new(),
                },
                outputting: false,
                mute: false,
                backward: false,
                path: path.as_ref().to_path_buf(),
            })
        }
    }

    pub fn next(&mut self) -> Result<Option<Sample>, SourceError<anyhow::Error>> {
        use SourceError::*;

        match &mut self.reader {
            Reader::File {
                reader,
                decoder,
                ref mut buffer,
            } => {
                if !self.outputting {
                    return Ok(Some(Sample::Empty));
                }

                if buffer.is_empty() {
                    // Re-fill buffer with next packet
                    let packet = match reader.next_packet() {
                        Ok(packet) => packet,
                        Err(SymphoniaError::IoError(err))
                            if err.kind() == io::ErrorKind::UnexpectedEof =>
                        {
                            return Ok(None);
                        }
                        // TODO reset, whatever that means
                        Err(SymphoniaError::ResetRequired) => todo!(),
                        Err(err) => {
                            return Err(Fatal(anyhow!("error getting next packet: {}", err)));
                        }
                    };

                    let decoded = match decoder.decode(&packet) {
                        Ok(decoded) => decoded,
                        // TODO reset, whatever that means
                        Err(SymphoniaError::ResetRequired) => todo!(),
                        Err(SymphoniaError::DecodeError(_) | SymphoniaError::IoError(_)) => {
                            return Err(Once(anyhow!("undecodeable packet discard")));
                        }
                        Err(err) => {
                            return Err(Fatal(anyhow!("error decoding packet: {}", err)));
                        }
                    };

                    *buffer = Vec::from(ToSample(decoded)).into();

                    if buffer.len() == 0 {
                        return Err(Fatal(anyhow!("Packet has 0 samples")));
                    }
                }

                Ok(Some(if !self.mute {
                    buffer.pop_front().expect("cannot be empty, just checked")
                } else {
                    Sample::Empty
                }))
            }

            Reader::Mem {
                samples,
                ref mut cursor,
            } => {
                if !self.outputting {
                    return Ok(Some(Sample::Empty));
                }

                if !self.backward {
                    trace!("!! cursor at {cursor}, sample len is {}", samples.len());
                    if *cursor >= samples.len() {
                        return Ok(None);
                    }
                    *cursor += 1;
                    trace!("Outputting samples: {:?}", samples[*cursor - 1]);
                    Ok(Some(if !self.mute {
                        samples[*cursor - 1].clone()
                    } else {
                        Sample::Empty
                    }))
                } else {
                    if *cursor == 0 {
                        return Ok(None);
                    }
                    *cursor -= 1;
                    Ok(Some(if !self.mute {
                        samples[*cursor].clone()
                    } else {
                        Sample::Empty
                    }))
                }
            }
        }
    }

    pub fn control(&mut self, event: AudioControls) -> Result<(), anyhow::Error> {
        match event {
            AudioControls::Seek(duration) => match &mut self.reader {
                Reader::File { reader, .. } => {
                    let duration = duration_str::parse(duration)
                        .map_err(|err| anyhow!("error parsing duration: {err}"))?;
                    let _ = reader.seek(
                        SeekMode::Accurate,
                        SeekTo::Time {
                            time: duration.into(),
                            track_id: None,
                        },
                    )?;
                }
                Reader::Mem {
                    samples: _samples,
                    cursor: _cursor,
                } => todo!(),
            },
            AudioControls::Pause => {
                info!("Pausing sample");
                self.outputting = false;
            }
            AudioControls::Resume => {
                info!("Resuming sample");
                self.outputting = true;
            }
            AudioControls::Reverse => {
                if let Reader::File { .. } = self.reader {
                    return Err(anyhow!(
                        "cannot reverse an opened file, import it to bring it entirely in \
                            memory so that it can be reversed"
                    ));
                }
                self.backward = !self.backward;
            }
            AudioControls::Mute => todo!(),
            AudioControls::Unmute => todo!(),
        }
        Ok(())
    }

    pub fn package(lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        package_instrument::<Sampler, String, AudioControls>(lua, MANUAL.to_string())
    }
}

mod debug {
    use std::fmt;

    use log::info;
    use symphonia::core::{formats::Track, units::TimeBase};

    fn fmt_time(ts: u64, tb: TimeBase) -> String {
        let time = tb.calc_time(ts);

        let hours = time.seconds / (60 * 60);
        let mins = (time.seconds % (60 * 60)) / 60;
        let secs = f64::from((time.seconds % 60) as u32) + time.frac;

        format!("{}:{:0>2}:{:0>6.3}", hours, mins, secs)
    }

    pub fn print_tracks(tracks: &[Track]) -> fmt::Result {
        use fmt::Write;
        let mut banner = String::new();

        if !tracks.is_empty() {
            writeln!(banner, "\n|")?;
            writeln!(banner, "| // Tracks //")?;

            for (idx, track) in tracks.iter().enumerate() {
                let params = &track.codec_params;

                write!(banner, "|     [{:0>2}] Codec:           ", idx + 1)?;

                if let Some(codec) = symphonia::default::get_codecs().get_codec(params.codec) {
                    writeln!(banner, "{} ({})", codec.long_name, codec.short_name)?;
                } else {
                    writeln!(banner, "Unknown (#{})", params.codec)?;
                }

                if let Some(sample_rate) = params.sample_rate {
                    writeln!(banner, "|          Sample Rate:     {}", sample_rate)?;
                }
                if params.start_ts > 0 {
                    if let Some(tb) = params.time_base {
                        writeln!(
                            banner,
                            "|          Start Time:      {} ({})",
                            fmt_time(params.start_ts, tb),
                            params.start_ts
                        )?;
                    } else {
                        writeln!(banner, "|          Start Time:      {}", params.start_ts)?;
                    }
                }
                if let Some(n_frames) = params.n_frames {
                    if let Some(tb) = params.time_base {
                        writeln!(
                            banner,
                            "|          Duration:        {} ({})",
                            fmt_time(n_frames, tb),
                            n_frames
                        )?;
                    } else {
                        writeln!(banner, "|          Frames:          {}", n_frames)?;
                    }
                }
                if let Some(tb) = params.time_base {
                    writeln!(banner, "|          Time Base:       {}", tb)?;
                }
                if let Some(padding) = params.delay {
                    writeln!(banner, "|          Encoder Delay:   {}", padding)?;
                }
                if let Some(padding) = params.padding {
                    writeln!(banner, "|          Encoder Padding: {}", padding)?;
                }
                if let Some(sample_format) = params.sample_format {
                    writeln!(banner, "|          Sample Format:   {:?}", sample_format)?;
                }
                if let Some(bits_per_sample) = params.bits_per_sample {
                    writeln!(banner, "|          Bits per Sample: {}", bits_per_sample)?;
                }
                if let Some(channels) = params.channels {
                    writeln!(banner, "|          Channel(s):      {}", channels.count())?;
                    writeln!(banner, "|          Channel Map:     {}", channels)?;
                }
                if let Some(channel_layout) = params.channel_layout {
                    writeln!(banner, "|          Channel Layout:  {:?}", channel_layout)?;
                }
                if let Some(language) = &track.language {
                    writeln!(banner, "|          Language:        {}", language)?;
                }
            }
        }
        info!("{}", banner);
        Ok(())
    }
}

impl Source for Sampler {
    type Err = anyhow::Error;

    fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<anyhow::Error>> {
        self.next()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioControls {
    Seek(String),
    Pause,
    Resume,
    Reverse,
    Mute,
    Unmute,
}

impl State<String, AudioControls> for Sampler {
    type TErr = anyhow::Error;
    type IErr = anyhow::Error;

    fn transform(&mut self, event: AudioControls) -> Result<(), anyhow::Error> {
        self.control(event)
    }

    fn initialize(route: &str, arguments: String) -> Result<Self, anyhow::Error> {
        match route {
            "open" => Self::load(&arguments, false)
                .context(format!("{ERR_PREFIX} while opening `{arguments}`")),
            "import" => Self::load(&arguments, true)
                .context(format!("{ERR_PREFIX} while importing `{arguments}`")),
            _ => Err(anyhow!(
                "{ERR_PREFIX} invalid route `{route}`. available: `open` and `import`"
            )),
        }
    }
}

impl Instrument<String, AudioControls> for Sampler {
    fn help(&self) -> String {
        format!(
            "<|SAMPLER|> An instrument for Plunder that can read & manipulate digital audio\n\
            This sampler contains `{}`",
            self.path.display()
        )
    }
}
