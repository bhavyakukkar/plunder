// 1. TODO units should be inferred by the engine instead of being parsed randomly or maybe it should explicitly be provided by the parser?
// 2. TODO experiment with using bits_per_sample(u16), sample_format(enum{Float, Int}) like hound instead of trait SampleDepth. we're still panicking from sample formats we don't expect

#![warn(missing_debug_implementations)]

mod lib {
    pub mod prelude {
        pub use super::{
            instrument::{BasicInstrument, Emittable, Source},
            lua::{InstrumentUserData, ParserUserData},
            parser::{ParseOutput, Parser},
            types::{GridIndex, SampleDepth},
        };

        pub use mlua::prelude::*;
    }

    pub mod types {
        pub type GridIndex = usize;

        pub trait SampleDepth: hound::Sample + std::ops::AddAssign + std::fmt::Debug {
            const MAX: Self;
            const MIN: Self;
            const MID: Self;
        }
    }

    pub mod instrument {
        use std::sync::{Arc, RwLock};

        pub trait Source<S> {
            fn next(&mut self) -> Option<Result<S, anyhow::Error>>;
        }

        pub trait BasicInstrument: Source<i8> + Source<i16> + Source<i32> + Source<f32> //+ fmt::Debug
        {
            fn next_i8(&mut self) -> Option<Result<i8, anyhow::Error>> {
                <Self as Source<i8>>::next(self)
            }
            fn next_i16(&mut self) -> Option<Result<i16, anyhow::Error>> {
                <Self as Source<i16>>::next(self)
            }
            fn next_i32(&mut self) -> Option<Result<i32, anyhow::Error>> {
                <Self as Source<i32>>::next(self)
            }
            fn next_f32(&mut self) -> Option<Result<f32, anyhow::Error>> {
                <Self as Source<f32>>::next(self)
            }

            fn transition(&mut self, event: mlua::Value, lua: &mlua::Lua) -> mlua::Result<()>;

            fn help(&mut self) -> String;
        }

        #[derive(/* Debug, */ Clone)]
        pub struct Emittable {
            pub instrument: Arc<RwLock<dyn BasicInstrument>>,
            pub event: mlua::Value,
        }
    }

    pub mod parser {
        use super::{instrument::Emittable, types::GridIndex};

        pub type ParseOutput = Vec<(GridIndex, Emittable)>;

        /// A parser converts a [`mlua::Value`] into a sequence of emittable events
        pub trait Parser {
            fn extend(&mut self, argument: mlua::Value, lua: &mlua::Lua) -> mlua::Result<()>;
            fn parse(&mut self, pattern_str: &str, lua: &mlua::Lua) -> mlua::Result<ParseOutput>;
            fn help(&mut self) -> String;
        }
    }

    pub mod engine {
        use super::{
            instrument::{BasicInstrument, Emittable, Source},
            types::{GridIndex, SampleDepth},
        };
        use std::{
            collections::HashSet,
            hash::Hash,
            marker::PhantomData,
            sync::{Arc, RwLock},
        };

        struct ArcPtr<T: ?Sized>(Arc<T>);

        impl<T: ?Sized> std::cmp::PartialEq for ArcPtr<T> {
            fn eq(&self, other: &Self) -> bool {
                // TODO test in playground
                std::ptr::addr_eq(Arc::as_ptr(&self.0), Arc::as_ptr(&other.0))
            }
        }

        impl<T: ?Sized> Eq for ArcPtr<T> {}

