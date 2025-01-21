
use crate::{instrument::Emittable, types::GridIndex};

pub type ParseOutput = Vec<(GridIndex, Emittable)>;

/// A parser converts a [`mlua::Value`] into a sequence of emittable events
pub trait Parser {
    fn extend(&mut self, argument: mlua::Value, lua: &mlua::Lua) -> mlua::Result<()>;
    fn parse(&mut self, pattern_str: &str, lua: &mlua::Lua) -> mlua::Result<ParseOutput>;
    fn help(&mut self) -> String;
}
