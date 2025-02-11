use std::hash::{DefaultHasher, Hasher};

use anyhow::Context;
use libplunder::{combine_i32, prelude::instrument::*, Engine};
use mlua::prelude::*;
use parser1::Parser;
use sampler::Sampler;

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

    Ok(exports)
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

pub fn render<'a>(
    _lua: &'a Lua,
    (path, instruments, interval, sample_bound, event_stream): (
        String,
        Vec<LuaUserDataRef<PackagedInstrument>>,
        usize,
        usize,
        (LuaFunction, LuaValue, LuaValue),
    ),
) -> LuaResult<()> {
    use std::hash::Hash;
    use std::ops::Deref;

    let engine = Engine::new(
        instruments
            .iter()
            .map(|instrument| (*instrument).clone())
            .collect(),
        LuaIterator::from(event_stream).map(|item| {
            let item = item?;
            let table = item.as_table().ok_or(LuaError::runtime(
                "expected event-stream to be an iterator that yields events, which are tables",
            ))?;
            let idx = table.get(1)?;
            let event: LuaUserDataRef<EmittableUserData> = table.get(2)?;
            Ok((idx, event.deref().clone()))
        }),
        interval,
        sample_bound,
    );

    let mut hasher = DefaultHasher::new();
    let mut samples = engine.map(|i| match i {
        Ok(i) => match combine_i32(&i) {
            Ok(Some(s)) => {
                // for si in &s {
                //     print!("{si} ");
                // }
                // println!();
                s.hash(&mut hasher);
                Some(Ok(s))
            }
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        },
        Err(err) => Some(Err(anyhow::anyhow!("engine error: {err}"))),
    });

    let first_sample = samples
        .next()
        .unwrap()
        // We unwrap the Option that combine_i32 returns because we will have no information about
        // the number of channels if the first sample is empty
        .unwrap()
        .unwrap();

    let num_channels = first_sample.len();
    println!("num of channels: `{num_channels}`");
    let spec = hound::WavSpec {
        channels: num_channels as u16,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    first_sample
        .iter()
        .map(|s| writer.write_sample(*s))
        .collect::<Result<(), _>>()
        .unwrap();
    samples
        .map(|s| {
            s.transpose()?
                // because we received the number of channels from that first-sample, we can replace future empty samples with a collection of empty samples in each channel
                .unwrap_or(vec![0; num_channels])
                .iter()
                .map(|s| writer.write_sample(*s))
                .collect::<Result<(), _>>()
                .context("wav write error")
        })
        .collect::<Result<(), _>>()
        .unwrap();

    writer.finalize().unwrap();

    println!("hash: {}", hasher.finish());
    Ok(())
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