        impl<T: ?Sized> Hash for ArcPtr<T> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                Arc::as_ptr(&self.0).hash(state)
            }
        }

        pub struct Engine<S> {
            active_instruments: HashSet<ArcPtr<RwLock<dyn BasicInstrument>>>,
            // lua: &'a mlua::Lua,
            s: PhantomData<S>,
        }

        impl<S> Engine<S>
        where
            (dyn BasicInstrument + 'static): Source<S>,
        {
            pub fn new(/*lua: &'a mlua::Lua*/) -> Self {
                Self {
                    active_instruments: HashSet::new(),
                    // lua,
                    s: PhantomData,
                }
            }

            fn tick(
                &mut self,
                elem: &(GridIndex, Emittable),
                lua: &mlua::Lua,
            ) -> Result<(), anyhow::Error>
            where
                S: SampleDepth,
            {
                // transition the instrument using the event
                let instrument = match self
                    .active_instruments
                    .get(&ArcPtr(elem.1.instrument.clone()))
                {
                    // if exists, get mutable reference to instrument
                    Some(_) => elem.1.instrument.clone(),

                    // if doesn't exist, insert and return mutable reference to instrument
                    None => {
                        self.active_instruments
                            .insert(ArcPtr(elem.1.instrument.clone()));
                        elem.1.instrument.clone()
                    }
                };

                let mut instrument = instrument.write().map_err(|err| {
                    anyhow::anyhow!("concurrency error transitioning the instrument: {}", err)
                })?;
                // .transition(&elem.1.event, self.lua)
                instrument
                    .transition(elem.1.event.clone(), lua)
                    .map_err(|err| anyhow::anyhow!("transition error: {err}"))
                // Ok(())
            }

            pub fn render(
                mut self,
                lua: &mlua::Lua,
                event_list: &[&[(GridIndex, Emittable)]],
                len: GridIndex,
                interval: usize,
            ) -> Result<Vec<S>, anyhow::Error>
            where
                S: SampleDepth,
            {
                let mut samples = Vec::with_capacity(len * interval);
                let num_playlists = event_list.len();
                if num_playlists == 0 {
                    return Ok(Vec::new());
                }
                let mut heads = vec![0; num_playlists];
                for j in 0..len {
                    for i in 0..num_playlists {
                        match event_list[i].get(heads[i]) {
                            Some(elem) => {
                                // println!("inserted 1 event at {j}");
                                if j == elem.0 {
                                    self.tick(elem, lua)?;
                                    heads[i] += 1;
                                }
                            }
                            None => (),
                        }
                    }

                    // let mut num_instruments = 0;
                    for _ in 0..interval {
                        let mut sample = S::MID;
                        // print!("(");
                        for instrument in &self.active_instruments {
                            // num_instruments += 1;
                            sample += instrument
                                .0
                                .write()
                                .map_err(|err| {
                                    anyhow::anyhow!(
                                        "concurrency error accessing instrument: {}",
                                        err
                                    )
                                })?
                                .next()
                                .transpose()?
                                .unwrap_or(S::MID);
                            // print!("sample:{sample:?}, adding:{adding:?}\t", adding = adding);
                        }
                        // println!(")");
                        samples.push(sample);
                    }
                }

                Ok(samples)
            }
        }
    }

    pub mod player {
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
    }

    pub mod utils {
        use std::str::Chars;

        /// Simple Wrapper
        pub struct W<T>(pub T);

        pub fn string_match(
            haystack: &[char],
            start: usize,
            needle: Chars,
            _regex: bool,
        ) -> Option<usize> {
            let mut last = None;
            for (i, nc) in needle.enumerate() {
                if haystack.get(start + i).is_none_or(|hc| *hc != nc) {
                    return None;
                }
                last = last.or(Some(start));
                last = Some(last.unwrap() + 1);
            }
            Some(last.unwrap() - 1)
        }
    }

    pub mod lua {
        use mlua::prelude::*;
        // use mlua::{FromLua, IntoLua, MetaMethod, UserData, Value};
        use std::sync::{Arc, RwLock};

        use super::{
            instrument::BasicInstrument,
            parser::{ParseOutput, Parser},
            prelude::Emittable,
            utils::W,
        };

        // pub struct InstrumentUserData(pub Arc<RwLock<dyn BasicInstrument>>);
        pub struct InstrumentUserData<T>(Arc<RwLock<T>>);

        impl<T> InstrumentUserData<T> {
            pub fn new(instrument: T) -> Self {
                Self(Arc::new(RwLock::new(instrument)))
            }
        }

        impl LuaUserData for Emittable {}

        impl<T> LuaUserData for InstrumentUserData<T>
        where
            T: BasicInstrument + 'static,
        {
            fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
                // read-only protection
                methods.add_meta_method(LuaMetaMethod::NewIndex, |_, _, _: LuaValue| {
                    Ok(LuaValue::Nil)
                });

                methods.add_meta_method_mut(
                    LuaMetaMethod::Index,
                    |_, InstrumentUserData(instrument), event| {
                        Ok(Emittable {
                            instrument: instrument.clone(),
                            event,
                        })
                    },
                );
            }
        }

        impl LuaUserData for W<ParseOutput> {}

        // TODO use when you want parser.parse()'s output to be usable from lua
        /*
        mod stuff {
            impl IntoLua for W<ParseOutput> {
                fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<Value> {
                    let array = lua.create_table()?;
                    for (grid_index, emittable) in self.0 {
                        let item = lua.create_table()?;
                        item.push(grid_index)?;
                        item.push(emittable.into_lua(lua)?)?;
                        array.push(item)?;
                    }
                    Ok(Value::Table(array))
                }
            }

            impl FromLua for W<ParseOutput> {
                fn from_lua(value: Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                    todo!()
                }
            }
        }
        */

        impl<T> From<T> for W<T> {
            fn from(value: T) -> Self {
                Self(value)
            }
        }

        /// A container for a Parser that implements LuaUserData and calls Parser.extend or Parser.parse when the respective methods are called from Lua
        pub struct ParserUserData<T>(Arc<RwLock<T>>);

        impl<T> ParserUserData<T> {
            pub fn new(parser: T) -> Self {
                Self(Arc::new(RwLock::new(parser)))
            }
        }

        impl<T> LuaUserData for ParserUserData<T>
        where
            T: Parser,
        {
            fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
                use mlua::IntoLua;

                // read-only protection
                methods.add_meta_method(LuaMetaMethod::NewIndex, |_, _, _: LuaValue| {
                    Ok(LuaValue::Nil)
                });

                methods.add_method_mut(
                    "extend",
                    |lua, ParserUserData(parser), argument: LuaValue| {
                        parser
                            .write()
                            .map_err(|err| {
                                mlua::Error::runtime(format!(
                                    "concurrency error extending the parser: {}",
                                    err
                                ))
                            })?
                            .extend(argument, lua)
                    },
                );

                methods.add_method_mut(
                    "parse",
                    |lua, ParserUserData(parser), pattern_str: LuaString| {
                        parser
                            .write()
                            .map_err(|err| {
                                mlua::Error::runtime(format!(
                                    "concurrency error extending the parser: {}",
                                    err
                                ))
                            })?
                            .parse(&pattern_str.to_string_lossy(), lua)
                            .map(|emit_list| W(emit_list).into_lua(lua))
                    },
                );
            }
        }
    }

    pub mod math {
        use super::types::SampleDepth;

        impl SampleDepth for i8 {
            const MAX: i8 = i8::MAX;
            const MIN: i8 = i8::MIN;
            const MID: i8 = 0;
        }
        impl SampleDepth for i16 {
            const MAX: i16 = i16::MAX;
            const MIN: i16 = i16::MIN;
            const MID: i16 = 0;
        }
        impl SampleDepth for i32 {
            const MAX: i32 = i32::MAX;
            const MIN: i32 = i32::MIN;
            const MID: i32 = 0;
        }
        impl SampleDepth for f32 {
            const MAX: f32 = f32::MAX;
            const MIN: f32 = f32::MIN;
            const MID: f32 = 0.;
        }
    }
}

