use std::collections::HashMap;

use mlua::{FromLua, Lua, MetaMethod, UserData, UserDataMethods, Value};

use crate::{
    instruments::Emittable,
    types::GridIndex,
    utils::{self, Log},
};

pub trait Parser {
    /// Parse the pattern-string drawn by the user with this configured parser
    fn parse(
        &self,
        pattern_str: &[char],
        log: Log,
    ) -> Result<Vec<(GridIndex, Box<dyn Emittable>)>, String>;

    /// Extend the parser with a lua argument
    ///
    /// Implementors need to manually parse Emittable's from the argument if they want to
    /// use it. Refer to [`DefaultParser`] as a reference implementation
    fn extend(&mut self, argument: Value, lua: &Lua) -> Result<(), String>
    where
        Self: Sized;
}

pub(crate) struct ParserUserData<T: Parser>(pub(crate) T);

impl<T: Parser> UserData for ParserUserData<T> {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Read-only protection
        // TODO test
        methods.add_meta_function(MetaMethod::NewIndex, |_, _: ()| Ok(()));

        methods.add_method(
            "parse",
            |lua, ParserUserData(parser), pattern_str: String| {
                use mlua::Error;

                let log = Log(lua);
                log.debug("called parse")?;
                Ok(parser
                    .parse(&pattern_str.chars().collect::<Vec<_>>(), log)
                    .map_err(|err| Error::runtime(err))?
                    .into_iter()
                    .collect::<HashMap<GridIndex, Box<dyn Emittable>>>())
            },
        );

        methods.add_method_mut("extend", |lua, ParserUserData(parser), argument: Value| {
            use mlua::Error;

            utils::lua_debug(lua, "called extend")?;
            parser
                .extend(argument, lua)
                .map_err(|err| Error::runtime(err))
        });
    }
}

pub mod default_parser {
    use super::*;
    use mlua::UserDataRefMut;

    pub struct DefaultParser(pub(crate) Option<ParseTable>);

    impl Parser for DefaultParser {
        fn parse(
            &self,
            pattern_str: &[char],
            _log: Log,
        ) -> Result<Vec<(GridIndex, Box<dyn Emittable>)>, String> {
            match &self.0 {
                None => Err("Need a parse-table to initialize the parser".to_string()),
                Some(parse_table) => Ok(parse_table.parse(pattern_str)),
            }
        }

        fn extend(&mut self, argument: Value, lua: &Lua) -> Result<(), String> {
            *self = DefaultParser(Some(
                ParseTable::from_lua(argument, lua).map_err(|err| err.to_string())?,
            ));
            Ok(())
        }
    }

    impl UserData for DefaultParser {}

    pub(crate) enum ParseTable {
        Map(Vec<(String, Box<dyn Emittable>)>),
        Single(Box<dyn Emittable>),
    }

    impl ParseTable {
        // TODO Insertion sort
        pub fn insert_sort(inputs: impl Iterator<Item = (String, Box<dyn Emittable>)>) -> Self {
            let mut map = inputs.collect::<Vec<_>>();
            map.sort_unstable_by(|a, b| a.0.len().cmp(&b.0.len()));
            Self::Map(map)
        }

        pub fn parse(&self, pattern_str: &[char]) -> Vec<(GridIndex, Box<dyn Emittable>)> {
            println!("default-parser now parsing {:?}", pattern_str);

            match self {
                ParseTable::Map(map) => {
                    let mut emit_map = Vec::new();
                    let mut read = 0;
                    while pattern_str.get(read).is_some() {
                        for (key, emit) in map {
                            if let Some(pattern_end) =
                                utils::string_match(pattern_str, read, key.chars(), false)
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
                            utils::lua_debug(
                                lua,
                                &format!("reading pair of passed lua parse-table with key '{key}'"),
                            )
                            .unwrap();
                            UserDataRefMut::<Box<dyn Emittable>>::from_lua(
                                emittable_lua.clone(),
                                lua,
                            )
                            .map(|emittable| (key, emittable.clone()))
                        })
                    })
                    .process_results(|it| it.process_results(|it| ParseTable::insert_sort(it)))?,

                // Single clip-event to trigger repeatedly every unit
                ref d @ Value::UserData(_) => Ok(ParseTable::Single(
                    UserDataRefMut::<Box<dyn Emittable>>::from_lua(d.clone(), lua)?.clone(),
                )),

                _ => Err(mlua::Error::FromLuaConversionError {
                    from: value.type_name(),
                    to: "DefaultParser".into(),
                    message: Some("Don't know how to convert".into()),
                }),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::instruments::{
            wav_instrument::{ClipEvent, Wav},
            EmittableInstrument,
        };
        use std::{
            collections::HashMap,
            sync::{Arc, RwLock},
        };

        #[test]
        fn test_parse_table() {
            use ClipEvent::*;

            let instrument = Arc::new(RwLock::new(Wav::<i16>::load("./bt.wav").unwrap()));
            let inputs: HashMap<String, _> =
                [("[", Play), ("]", Stop), (")", Pause), ("(", Resume)]
                    .into_iter()
                    .map(|(key, event)| -> (_, Box<dyn Emittable>) {
                        (
                            key.to_string(),
                            Box::new(EmittableInstrument {
                                instrument: instrument.clone(),
                                event,
                            }),
                        )
                    })
                    .collect::<HashMap<_, _>>();

            let parse_table = ParseTable::insert_sort(inputs.into_iter());

            let pattern_str = "[......][......)        (......]";

            assert_eq!(
                parse_table
                    .parse(&pattern_str.chars().collect::<Vec<_>>())
                    .into_iter()
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>(),
                Vec::from([0, 7, 8, 15, 24, 31,])
            )
        }
    }
}

pub mod piano_note_parser {
    use std::sync::{Arc, RwLock};

