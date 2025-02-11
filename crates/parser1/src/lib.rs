use std::str::Chars;

use libplunder::is_event;
use log::{info, trace};
use mlua::prelude::*;

pub struct Parser(pub(crate) Option<ParseTable>);

impl Parser {
    pub fn new() -> Self {
        Parser(None)
    }
}

impl Parser {
    fn parse(&self, pattern_str: &[char]) -> LuaResult<Vec<(usize, LuaValue)>> {
        self.0
            .as_ref()
            .ok_or(LuaError::runtime(
                "Need a parse-table to initialize the parser".to_string(),
            ))?
            .parse(pattern_str)
    }

    fn extend(&mut self, argument: LuaValue, lua: &Lua) -> Result<(), String> {
        *self = Parser(Some(
            ParseTable::from_lua(argument, lua).map_err(|err| err.to_string())?,
        ));
        Ok(())
    }
}

impl LuaUserData for Parser {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // Read-only protection
        // TODO test
        methods.add_meta_function(LuaMetaMethod::NewIndex, |_, _: ()| Ok(()));

        methods.add_method("parse", |lua, parser: &Parser, pattern_str: String| {
            parser
                .parse(&pattern_str.chars().collect::<Vec<_>>())?
                .into_iter()
                .map(|(id, event)| {
                    let table = lua.create_table()?;
                    table.push(id)?;
                    table.push(event)?;
                    Ok(table)
                })
                .collect::<LuaResult<Vec<LuaTable>>>()
        });

        methods.add_method_mut("extend", |lua, parser: &mut Parser, argument: LuaValue| {
            parser
                .extend(argument, lua)
                .map_err(|err| LuaError::runtime(err))
        });
    }
}

pub(crate) enum ParseTable {
    Map(Vec<(String, LuaValue)>),
    Single(LuaValue),
}

impl ParseTable {
    // TODO Insertion sort
    pub fn insert_sort(inputs: impl Iterator<Item = (String, LuaValue)>) -> Self {
        let mut map = inputs.collect::<Vec<_>>();
        map.sort_unstable_by(|a, b| a.0.len().cmp(&b.0.len()));
        Self::Map(map)
    }

    pub fn parse(&self, pattern_str: &[char]) -> LuaResult<Vec<(usize, LuaValue)>> {
        info!("default-parser now parsing {:?}", pattern_str);

        match self {
            ParseTable::Map(map) => {
                let mut emit_map = Vec::new();
                let mut read = 0;
                while pattern_str.get(read).is_some() {
                    for (key, emit) in map {
                        if let Some(pattern_end) =
                            string_match(pattern_str, read, key.chars(), false)
                        {
                            trace!("matched '{}', pushing at {}", key, read);
                            match emit {
                                _ if is_event(emit) => {
                                    emit_map.push((read, emit.clone()));
                                }
                                LuaValue::Table(table) => table
                                    .pairs::<LuaValue, LuaValue>()
                                    .try_for_each(|pair| -> LuaResult<_> {
                                        let (_, emit) = pair?;
                                        emit_map.push((read, emit));
                                        Ok(())
                                    })?,
                                v @ _ => Err(LuaError::runtime(format!(
                                    "unexpected value in sequence of events: `{}` is not an event",
                                    v.to_string()?
                                )))?,
                            }
                            read = pattern_end;
                            break;
                        }
                    }
                    read = read + 1;
                }
                Ok(emit_map)
            }
            ParseTable::Single(emit) => Ok(std::iter::repeat(emit)
                .cloned()
                .enumerate()
                .take(pattern_str.len())
                .collect()),
        }
    }
}

impl FromLua for ParseTable {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        use itertools::Itertools;

        match value {
            // Single clip-event to trigger repeatedly every unit
            _ if is_event(&value) => Ok(ParseTable::Single(value)),

            // Map of what emit-event to trigger when string encountered
            LuaValue::Table(table) => Ok(table
                .pairs::<String, LuaValue>()
                .process_results(|it| ParseTable::insert_sort(it))?),

            _ => Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "ParseTable".into(),
                message: Some("Don't know how to convert for use in Parser1".into()),
            }),
        }
    }
}

pub fn string_match(haystack: &[char], start: usize, needle: Chars, _regex: bool) -> Option<usize> {
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

// #[cfg(test)]
// mod tests {
//     use sampler::Sampler;

//     use super::*;

//     use std::{
//         collections::HashMap,
//         sync::{Arc, RwLock},
//     };

//     // #[test]
//     // fn test_parse_table() {
//     //     let sample = Sampler::load("one.wav", true).unwrap();
//     //     let inputs: HashMap<String, _> = [("[", Play), ("]", Stop), (")", Pause), ("(", Resume)]
//     //         .into_iter()
//     //         .map(|(key, event)| -> (_, EmittableUserData) {
//     //             (
//     //                 key.to_string(),
//     //                 Box::new(EmittableInstrument {
//     //                     instrument: instrument.clone(),
//     //                     event,
//     //                 }),
//     //             )
//     //         })
//     //         .collect::<HashMap<_, _>>();

//     //     let parse_table = ParseTable::insert_sort(inputs.into_iter());

//     //     let pattern_str = "[......][......)        (......]";

//     //     assert_eq!(
//     //         parse_table
//     //             .parse(&pattern_str.chars().collect::<Vec<_>>())
//     //             .into_iter()
//     //             .map(|(i, _)| i)
//     //             .collect::<Vec<_>>(),
//     //         Vec::from([0, 7, 8, 15, 24, 31,])
//     //     )
//     // }
// }
