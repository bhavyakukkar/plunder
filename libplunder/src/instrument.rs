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