    use crate::{
        instruments::{
            midi_instrument::{MidiPlayer, Note},
            EmittableInstrument,
        },
        types::SampleDepth,
    };

    use super::*;

    /// Only supports C0 to B9 so far
    pub struct PianoNoteParser<T>(Arc<RwLock<MidiPlayer<T>>>);

    impl<T> PianoNoteParser<T> {
        fn parse_notes(&self, pattern_str: &[char]) -> Result<Vec<Note>, String> {
            let mut notes = Vec::new();
            let pattern_str = pattern_str.iter().copied().enumerate().collect::<Vec<_>>();

            for note_str in pattern_str.split(|(_, c)| c.is_whitespace()) {
                let Some(note) = Note::from_spanned_str(note_str)? else {
                    continue;
                };
                notes.push(note);
            }
            Ok(notes)
        }
    }

    impl<T> Parser for PianoNoteParser<T>
    where
        T: SampleDepth + 'static,
    {
        fn extend(&mut self, _argument: Value, _lua: &Lua) -> Result<(), String> {
            Err("piano-note-parser does not use any arguments".to_string())
        }

        fn parse(
            &self,
            pattern_str: &[char],
            _log: Log,
        ) -> Result<Vec<(GridIndex, Box<dyn Emittable>)>, String> {
            self.parse_notes(pattern_str).map(|notes| {
                notes
                    .into_iter()
                    .enumerate()
                    .map(|(id, note)| -> (_, Box<dyn Emittable>) {
                        (
                            id,
                            Box::new(EmittableInstrument::<MidiPlayer<T>> {
                                instrument: self.0.clone(),
                                event: note,
                            }),
                        )
                    })
                    .collect()
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::instruments::midi_instrument::Key;

        #[test]
        fn test_piano_note_parser() {
            let midi_player = Arc::new(RwLock::new(
                MidiPlayer::<i16>::new("dummyvalue.sf2").unwrap(),
            ));
            let parser = PianoNoteParser(midi_player.clone());

            assert_eq!(
                parser.parse_notes(
                    &"C5 Eb5 G5 Bb5 d6 Bb5 G5 Eb5 C5 B4"
                        .chars()
                        .collect::<Vec<_>>()
                ),
                Ok(Vec::from([
                    Note(Key::C, 5),
                    Note(Key::Eb, 5),
                    Note(Key::G, 5),
                    Note(Key::Bb, 5),
                    Note(Key::D, 6),
                    Note(Key::Bb, 5),
                    Note(Key::G, 5),
                    Note(Key::Eb, 5),
                    Note(Key::C, 5),
                    Note(Key::B, 4),
                ]))
            );

            assert_eq!(
                parser.parse_notes(&"C5 Eb5 6".chars().collect::<Vec<_>>()),
                Err("at 7: note string may only be or 2 or 3 characters long".to_string())
            );

            assert_eq!(
                parser.parse_notes(&"C5 Eb5 67".chars().collect::<Vec<_>>()),
                Err("at 7: invalid key".to_string())
            );

            assert_eq!(
                parser.parse_notes(&"C5  Eb5  S7".chars().collect::<Vec<_>>()),
                Err("at 9: invalid key".to_string())
            );

            assert_eq!(
                parser.parse_notes(&"C5 Eb5 F%".chars().collect::<Vec<_>>()),
                Err("at 8: invalid octave number".to_string())
            );
        }
    }
}
