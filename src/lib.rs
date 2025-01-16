use std::ffi::{c_char, c_int, CStr};
use std::sync::{Arc, RwLock};
use std::thread;

use instruments::wav_instrument::Wav;
use instruments::InstrumentUserData;
use mlua::{Lua, ObjectLike, Table, UserData, UserDataRefMut, Value};
use parse::default_parser::DefaultParser;
use parse::ParserUserData;

// // Lua external imports
// #[link(name = "lua5.4")]
// extern "C" {
//     fn lua_pushcclosure(
//         L: *mut lua_State,
//         r#fn: unsafe extern "C" fn(*mut lua_State) -> c_int,
//         n: c_int,
//     );
//     // fn lua_setglobal(L: *mut lua_State, name: *const c_char);
//     // fn lua_setlocal(L: *mut lua_State, at: *const lua_Debug, n: c_int) -> *const c_char;
// }
// #[repr(C)]
// #[allow(non_camel_case_types)]
// pub struct lua_State {
//     _data: [u8; 0],
//     _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
// }
// #[repr(C)]
// #[allow(non_camel_case_types)]
// pub struct lua_Debug {
//     _data: [u8; 0],
//     _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
// }

// // Lua library entrypoint
// #[allow(non_snake_case)]
// #[no_mangle]
// pub unsafe extern "C" fn luaopen_libplunder(L: *mut lua_State) -> c_int {
//     lua_pushcclosure(L, play_audio, 0);
//     // lua_setglobal(L, c"dir".as_ptr());
//     return 1;
// }

// // Lua exports
// #[allow(non_snake_case)]
// #[no_mangle]
// pub unsafe extern "C" fn play_audio(_ctx: *mut lua_State) -> c_int {
//     // TODO take path as string from ctx stack
//     thread::spawn(|| rust::play_audio("/home/bhavya/music/mine/bt.wav"));
//     return 1;
// }

// fn sum(_: &Lua, (a, b): (i64, i64)) -> LuaResult<i64> {
//     Ok(a + b)
// }

#[mlua::lua_module]
fn libplunder(lua: &Lua) -> mlua::Result<Table> {
    use mlua::{FromLua, ObjectLike};

    let exports = lua.create_table()?;
    exports.set(
        "playaudio",
        lua.create_function(|_, path: String| Ok(rust::play_audio(&path)))?,
    )?;
    exports.set(
        "NewDefaultParser",
        lua.create_function(|_, _: ()| Ok(ParserUserData(DefaultParser(None))))?,
    )?;
    exports.set(
        "LoadWav",
        lua.create_function(|_, path: String| {
            Wav::<i16>::load(path)
                .map_err(|err| mlua::Error::runtime(err.to_string()))
                .map(|wav_reader| InstrumentUserData(Arc::new(RwLock::new(wav_reader))))
        })?,
    )?;
    exports.set(
        "Debug",
        lua.create_function(|lua, value: Value| match value {
            u @ Value::UserData(_) => UserDataRefMut::<Box<dyn std::fmt::Debug>>::from_lua(u, lua)
                .map(|debug| format!("{debug:?}")),
            v => v.to_string(),
        })?,
    );
    Ok(exports)
}

pub mod types;

pub mod instruments;

pub mod filters;

pub mod parse;

// pub mod engine;

mod utils;

#[no_mangle]
pub unsafe fn play_audio(path: *const c_char) {
    rust::play_audio(CStr::from_ptr(path).to_str().unwrap());
}

mod rust {
    use std::path::Path;

    pub fn play_audio<P>(path: P)
    where
        P: AsRef<Path>,
    {
        use web_audio_api::context::{
            AudioContext, AudioContextLatencyCategory, AudioContextOptions, BaseAudioContext,
        };
        use web_audio_api::node::{AudioNode, AudioScheduledSourceNode};

        // set up the audio context with optimized settings for your hardware
        #[cfg(not(target_os = "linux"))]
        let context = AudioContext::default();
        #[cfg(target_os = "linux")]
        let context = AudioContext::new(AudioContextOptions {
            latency_hint: AudioContextLatencyCategory::Playback,
            ..AudioContextOptions::default()
        });

        // for background music, read from local file
        let file = std::fs::File::open(path).unwrap();
        let mut buffer = context.decode_audio_data_sync(file).unwrap();
        let dur = buffer.duration();
        for channel_number in 0..buffer.number_of_channels() {
            buffer.get_channel_data_mut(channel_number).reverse();
        }
        println!("Audio Duration: {}", dur);

        // setup an AudioBufferSourceNode
        let mut src = context.create_buffer_source();
        src.set_buffer(buffer);
        src.set_loop(true);

        // create a biquad filter
        // let biquad = context.create_biquad_filter();
        // biquad.frequency().set_value(250.);

        // connect the audio nodes
        // biquad.connect(&context.destination());
        // src.connect(&biquad);
        src.connect(&context.destination());

        // play the buffer
        src.start();

        // enjoy listening
        std::thread::sleep(std::time::Duration::from_secs(6 * 60 + 40));
    }
    pub fn _render() {}
}
