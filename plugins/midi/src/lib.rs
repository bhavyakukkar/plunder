
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, RwLock},
};

use crate::prelude::*;

impl Source<f32> for Synthesizer {
    fn next(&mut self) -> Option<Result<f32, anyhow::Error>> {
        let mut left = [0f32];
        let mut right = [0f32];
        self.render(&mut left, &mut right);

        // let mut testl = [0f32; 1];
        // let mut testr = [0f32; 1];

        // for _ in 0..100 {
        //     self.render(&mut testl, &mut testr);
        //     print!("{}:{} ", testl[0], testr[0]);
        // }
        Some(Ok(left[0]))
    }
}

impl Source<i8> for Synthesizer {
    fn next(&mut self) -> Option<Result<i8, anyhow::Error>> {
        todo!()
    }
}
impl Source<i16> for Synthesizer {
    fn next(&mut self) -> Option<Result<i16, anyhow::Error>> {
        todo!()
    }
}
impl Source<i32> for Synthesizer {
    fn next(&mut self) -> Option<Result<i32, anyhow::Error>> {
        todo!()
    }
}

impl BasicInstrument for Synthesizer {
    fn transition(&mut self, event: mlua::Value, lua: &mlua::Lua) -> mlua::Result<()> {
        let note = LuaUserDataRef::<Note>::from_lua(event, lua)?;
        self.note_off_all(false);
        // println!("note on:`{}`", note.freq());
        self.note_on(1, note.number(), 100);
        Ok(())
    }

    fn help(&mut self) -> String {
        todo!()
    }
}

#[derive(Clone)]
pub struct SFPlayer(Arc<RwLock<Synthesizer>>);

// impl std::fmt::Debug for SFPlayer {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "sfplayer")
//     }
// }

impl SFPlayer {
    pub fn load_sf2<P>(path: P) -> anyhow::Result<Synthesizer>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path.as_ref())?;
        let mut reader = BufReader::new(file);
        Ok(Synthesizer::new(
            &Arc::new(SoundFont::new(&mut reader)?),
            &SynthesizerSettings::new(44100),
        )?)
    }
}

impl LuaUserData for SFPlayer {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("note_parser", |_, this: &Self| {
            Ok(ParserUserData::new(PianoNoteParser(this.0.clone())))
        });
    }
}

#[rustfmt::skip]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Key { C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note(pub Key, pub(crate) usize);

