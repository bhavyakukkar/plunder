use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use anyhow::{anyhow, Context};
use log::trace;
use rustysynth::{SoundFont, Synthesizer, SynthesizerError, SynthesizerSettings};

use libplunder::prelude::instrument::*;
use serde::{Deserialize, Serialize};

pub struct Synth(Synthesizer);

impl Synth {
    pub fn load_sf2<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path.as_ref())?;
        let mut reader = BufReader::new(file);
        Ok(Self(Synthesizer::new(
            &Arc::new(SoundFont::new(&mut reader)?),
            &SynthesizerSettings::new(44100),
        )?))
    }
}

impl Source for Synth {
    type Err = SynthesizerError;
    fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<Self::Err>> {
        let mut left = [0f32];
        let mut right = [0f32];
        self.0.render(&mut left, &mut right);
        trace!("rendered two channels");
        Ok(Some(Sample::F32(vec![left[0], right[0]])))
    }
}

#[rustfmt::skip]
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Key { C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Note {
    pub key: Key,
    pub(crate) octave: usize,
}

impl Note {
    pub fn from_spanned_str(s: &[(usize, char)]) -> Result<Option<Self>, String> {
        // let s = s.chars().collect::<Vec<_>>();
        Ok(Some(match s.len() {
            2 => Note {
                key: match s[0].1 {
                    'C' | 'c' => Key::C,
                    'D' | 'd' => Key::D,
                    'E' | 'e' => Key::E,
                    'F' | 'f' => Key::F,
                    'G' | 'g' => Key::G,
                    'A' | 'a' => Key::A,
                    'B' | 'b' => Key::B,
                    _ => return Err(format!("at {}: invalid key", s[0].0)),
                },
                octave: String::from(s[1].1)
                    .parse()
                    .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
            },
            3 => Note {
                key: match (s[0].1, s[1].1) {
                    ('C', '#') | ('c', '#') | ('D', 'b') | ('d', 'b') => Key::Db,
                    ('D', '#') | ('d', '#') | ('E', 'b') | ('e', 'b') => Key::Eb,
                    ('F', '#') | ('f', '#') | ('G', 'b') | ('g', 'b') => Key::Gb,
                    ('G', '#') | ('g', '#') | ('A', 'b') | ('a', 'b') => Key::Ab,
                    ('A', '#') | ('a', '#') | ('B', 'b') | ('b', 'b') => Key::Bb,

                    ('C', _) | ('c', _) => Key::C,
                    ('D', _) | ('d', _) => Key::D,
                    ('E', _) | ('e', _) => Key::E,
                    ('F', _) | ('f', _) => Key::F,
                    ('G', _) | ('g', _) => Key::G,
                    ('A', _) | ('a', _) => Key::A,
                    ('B', _) | ('b', _) => Key::B,
                    _ => return Err(format!("at {}: invalid key", s[0].0)),
                },
                octave: String::from(s[2].1)
                    .parse()
                    .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
            },
            0 => {
                return Ok(None);
            }
            _ => {
                return Err(format!(
                    "at {}: note string may only be or 2 or 3 characters long",
                    s[0].0
                ));
            }
        }))
    }

    fn number(&self) -> i32 {
        let num_c_o: i32 = match self.octave {
            octave @ 0..=9 => octave * 12,
            _ => unreachable!(),
        }
        .try_into()
        .unwrap();

        match self.key {
            Key::C => num_c_o,
            Key::Db => num_c_o + 1,
            Key::D => num_c_o + 2,
            Key::Eb => num_c_o + 3,
            Key::E => num_c_o + 4,
            Key::F => num_c_o + 5,
            Key::Gb => num_c_o + 6,
            Key::G => num_c_o + 7,
            Key::Ab => num_c_o + 8,
            Key::A => num_c_o + 9,
            Key::Bb => num_c_o + 10,
            Key::B => num_c_o + 11,
        }
    }

    fn _freq(&self) -> f32 {
        const INCR: f32 = 1.0594630943592953;
        let (freq_ao, freq_ao_1) = match self.octave {
            octave @ 0..=9 => (
                27.5 * 2f32.powf(octave as f32),
                27.5 * 2f32.powf(octave as f32 - 1.),
            ),
            _ => unreachable!(),
        };

        match self.key {
            Key::C => freq_ao_1 * INCR.powf(3.),
            Key::Db => freq_ao_1 * INCR.powf(4.),
            Key::D => freq_ao_1 * INCR.powf(5.),
            Key::Eb => freq_ao_1 * INCR.powf(6.),
            Key::E => freq_ao_1 * INCR.powf(7.),
            Key::F => freq_ao_1 * INCR.powf(8.),
            Key::Gb => freq_ao_1 * INCR.powf(9.),
            Key::G => freq_ao_1 * INCR.powf(10.),
            Key::Ab => freq_ao_1 * INCR.powf(11.),
            Key::A => freq_ao,
            Key::Bb => freq_ao * INCR.powf(1.),
            Key::B => freq_ao * INCR.powf(2.),
        }
    }
}

impl State<String, Note> for Synth {
    type TErr = u8;
    type IErr = anyhow::Error;

    fn transform(&mut self, note: Note) -> Result<(), u8> {
        self.0.note_off_all(false);
        // println!("note on:`{}`", note.freq());
        self.0.note_on(1, note.number(), 100);
        Ok(())
    }

    fn initialize(route: &str, arguments: String) -> Result<Self, Self::IErr>
    where
        Self: Sized,
    {
        match route {
            "open" => Self::load_sf2(&arguments).context(format!("while opening `{arguments}`")),
            _ => Err(anyhow!("invalid route `{route}`. available: `open`")),
        }
    }
}

impl Instrument<String, Note> for Synth {
    fn help(&self) -> String {
        "MIDI synthesizer".into()
    }
}

#[cfg(test)]
mod tests {}
