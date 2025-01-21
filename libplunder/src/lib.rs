// 1. TODO units should be inferred by the engine instead of being parsed randomly or maybe it should explicitly be provided by the parser?
// 2. TODO experiment with using bits_per_sample(u16), sample_format(enum{Float, Int}) like hound instead of trait SampleDepth. we're still panicking from sample formats we don't expect

#![warn(missing_debug_implementations)]

mod engine;
mod instrument;
mod lua;
mod math;
mod parser;
mod player;
mod types;
mod utils;

pub mod prelude {
    pub use crate::{
        instrument::{BasicInstrument, Emittable, Source},
        lua::{InstrumentUserData, ParserUserData},
        parser::{ParseOutput, Parser},
        types::{GridIndex, SampleDepth},
    };

    pub use mlua::prelude::*;
}
