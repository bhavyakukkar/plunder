// TODO move parser to different crate than synth
use std::sync::{Arc, RwLock};

use libplunder::prelude::instrument::*;
use log::{info, trace};
use mlua::prelude::*;

use crate::{instrument::Note, Synth};

pub struct MidiParser(PackagedInstrument);

impl MidiParser {
    pub fn new(synth: PackagedInstrument) -> Self {
        // let any: &mut dyn Any = &mut synth.factory.0;
        // // if can downcast as SharedPtr<Instrument>
        // let _ = any
        // .downcast_mut::<SharedPtr<ToPlunderInstrument<String, Note, I, SharedPlunderInstrument>>>()
        // .unwrap()
        // .write()
        // .unwrap()
        // .0
        // .transform(LuaValue::Nil);
        Self(synth)
    }

    pub fn parse(
        &self,
        pattern_str: &str,
        _lua: &Lua,
    ) -> LuaResult<Vec<(usize, EmittableUserData)>> {
        self.parse_notes(pattern_str.chars().collect::<Vec<_>>().as_slice())
            .map_err(LuaError::runtime)?
            .into_iter()
            .enumerate()
            .map(|(id, note)| {
                info!("generate next event with id: `{id}`");
                let instrument_and_event: InstrumentAndEvent<
                    SharedPlunderInstrument,
                    (Synth, String),
                    Note,
                    Note,
                    DownInstrumentUpEvent,
                > = InstrumentAndEvent::new(self.0.factory.clone(), note);
                trace!(
                    "instrument-and-event's help: `{}`",
                    instrument_and_event.instrument_help()
                );

                // let any: &dyn Any = &instrument_and_event.instrument.0;
                // warn!("type-id at parse: `{:?}`", any.type_id());
                Ok((
                    id,
                    EmittableUserData(Arc::new(RwLock::new(instrument_and_event))),
                ))
            })
            .collect::<LuaResult<Vec<_>>>()
    }

    fn parse_notes(&self, pattern_str: &[char]) -> Result<Vec<Note>, String> {
        let mut notes = Vec::new();
        let pattern_str = pattern_str.iter().copied().enumerate().collect::<Vec<_>>();

        for note_str in pattern_str.split(|(_, c)| c.is_whitespace()) {
            trace!(
                "got note-string: `{}`",
                note_str.iter().fold(String::new(), |mut a, (_, c)| {
                    a.push(*c);
                    a
                })
            );
            let Some(note) = Note::from_spanned_str(note_str)? else {
                continue;
            };
            trace!("parsed as: `{:?}`", note);
            notes.push(note);
        }
        Ok(notes)
    }
}

impl LuaUserData for MidiParser {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("parse", |lua, this: &Self, pattern: String| {
            let table = lua.create_table()?;
            this.parse(&pattern, lua)?.into_iter().try_for_each(
                |(id, event)| -> LuaResult<()> {
                    let elem = lua.create_table()?;
                    elem.push(id)?;
                    elem.push(event)?;
                    table.push(elem)
                },
            )?;
            Ok(table)
        });
    }
}
