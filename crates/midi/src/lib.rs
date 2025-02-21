use libplunder::instrument::package_instrument;

mod instrument;
use instrument::Note;
pub use instrument::Synth;
impl Synth {
    pub fn package(lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        package_instrument::<Self, String, Note>(lua, "synth".to_string())
    }
}

mod parser;
pub use parser::MidiParser;
