use std::{
    cmp::Ordering,
    iter::{Map, Peekable},
};

use itertools::Itertools;
use libplunder::prelude::instrument::*;
use log::{info, warn};
use midi::{MidiParser, Synth};
use mlua::prelude::*;
use parser1::Parser;
use render::EventStreamPair;
use sampler::Sampler;

mod render;

#[mlua::lua_module]
fn libplunder(lua: &Lua) -> LuaResult<LuaTable> {
    env_logger::init();

    let exports = lua.create_table()?;
    exports.set(
        "Debug",
        lua.create_function(|lua, value| debug(lua, value))?,
    )?;

    exports.set("help", lua.create_function(help)?)?;

    exports.set("Sampler", Sampler::package(lua)?)?;

    exports.set("render", lua.create_function(render)?)?;

    exports.set("Parser", lua.create_function(|_, _: ()| Ok(Parser::new()))?)?;

    exports.set("Synth", Synth::package(lua)?)?;

    exports.set("Midi", lua.create_function(midi_parser)?)?;

    Ok(exports)
}

pub fn midi_parser(lua: &Lua, instrument: LuaValue) -> LuaResult<MidiParser> {
    if !instrument
        .as_userdata()
        .is_some_and(|userdata| userdata.is::<PackagedInstrument>())
    {
        warn!("passed value to Midi is not Userdata containing PackagedInstrument");
    }
    Ok(MidiParser::new(
        LuaUserDataRef::<PackagedInstrument>::from_lua(instrument, lua)?.clone(),
    ))
}

pub fn debug(lua: &Lua, value: LuaValue) -> LuaResult<String> {
    use std::fmt::Write;
    let mut s = String::new();

    fn debug_rec(
        lua: &Lua,
        out: &mut String,
        value: LuaValue,
        inline: bool,
        indent: usize,
    ) -> std::fmt::Result {
        if indent == 10 {
            return out.write_str("<...>");
        }

        let indentify = |n| " ".repeat(4).repeat(n);
        match value {
            u @ LuaValue::UserData(_) => Ok(
                match LuaUserDataRefMut::<Box<dyn std::fmt::Debug>>::from_lua(u, lua) {
                    Ok(debug) => write!(out, "{debug:?}"),
                    Err(_) => write!(out, "<userdata> of unknown type"),
                }?,
            ),

            LuaValue::Table(ref t) => {
                out.write_str("<table>: {")?;
                t.pairs::<LuaValue, LuaValue>().try_for_each(|pair| {
                    let (key, value) =
                        pair.expect("since expecting LuaValue, this is not expected to fail");
                    if inline {
                        write!(out, " `")?;
                    } else {
                        write!(out, "\n{}`", indentify(indent + 1))?;
                    }
                    debug_rec(lua, out, key, indent >= 1, indent + 1)?;
                    write!(out, "`: `")?;
                    debug_rec(lua, out, value, indent >= 1, indent + 1)?;
                    write!(out, "`{sep2}", sep2 = if inline { ", " } else { "," })?;
                    Ok(())
                })?;

                if inline {
                    write!(out, " }}")
                } else {
                    write!(out, "\n{}}}", indentify(indent))
                }
            }

            v => match v.to_string() {
                Ok(ref s) => out.write_str(s),
                Err(_) => out.write_str(""),
            },
        }
    }
    debug_rec(lua, &mut s, value, false, 0).map_err(LuaError::runtime)?;
    println!("DEBUG :: `{}`", s);
    Ok(s)
}

/// An iterator that takes a collection of iterators assumed to be sorted and a fallible function to
/// order their items, and yields another iterator that returns the items from the two iterators sorted **in
/// ascending-order**
pub struct SortIterator<I, E>
where
    I: Iterator,
{
    its: Vec<Peekable<I>>,
    // a: Peekable<I>,
    // b: Peekable<I>,
    cmp: fn(&I::Item, &I::Item) -> Result<Ordering, E>,
}

impl<I, E> SortIterator<I, E>
where
    I: Iterator,
{
    pub fn new(its: Vec<I>, cmp: fn(&I::Item, &I::Item) -> Result<Ordering, E>) -> Self {
        let its = its.into_iter().map(I::peekable).collect();
        SortIterator { its, cmp }
    }
}

impl<I, E> Iterator for SortIterator<I, E>
where
    I: Iterator,
{
    type Item = Result<I::Item, E>;

    fn next(&mut self) -> Option<Result<I::Item, E>> {
        let mut next_id = None;
        {
            let mut next_elem = None;
            for (it_id, it) in self.its.iter_mut().enumerate() {
                (next_id, next_elem) = match (next_id, next_elem, it.peek()) {
                    (None, None, None) => (None, None),
                    (None, None, Some(new)) => (Some(it_id), Some(new)),
                    (Some(i), Some(next), None) => (Some(i), Some(next)),
                    (Some(i), Some(next), Some(new)) => match (self.cmp)(next, new) {
                        Ok(Ordering::Less | Ordering::Equal) => (Some(i), Some(next)),
                        Ok(Ordering::Greater) => (Some(it_id), Some(new)),
                        Err(e) => {
                            return Some(Err(e));
                        }
                    },
                    _ => unreachable!(),
                }
            }
        }
        next_id
            .and_then(|i| self.its.get_mut(i).unwrap().next())
            .map(|next| Ok(next))
    }
}