mod midi_example {
    use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
    use std::{
        fs::File,
        io::BufReader,
        path::Path,
        sync::{Arc, RwLock},
    };

    use crate::lib::prelude::*;

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

    //     use crate::lib::prelude::*;

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

        use crate::{
            lib::{engine::Engine, parser::ParseOutput, player, utils::W},
            midi_example::SFPlayer,
        };

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
}

mod wav_instrument {
    use crate::lib::prelude::*;

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
}

mod default_parser {
    use mlua::{FromLua, UserDataRefMut};
    use std::str::Chars;

    use crate::lib::prelude::*;

    pub(crate) enum ParseTable {
        Map(Vec<(String, Emittable)>),
        Single(Emittable),
    }

    impl ParseTable {
        // TODO Insertion sort
        pub fn insert_sort(inputs: impl Iterator<Item = (String, Emittable)>) -> Self {
            let mut map = inputs.collect::<Vec<_>>();
            map.sort_unstable_by(|a, b| a.0.len().cmp(&b.0.len()));
            Self::Map(map)
        }

        pub fn parse(&self, pattern_str: &[char]) -> Vec<(GridIndex, Emittable)> {
            println!("default-parser now parsing {:?}", pattern_str);

            match self {
                ParseTable::Map(map) => {
                    let mut emit_map = Vec::new();
                    let mut read = 0;
                    while pattern_str.get(read).is_some() {
                        for (key, emit) in map {
                            if let Some(pattern_end) =
                                Self::string_match(pattern_str, read, key.chars(), false)
                            {
                                println!("matched '{}', pushing at {}", key, read);
                                emit_map.push((read, emit.clone()));
                                read = pattern_end;
                                break;
                            }
                        }
                        read = read + 1;
                    }
                    emit_map
                }
                ParseTable::Single(emit) => std::iter::repeat(emit)
                    .cloned()
                    .enumerate()
                    .take(pattern_str.len())
                    .collect(),
            }
        }

        fn string_match(
            haystack: &[char],
            start: usize,
            needle: Chars,
            _regex: bool,
        ) -> Option<usize> {
            let mut last = None;
            for (i, nc) in needle.enumerate() {
                if haystack.get(start + i).is_none_or(|hc| *hc != nc) {
                    return None;
                }
                last = last.or(Some(start));
                last = Some(last.unwrap() + 1);
            }
            Some(last.unwrap() - 1)
        }
    }

    impl FromLua for ParseTable {
        fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
            use itertools::Itertools;
            use mlua::Value;

