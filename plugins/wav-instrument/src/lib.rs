use crate::prelude::*;

use hound::WavReader;
use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Clone)]
enum ClipEvent {
    Play,
    Pause,
    Stop,
    Resume,
    Multiple(Vec<ClipEvent>),
}

impl TryFrom<&LuaValue> for ClipEvent {
    type Error = mlua::Error;

    fn try_from(value: &LuaValue) -> Result<Self, mlua::Error> {
        match value {
            LuaValue::Boolean(b) => Ok(if *b {
                ClipEvent::Resume
            } else {
                ClipEvent::Pause
            }),
            LuaValue::Integer(n) => match n {
                1 => Ok(ClipEvent::Play),
                2 => Ok(ClipEvent::Resume),
                3 => Ok(ClipEvent::Pause),
                4 => Ok(ClipEvent::Stop),
                n => Err(mlua::Error::runtime(format!(
                    "unknown clip-event index: {n}"
                ))),
            },
            LuaValue::String(s) => match s.to_string_lossy().as_str() {
                "play" | "Play" | "||>" => Ok(ClipEvent::Play),
                "pause" | "Pause" | "||" => Ok(ClipEvent::Pause),
                "stop" | "Stop" | "|]" | "[|" | "o" => Ok(ClipEvent::Stop),
                "resume" | "Resume" | "|>" => Ok(ClipEvent::Resume),
                s => Err(mlua::Error::runtime(format!("unknown clip-event: {s}"))),
            },
            LuaValue::Table(table) => Ok(ClipEvent::Multiple(
                table
                    .pairs::<LuaValue, LuaValue>()
                    .filter_map(|pair| pair.ok())
                    .map(|(_, value)| ClipEvent::try_from(&value))
                    // collect here is converting an iterator of results to a result of a vector
                    // similar to haskell's sequence
                    // https://doc.rust-lang.org/src/core/result.rs.html#1936
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            //TODO better errors man
            _ => Err(mlua::Error::runtime("bad type")),
        }
    }
}

pub struct Wav {
    reader: WavReader<BufReader<File>>,
    outputting: bool,
}

impl std::fmt::Debug for Wav {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "wav that is {}outputting",
            if self.outputting { "" } else { "not " }
        )
    }
}

impl Wav {
    pub fn load<P>(path: P) -> Result<Self, hound::Error>
    where
        P: AsRef<Path>,
    {
        WavReader::open(path).map(|reader| Self {
            reader,
            outputting: true,
        })
    }

    fn control(&mut self, event: ClipEvent) -> anyhow::Result<()> {
        use anyhow::Context;
        use ClipEvent::*;

        match event {
            Play => {
                self.reader
                    .seek(0)
                    .context("Can't seek to beginning of file")?;
                self.outputting = true;
            }
            Pause => self.outputting = false,
            Stop => {
                self.reader
                    .seek(0)
                    .context("Can't seek to beginning of file")?;
                self.outputting = false;
            }
            Resume => self.outputting = true,
            Multiple(clip_events) => {
                for clip_event in clip_events {
                    self.control(clip_event)?;
                }
            }
        }
        Ok(())
    }
}

impl<S> Source<S> for Wav
where
    S: SampleDepth,
{
    fn next(&mut self) -> Option<Result<S, anyhow::Error>> {
        if self.outputting {
            Some(self.reader.samples().next()?.map_err(|err| err.into()))
        } else {
            None
        }
    }
}

impl BasicInstrument for Wav {
    fn transition(&mut self, event: mlua::Value, _: &Lua) -> mlua::Result<()> {
        self.control(ClipEvent::try_from(&event)?)
            .map_err(mlua::Error::runtime)
    }

    fn help(&mut self) -> String {
        todo!()
    }
}