pub struct LuaIterator {
    iter: LuaFunction,
    obj: LuaValue,
    i: Option<LuaValue>,
}

impl From<(LuaFunction, LuaValue, LuaValue)> for LuaIterator {
    fn from(value: (LuaFunction, LuaValue, LuaValue)) -> Self {
        LuaIterator {
            iter: value.0,
            obj: value.1,
            i: Some(value.2),
        }
    }
}

impl Iterator for LuaIterator {
    type Item = LuaResult<LuaValue>;

    fn next(&mut self) -> Option<LuaResult<LuaValue>> {
        if let Some(i) = &self.i {
            let (next, out): (LuaValue, LuaValue) = match self.iter.call((&self.obj, i)) {
                Ok(value) => value,
                Err(err) => {
                    return Some(Err(err));
                }
            };
            self.i = Some(next);
            if matches!(out, LuaNil) {
                self.i = None;
                None
            } else {
                Some(Ok(out))
            }
        } else {
            None
        }
    }
}

// type SortIteratorAccumulator<I> = Result<SortIterator<I>, Option<I>>;
// fn empty_sort_iterator_accumulator<I>() -> SortIteratorAccumulator<I>
// where
//     I: Iterator,
// {
//     Err(None)
// }

pub fn render(
    _lua: &Lua,
    (path, instruments, bitrate, interval, sample_bound, event_streams): (
        String,
        Vec<LuaUserDataRef<PackagedInstrument>>,
        u32,
        usize,
        usize,
        LuaTable,
        // (LuaFunction, LuaValue, LuaValue),
    ),
) -> LuaResult<()> {
    use std::ops::Deref;

    // Collection of event-streams, each of which yields LuaResult<(usize, EmittableUserData)>
    let valid_event_streams: Vec<_> = event_streams
        .pairs::<LuaValue, LuaTable>()
        .map(|pair| -> LuaResult<_> {
            let (_name, event_stream) = pair?;
            let event_stream_fun: LuaFunction = event_stream.get(1)?;
            let event_stream_obj: LuaValue = event_stream.get(2)?;
            let event_stream_init: LuaValue = event_stream.get(3)?;
            Ok(
                LuaIterator::from((event_stream_fun, event_stream_obj, event_stream_init)).map(
                    |item| -> LuaResult<EventStreamPair> {
                        let item = item?;
                        let table = item.as_table().ok_or(LuaError::runtime(
                            "expected event-stream to be an iterator \
                            that yields events, which are tables",
                        ))?;
                        let idx = table.get(1)?;
                        let event: LuaUserDataRef<EmittableUserData> = table.get(2)?;
                        info!("Popped next even in sorted event stream with id: `{idx}`");
                        Ok((idx, event.deref().clone()))
                    },
                ),
            )
        })
        .collect::<Result<Vec<Map<_, _>>, _>>()?;

    SortIterator::new(valid_event_streams, |event_a, event_b| -> LuaResult<_> {
        Ok(event_a
            .as_ref()
            .map_err(LuaError::clone)?
            .0
            .cmp(&event_b.as_ref().map_err(LuaError::clone)?.0))
    })
    .map(|next| match next {
        Ok(next) => next,
        Err(e) => Err(e),
    })
    .process_results(|sorted_event_stream| {
        render::render_single_event_stream(
            path,
            instruments,
            sorted_event_stream,
            bitrate,
            interval,
            sample_bound,
        )
    })
}

pub fn help(lua: &Lua, value: LuaValue) -> LuaResult<()> {
    // Packaged Instrument
    if let Ok(packaged_instrument) =
        LuaUserDataRefMut::<PackagedInstrument>::from_lua(value.clone(), lua)
    {
        println!("Instrument: {}", packaged_instrument);
        println!("Manual: {}", packaged_instrument.manual);
    }
    // Instrument & Event
    else if let Ok(instrument_and_event) =
        LuaUserDataRefMut::<EmittableUserData>::from_lua(value.clone(), lua)
    {
        println!("Event from Instrument: {}", (*instrument_and_event).help());
    }
    // TODO Packaged Parser
    else if false {
    }
    // TODO Packaged Parser Factory
    else if false {
    }
    // Any other value (includes Packaged Instrument Factory)
    else {
        println!("Value: {}", value.to_string()?);
    }
    Ok(())
}