impl Note {
    pub fn from_spanned_str(s: &[(usize, char)]) -> Result<Option<Self>, String> {
        // let s = s.chars().collect::<Vec<_>>();
        Ok(Some(match s.len() {
            2 => Note(
                match s[0].1 {
                    'C' | 'c' => Key::C,
                    'D' | 'd' => Key::D,
                    'E' | 'e' => Key::E,
                    'F' | 'f' => Key::F,
                    'G' | 'g' => Key::G,
                    'A' | 'a' => Key::A,
                    'B' | 'b' => Key::B,
                    _ => return Err(format!("at {}: invalid key", s[0].0)),
                },
                String::from(s[1].1)
                    .parse()
                    .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
            ),
            3 => Note(
                match (s[0].1, s[1].1) {
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
                String::from(s[2].1)
                    .parse()
                    .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
            ),
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
        let num_c_o: i32 = match self.1 {
            octave @ 0..=9 => octave * 12,
            _ => unreachable!(),
        }
        .try_into()
        .unwrap();

        match self.0 {
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

    fn freq(&self) -> f32 {
        const INCR: f32 = 1.0594630943592953;
        let (freq_ao, freq_ao_1) = match self.1 {
            octave @ 0..=9 => (
                27.5 * 2f32.powf(octave as f32),
                27.5 * 2f32.powf(octave as f32 - 1.),
            ),
            _ => unreachable!(),
        };

        match self.0 {
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

impl LuaUserData for Note {}

// impl FromLua for Note {
//     fn from_lua(value: LuaValue, _lua: &Lua) -> mlua::Result<Self> {
//         use mlua::Error;

//         let spanned_str: Vec<_> = dbg!(value)
//             .as_string()
//             .ok_or(Error::runtime("invalid string: not UTF-8"))?
//             .to_str()?
//             .chars()
//             .enumerate()
//             .collect();

//         Ok(Note::from_spanned_str(&spanned_str)
//             .map_err(|err| Error::runtime(err))?
//             .ok_or(Error::runtime(
//                 "cannot make Note out of empty string".to_string(),
//             ))?)
//     }
// }

struct PianoNoteParser(Arc<RwLock<Synthesizer>>);

impl PianoNoteParser {
    fn parse_notes(&self, pattern_str: &[char]) -> Result<Vec<Note>, String> {
        let mut notes = Vec::new();
        let pattern_str = pattern_str.iter().copied().enumerate().collect::<Vec<_>>();

        for note_str in pattern_str.split(|(_, c)| c.is_whitespace()) {
            let Some(note) = Note::from_spanned_str(note_str)? else {
                continue;
            };
            // println!("{note:?}: {}", note.freq());
            notes.push(note);
        }
        Ok(notes)
    }
}

impl Parser for PianoNoteParser {
    fn extend(&mut self, _argument: LuaValue, _lua: &Lua) -> mlua::Result<()> {
        Err(LuaError::runtime(
            "piano-note-parser does not use any arguments",
        ))
    }

    fn parse(&mut self, pattern_str: &str, lua: &Lua) -> mlua::Result<ParseOutput> {
        self.parse_notes(pattern_str.chars().collect::<Vec<_>>().as_slice())
            .map_err(LuaError::runtime)?
            .into_iter()
            .enumerate()
            .map(|(id, note)| {
                note.into_lua(lua).map(|note| {
                    (
                        id,
                        Emittable {
                            instrument: self.0.clone(),
                            event: note,
                        },
                    )
                })
            })
            // iter of results into result of vec which fails at first failure
            .collect::<LuaResult<Vec<_>>>()
    }

    fn help(&mut self) -> String {
        todo!()
    }
}

// TODO downsampling

//     use crate::prelude::*;

//     struct MidiInstrument;

//     impl<S> Source<S> for MidiInstrument {
//         fn next(&mut self) -> Option<Result<S, anyhow::Error>> {
//             todo!()
//         }
//     }

//     impl BasicInstrument for MidiInstrument {
//         fn transition(&mut self, event: &mlua::Value) -> mlua::Result<()> {
//             todo!()
//         }
//     }

//     struct MidiParser;

//     impl Parser for MidiParser {
//         fn extend(&mut self, argument: mlua::Value) -> mlua::Result<()> {
//             todo!()
//         }

//         fn parse(&mut self, pattern_str: &str) -> Vec<(GridIndex, Emittable)> {
//             todo!()
//         }
//     }
//

#[cfg(test)]
mod tests {
    use mlua::{FromLua, Lua, UserDataRef, Value};
    use std::{
        io::Read,
        sync::{Arc, LazyLock, RwLock},
    };

    use crate::{engine::Engine, midi_example::SFPlayer, parser::ParseOutput, player, utils::W};

    static SAMPLES: LazyLock<RwLock<Vec<f32>>> = LazyLock::new(|| RwLock::new(Vec::new()));

    #[test]
    fn test_midi() {
        const UNITS: usize = 16;

        let lua = Lua::new();

        lua.globals()
            .set(
                "loadSF2",
                lua.create_function(|_, path: String| {
                    SFPlayer::load_sf2(path)
                        .map_err(mlua::Error::runtime)
                        .map(|synth| SFPlayer(Arc::new(RwLock::new(synth))))
                })
                .unwrap(),
            )
            .unwrap();

        lua.globals()
            .set(
                "export",
                lua.create_function_mut(|lua, (track, interval): (Value, usize)| {
                    let engine = Engine::new();
                    let track = &UserDataRef::<W<ParseOutput>>::from_lua(track, lua)?.0;
                    *SAMPLES.write().unwrap() = engine
                        .render(lua, &[track.as_slice()], UNITS, interval)
                        .unwrap();
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();

        lua.load(
            r#"
                    piano  = loadSF2 './TimGM6mb.sf2'
                    parser = piano.note_parser

                    -- track  = parser:parse 'C5 Eb5 G5 Bb5 d6 Bb5 G5 Eb5 C5 B4'
                    track  = parser:parse 'A5 C6 E6 C6 F5 A5 C6 A5 C5 E5 G5 E5 G5 B5 D6 B5'

                    song   = export (track, 44100 / 4)
                "#,
        )
        .exec()
        .unwrap();

        _ = std::fs::remove_file("./two.wav");
        _ = std::fs::File::open("./one.wav").unwrap();
        player::export_wav("./two.wav", SAMPLES.read().unwrap().as_slice());

        let mut file_one = Vec::new();
        std::fs::File::open("./one.wav")
            .unwrap()
            .read_to_end(&mut file_one)
            .unwrap();

        let mut file_two = Vec::new();
        std::fs::File::open("./two.wav")
            .unwrap()
            .read_to_end(&mut file_two)
            .unwrap();
        std::fs::remove_file("./two.wav").unwrap();

        assert_eq!(file_one, file_two);
    }
}
