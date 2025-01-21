use mlua::prelude::*;
// use mlua::{FromLua, IntoLua, MetaMethod, UserData, Value};
use std::sync::{Arc, RwLock};

use crate::{
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