            match value {
                // Map of what emit-event to trigger when string encountered
                Value::Table(table) => table
                    .pairs::<String, Value>()
                    .map(|pair| {
                        pair.map(|(key, emittable_lua)| {
                            UserDataRefMut::<Emittable>::from_lua(emittable_lua.clone(), lua)
                                .map(|emittable| (key, emittable.clone()))
                        })
                    })
                    .process_results(|it| it.process_results(|it| ParseTable::insert_sort(it)))?,

                // Single clip-event to trigger repeatedly every unit
                ref d @ Value::UserData(_) => Ok(ParseTable::Single(
                    UserDataRefMut::<Emittable>::from_lua(d.clone(), lua)?.clone(),
                )),

                _ => Err(mlua::Error::FromLuaConversionError {
                    from: value.type_name(),
                    to: "DefaultParser".into(),
                    message: Some("Don't know how to convert".into()),
                }),
            }
        }
    }

    struct DefaultParser(Option<ParseTable>);

    impl Parser for DefaultParser {
        fn extend(&mut self, argument: LuaValue, lua: &Lua) -> mlua::Result<()> {
            *self = DefaultParser(Some(ParseTable::from_lua(argument, lua)?));
            Ok(())
        }

        fn parse(&mut self, pattern_str: &str, _lua: &Lua) -> mlua::Result<ParseOutput> {
            match &self.0 {
                None => Err(mlua::Error::runtime(
                    "Need a parse-table to initialize the parser",
                )),
                Some(parse_table) => {
                    Ok(parse_table.parse(&pattern_str.chars().collect::<Vec<_>>()))
                }
            }
        }

        fn help(&mut self) -> String {
            todo!()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::DefaultParser;
        use mlua::{Lua, UserDataRef, Value};
        use std::sync::{LazyLock, RwLock};

        use crate::{
            lib::{
                engine::Engine,
                lua::{InstrumentUserData, ParserUserData},
                parser::ParseOutput,
                utils::W,
            },
            wav_instrument::Wav,
        };

        /*const FIRST_1000_SAMPLES: [f32; 1000] = [
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., -1., -17., -26., -175., -278.,
            -723., -1218., -1442., -2451., -1954., -2364., -2904., -1470., -3788., -1402., -4261.,
            -1843., -5106., -3275., -4846., -4359., -4975., -4075., -5853., -4503., -6237., -5206.,
            -7248., -6282., -8699., -8152., -9094., -8661., -8550., -8243., -8323., -8237., -7140.,
            -8123., -6510., -7825., -7070., -8273., -6862., -8683., -7205., -8692., -7963., -8341.,
            -7309., -7214., -7334., -7276., -7472., -7986., -6278., -6924., -5420., -5418., -5608.,
            -5387., -3608., -4339., -3146., -4229., -3271., -3278., -1584., -2387., 212., -1796.,
            1171., -1103., 2394., -175., 3068., 655., 2399., 388., 2550., 489., 4550., 2216.,
            6906., 4800., 7663., 4780., 7880., 4916., 7887., 4422., 7002., 2958., 6902., 2645.,
            8152., 4298., 9677., 5726., 10265., 5951., 10361., 6065., 9513., 5911., 8262., 5883.,
            7354., 5595., 8822., 6768., 10984., 9482., 12944., 11670., 13667., 11984., 14435.,
            12252., 13452., 11993., 12650., 11400., 13584., 11653., 14514., 12326., 14061., 11368.,
            13308., 9862., 12482., 9630., 12591., 9669., 12346., 9394., 11691., 8349., 10573.,
            7139., 10438., 6942., 10613., 7613., 10871., 8329., 10883., 7939., 10486., 7276.,
            8517., 5510., 7185., 4810., 7632., 5060., 8024., 5593., 8128., 6024., 7079., 4818.,
            5332., 3771., 3840., 2443., 2688., 1899., 2070., 1328., 2683., 2542., 1530., 1435.,
            608., 356., 314., -303., -900., -2048., -3658., -4495., -5376., -5658., -5427., -5044.,
            -5766., -5491., -6223., -6055., -7939., -7763., -9895., -9874., -11458., -10613.,
            -11971., -11831., -12827., -13160., -14066., -13964., -13258., -13198., -11676.,
            -12875., -10817., -12726., -11140., -13472., -11861., -14386., -13998., -15426.,
            -15302., -15893., -14250., -14377., -13890., -13367., -14742., -14097., -15871.,
            -15951., -17291., -17047., -19807., -18376., -20577., -19101., -18941., -18965.,
            -18803., -18507., -18657., -17840., -18409., -16973., -17892., -16686., -17005.,
            -16637., -15973., -15604., -16683., -16056., -16836., -15838., -15645., -15453.,
            -14630., -14003., -14884., -13405., -15290., -12840., -14837., -12524., -13211.,
            -12443., -12861., -12323., -12952., -11996., -11720., -10864., -10750., -10240.,
            -11459., -10544., -12154., -11597., -13003., -11830., -12970., -11403., -11975.,
            -11044., -11256., -10234., -10580., -9158., -9693., -8195., -8865., -7972., -8620.,
            -6758., -8011., -5797., -7214., -5251., -5887., -4584., -3874., -2263., -1380., 406.,
            183., 1399., 1055., 3086., 1917., 2902., 2420., 2574., 2181., 2809., 3104., 3377.,
            4187., 4389., 4366., 5279., 5256., 5918., 6379., 6957., 6912., 8066., 7269., 8884.,
            7725., 8439., 6715., 7580., 4830., 6058., 5100., 5455., 6648., 7303., 6948., 7860.,
            7454., 8685., 7930., 9331., 8736., 10045., 10799., 11535., 12608., 13783., 14465.,
            16779., 16234., 18866., 17985., 20973., 18544., 21883., 18547., 21990., 18977., 23223.,
            20125., 24676., 22993., 27056., 24982., 28948., 25875., 29941., 25862., 29728., 25493.,
            29355., 24882., 28625., 24248., 27986., 23107., 26663., 22795., 26160., 22262., 25111.,
            20489., 23062., 17504., 20289., 14438., 17249., 12102., 15785., 9708., 14172., 8809.,
            12219., 7696., 10518., 5934., 9000., 4010., 7284., 2582., 5039., 2009., 3897., 1684.,
            2993., 577., 1949., -2610., -569., -3823., -2990., -2942., -2325., -4240., -2989.,
            -6709., -4787., -8114., -6220., -8788., -6504., -8445., -5104., -8611., -5121.,
            -10754., -7387., -12422., -8371., -12253., -8127., -14099., -9973., -14969., -11987.,
            -15796., -13229., -16086., -13935., -16520., -14788., -16717., -15287., -16860.,
            -15091., -15488., -14117., -14994., -13842., -14080., -12520., -12869., -11518.,
            -11705., -10734., -10860., -10520., -9091., -8353., -8262., -6386., -8296., -6006.,
            -6829., -6483., -4856., -5975., -3087., -3618., -3633., -3126., -5241., -5389., -5483.,
            -6989., -2312., -4125., -493., -2526., -1078., -3383., -1242., -3534., -947., -2319.,
            -257., -1654., 533., -1184., 1188., -716., 606., -703., 554., 465., 565., 87., -506.,
            -2144., -1615., -2968., -1527., -2323., -887., -3441., -73., -3520., -1072., -3650.,
            -1425., -3355., 359., -2131., 1047., -2468., 1096., -2054., 1578., -1188., 1933., 1.,
            2073., 436., 1517., -30., 954., -181., 1143., 547., 1651., 210., 1224., -1568., 1702.,
            -1469., 1644., -2044., 939., -1918., 15., -1826., 377., -2426., 995., -2629., 358.,
            -2951., -608., -3542., -1473., -4096., -2318., -4657., -3996., -6970., -4696., -7537.,
            -4840., -8615., -4505., -9256., -3834., -9094., -5191., -9000., -5441., -9430., -4168.,
            -9743., -3580., -9560., -3190., -8846., -2891., -7832., -3749., -7471., -3677., -6564.,
            -3322., -5967., -3867., -6133., -2613., -5356., -1339., -4812., -1370., -4786., -276.,
            -4078., 649., -2336., 538., -1891., 1626., -675., 3687., 1471., 3427., 1725., 2839.,
            727., 2778., 1153., 1479., -13., 653., -1296., 629., -236., 482., -52., 427., -1553.,
        ];*/

        #[rustfmt::skip]
        const FIRST_1000_SAMPLES: [f32; 1000] = [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.6797865e-7, 1.0142562e-6, 3.1263526e-6, 6.8213776e-6, 1.2063852e-5,
            1.6710525e-5, 1.5797394e-5, 5.0552558e-6, -1.7103846e-5, -4.673276e-5, -6.798574e-5,
            -5.7984143e-5, -8.6038114e-7, 0.00010821957, 0.00024404166, 0.00033728272,
            0.0003162533, 0.00014356288, -0.0001841893, -0.0006287382, -0.0011175043,
            -0.0015852847, -0.0019948035, -0.0023166453, -0.0025067513, -0.002519844,
            -0.0023368353, -0.001965969, -0.0015193247, -0.0012526999, -0.0013989118,
            -0.0020611219, -0.003152573, -0.004194317, -0.004426646, -0.003283052, -0.00056004385,
            0.003647318, 0.008991986, 0.015045886, 0.02145638, 0.027945278, 0.034034595,
            0.038959343, 0.04213112, 0.043371234, 0.04283908, 0.041029982, 0.038594972, 0.03606573,
            0.033772994, 0.03183017, 0.030136492, 0.02852552, 0.026879407, 0.025110066,
            0.023077294, 0.02058543, 0.017504014, 0.013813687, 0.00956811, 0.0048657884,
            -0.00017936913, -0.0054617748, -0.010890249, -0.016261088, -0.021199657, -0.025396248,
            -0.028738586, -0.031176282, -0.032534007, -0.03254865, -0.03109803, -0.0282561,
            -0.024339385, -0.019944621, -0.015680159, -0.011934435, -0.008924247, -0.00693394,
            -0.0063842377, -0.007542711, -0.010398738, -0.014586668, -0.019228794, -0.023268212,
            -0.025980776, -0.027062807, -0.02669272, -0.025530882, -0.024276005, -0.023370832,
            -0.023063056, -0.023504417, -0.024789305, -0.026921073, -0.02981634, -0.0330744,
            -0.035760444, -0.036875103, -0.03586361, -0.032620378, -0.027476445, -0.021117568,
            -0.014220048, -0.0072467946, -0.0005921808, 0.0052083973, 0.009580285, 0.012205264,
            0.0130672585, 0.012345929, 0.010334419, 0.0073551037, 0.003702143, -0.00035890273,
            -0.0045268284, -0.008469373, -0.011948729, -0.014867939, -0.01715794, -0.018667798,
            -0.01924058, -0.018837262, -0.017534722, -0.015586763, -0.013439095, -0.01149377,
            -0.009973281, -0.008917657, -0.008124269, -0.0072390293, -0.0059804437, -0.0042160973,
            -0.0016747421, 0.0022798367, 0.00826411, 0.016512007, 0.026852429, 0.03839004,
            0.04955372, 0.059019305, 0.06615807, 0.07078811, 0.07294937, 0.07281896, 0.0707053,
            0.06699475, 0.06224562, 0.057238586, 0.052657116, 0.048882164, 0.04602543, 0.043909065,
            0.04215684, 0.040426258, 0.038508207, 0.036126036, 0.032750204, 0.02785019,
            0.021188408, 0.012820286, 0.0032120554, -0.0067721303, -0.01635422, -0.025104703,
            -0.03274377, -0.038889952, -0.04312688, -0.04527944, -0.04545203, -0.044023525,
            -0.041635912, -0.03892869, -0.036339834, -0.034150165, -0.032634158, -0.032095388,
            -0.03270749, -0.03445928, -0.03709765, -0.040038455, -0.04260893, -0.044361018,
            -0.04510704, -0.04489236, -0.043954346, -0.04256566, -0.040936474, -0.03929176,
            -0.038030617, -0.037657447, -0.038501572, -0.040639173, -0.043652967, -0.04644093,
            -0.04783553, -0.04717608, -0.044249382, -0.03900182, -0.031391773, -0.021521328,
            -0.009680118, 0.0035549598, 0.017118394, 0.02979001, 0.04075249, 0.049684417,
            0.05634338, 0.060286548, 0.061228126, 0.059278607, 0.05482983, 0.048511405, 0.04107346,
            0.033155903, 0.025193723, 0.017641865, 0.011191041, 0.006499026, 0.0038677678,
            0.003259956, 0.0042883297, 0.006268315, 0.008553502, 0.010722585, 0.012506622,
            0.013699557, 0.01415004, 0.013811338, 0.012734803, 0.011328485, 0.010531677, 0.0112616,
            0.013972723, 0.018713886, 0.025172131, 0.0327875, 0.041019104, 0.04947801, 0.057702187,
            0.06490047, 0.07023939, 0.0732755, 0.07397314, 0.0726194, 0.06974022, 0.06588001,
            0.061466675, 0.056854874, 0.052424554, 0.04855712, 0.04549114, 0.04328991, 0.04173747,
            0.04025334, 0.038211767, 0.03524032, 0.031195158, 0.02600165, 0.019576378, 0.011922407,
            0.0031655054, -0.0064069917, -0.016252099, -0.02574035, -0.03444054, -0.042171642,
            -0.048766874, -0.053916126, -0.057395503, -0.059224505, -0.059636053, -0.059148863,
            -0.05847301, -0.058182187, -0.05858415, -0.059710715, -0.0613011, -0.06298997,
            -0.0645083, -0.06570057, -0.066352196, -0.066111565, -0.06472309, -0.062148117,
            -0.058599494, -0.054676756, -0.051175658, -0.04867405, -0.047420256, -0.04728154,
            -0.047721356, -0.04814006, -0.048147354, -0.04746733, -0.04555468, -0.041553285,
            -0.034871876, -0.02542352, -0.013553666, -3.0149497e-5, 0.014243291, 0.02856795,
            0.042516537, 0.05550707, 0.06655761, 0.07490195, 0.08034674, 0.082995765, 0.08293016,
            0.08016113, 0.07484818, 0.06734424, 0.058222596, 0.048315242, 0.03843174, 0.02910308,
            0.020621931, 0.013302971, 0.0075822184, 0.0037295532, 0.0017196186, 0.0012138446,
            0.0014575211, 0.001534403, 0.00078411424, -0.0011088892, -0.0038938508, -0.00664488,
            -0.0084155835, -0.00870092, -0.007338799, -0.0043676607, 6.6257184e-5, 0.0057433234,
            0.012405474, 0.019555084, 0.026242584, 0.031468462, 0.034661226, 0.035680167,
            0.034651097, 0.031841215, 0.02758413, 0.022227334, 0.016174851, 0.01001058,
            0.004388351, -0.00024817474, -0.003730369, -0.0061331834, -0.007743963, -0.0089013595,
            -0.009868075, -0.010836511, -0.012004805, -0.013593272, -0.015745802, -0.018492432,
            -0.021757718, -0.025353752, -0.029054653, -0.03269071, -0.036165945, -0.039280675,
            -0.0416319, -0.042889345, -0.04296085, -0.041979052, -0.040387042, -0.03882128,
            -0.037782893, -0.037521448, -0.0381851, -0.03994681, -0.04292667, -0.047099687,
            -0.052275058, -0.0577661, -0.062351618, -0.06502559, -0.06537301, -0.06350663,
            -0.06009568, -0.056069423, -0.05217151, -0.048838276, -0.046360955, -0.04500761,
            -0.044929907, -0.046100117, -0.048251715, -0.05056649, -0.05178886, -0.05095998,
            -0.04770744, -0.041951496, -0.03364907, -0.022833696, -0.009730044, 0.0052736513,
            0.021320373, 0.036990624, 0.05107739, 0.06301113, 0.072578035, 0.07960199, 0.08392232,
            0.08560274, 0.08494796, 0.08233752, 0.07810905, 0.07257215, 0.066037096, 0.058847673,
            0.05160502, 0.04516553, 0.0401817, 0.036900923, 0.035179514, 0.034445897, 0.033964217,
            0.033179723, 0.031787805, 0.029652974, 0.026742494, 0.023101388, 0.018830182,
            0.014196499, 0.009943854, 0.007143531, 0.0065548657, 0.00841356, 0.012425887,
            0.017752279, 0.023431038, 0.028779315, 0.03339406, 0.036890578, 0.03879033,
            0.038797725, 0.036925014, 0.033447757, 0.028927248, 0.02403822, 0.01930141,
            0.015016067, 0.011289218, 0.008069331, 0.005239095, 0.0026849362, 0.00029680508,
            -0.0020713126, -0.0045944448, -0.007398303, -0.010530524, -0.013984979, -0.01772095,
            -0.021681804, -0.025809724, -0.030046795, -0.03420179, -0.0378954, -0.04081205,
            -0.042839434, -0.04397186, -0.044204313, -0.043523442, -0.0419765, -0.03967939,
            -0.03702724, -0.03485126, -0.033970594, -0.034807265, -0.037383087, -0.041169837,
            -0.045174103, -0.04852768, -0.050771672, -0.051684186, -0.0511114, -0.048971675,
            -0.04533275, -0.040386744, -0.03473681, -0.029542226, -0.025839439, -0.02411754,
            -0.024413988, -0.026322102, -0.029156838, -0.032301594, -0.035352584, -0.03772959,
            -0.038331762, -0.036073584, -0.030460898, -0.021554543, -0.009927082, 0.0034571388,
            0.017676875, 0.0320913, 0.046142794, 0.05907828, 0.07010222, 0.07877561, 0.08507027,
            0.088994145, 0.09033292, 0.08894607, 0.084995255, 0.07889651, 0.0714014, 0.063492544,
            0.055969257, 0.049275566, 0.043687977, 0.03949502, 0.036920726, 0.035986416,
            0.03652512, 0.037861522, 0.0386988, 0.03790684, 0.034967206, 0.029955449, 0.02370919,
            0.01750849, 0.012360778, 0.008768489, 0.0069177654, 0.0068340353, 0.008413283,
            0.011434634, 0.015567649, 0.020173173, 0.024317278, 0.027257983, 0.028668385,
            0.028528413, 0.02704584, 0.024539405, 0.021322303, 0.017655412, 0.013772183,
            0.0098978765, 0.006201039, 0.0027670488, -0.0003826268, -0.003277936, -0.005980301,
            -0.008553326, -0.011048482, -0.01348326, -0.015823167, -0.018018998, -0.020047328,
            -0.021914542, -0.023678688, -0.02545016, -0.027325537, -0.029352693, -0.031426597,
            -0.03311852, -0.033842806, -0.033219952, -0.03115654, -0.02801869, -0.024730306,
            -0.022221657, -0.021015966, -0.021300368, -0.023049526, -0.02611686, -0.030287873,
            -0.035321675, -0.040743217, -0.04560631, -0.048882976, -0.049977064, -0.04875537,
            -0.04563868, -0.04158729, -0.03752969, -0.034027454, -0.0314433, -0.030176984,
            -0.030645141, -0.033038855, -0.037279364, -0.04278115, -0.048268665, -0.052438557,
            -0.054545067, -0.0543221, -0.05168736, -0.04659232, -0.0391466, -0.029653482,
            -0.01860356, -0.0067306194, 0.005180847, 0.016574346, 0.02716591, 0.036546975,
            0.04393882, 0.04871726, 0.050750572, 0.0502366, 0.047554225, 0.043154065, 0.03748286,
            0.03093868, 0.024148608, 0.018233176, 0.014319271, 0.013005636, 0.014355862,
            0.01773594, 0.021858254, 0.025538057, 0.028104208, 0.029313993, 0.029312765,
            0.028469555, 0.027146552, 0.025610954, 0.024252754, 0.023736125, 0.024649004,
            0.027234618, 0.031417854, 0.036724824, 0.042381227, 0.047723364, 0.052381974,
            0.056064792, 0.058337536, 0.058782026, 0.057250034, 0.053853795, 0.04899403,
            0.04334287, 0.037526183, 0.031938795, 0.026781833, 0.022083566, 0.017757919,
            0.013694008, 0.009798447, 0.006041827, 0.0024923962, -0.00076142827, -0.0036839964,
            -0.0062943227, -0.008684807, -0.01101086, -0.013408004, -0.015950948, -0.018570941,
            -0.020940118, -0.022626108, -0.023356395, -0.023062734, -0.021873321, -0.020092765,
            -0.01803981, -0.015938958, -0.01402647, -0.012820518, -0.013064411, -0.015263847,
            -0.019527132, -0.025492724, -0.032241166, -0.038735777, -0.044284146, -0.048551623,
            -0.051320814, -0.05235793, -0.05157552, -0.04910426, -0.04537776, -0.041359045,
            -0.038255285, -0.036909297, -0.037653003, -0.04034578, -0.04442349, -0.049231526,
            -0.054278906, -0.05915645, -0.06317786, -0.06536765, -0.065041795, -0.062040765,
            -0.05652682, -0.04882242, -0.03931872, -0.028426796, -0.01653731, -0.00426987,
            0.007404035, 0.017678974, 0.02620944, 0.03282892, 0.037154894, 0.03863936, 0.037030585,
            0.032488428, 0.025765633, 0.018361464, 0.011818655, 0.0070863417, 0.004546139,
            0.004140723, 0.0055142115, 0.008223821, 0.011867295, 0.015988529, 0.019926298,
            0.022981344, 0.02472054, 0.02501492, 0.024177266, 0.02301973,
        ];

        static SAMPLES: LazyLock<RwLock<Vec<f32>>> = LazyLock::new(|| RwLock::new(Vec::new()));
        const UNITS: usize = 10;

        #[test]
        fn test_wav() {
            use mlua::FromLua;

            let lua = Lua::new();

            lua.globals()
                .set(
                    "loadWav",
                    lua.create_function(|_, path: String| {
                        Wav::load(path)
                            .map_err(mlua::Error::runtime)
                            .map(|wav| InstrumentUserData::new(wav))
                    })
                    .unwrap(),
                )
                .unwrap();

            lua.globals()
                .set(
                    "newDefaultParser",
                    lua.create_function(|_, _: Value| Ok(ParserUserData::new(DefaultParser(None))))
                        .unwrap(),
                )
                .unwrap();

            lua.globals()
                .set(
                    "export",
                    lua.create_function_mut(|lua, (track, interval): (Value, usize)| {
                        let engine = Engine::<f32>::new();
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
                    wav    = loadWav './one.wav'
                    parser = newDefaultParser()

                    track  = parser:extend { ['+'] = wav.play }
                    track  = parser:parse  '--+-----------------------------'

                    song   = export (track, 100)
                "#,
                // r#"
                //     wav    = loadWav '../plunder/bt.wav'
                //     parser = newWavParser()

                //     track = parser {
                //       ['['] = wav.play,
                //       [']'] = wav.stop,
                //       [')'] = wav.pause,
                //       ['('] = wav.resume
                //     }
                //     '[......][......)        (......]'

                //     song = export (
                //         track,
                //         100
                //     )
                // "#,
            )
            .exec()
            .unwrap();

            let interval = 100;

            assert_eq!(SAMPLES.read().unwrap().len(), UNITS * interval);

            // first 200 samples before instrument starts
            assert_eq!(
                SAMPLES
                    .read()
                    .unwrap()
                    .iter()
                    .copied()
                    .take(200)
                    .collect::<Vec<_>>(),
                vec![0.; 200]
            );

            // last 800 samples match that of wav
            assert_eq!(
                SAMPLES
                    .read()
                    .unwrap()
                    .iter()
                    .skip(200)
                    .map(|a| *a as f32)
                    .collect::<Vec<_>>(),
                &FIRST_1000_SAMPLES[..800]
            );
        }
    }
}
