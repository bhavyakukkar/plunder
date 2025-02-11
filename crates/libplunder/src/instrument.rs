use mlua::{prelude::*, serde::Deserializer};
use serde::de::DeserializeOwned;
use std::{
    fmt::{self, Display},
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use crate::{Sample, SharedPtr};

// pub trait IntoSource<'a> {
//     type S: Source;
//     type Err: Display;
//     fn into_source(&'a mut self, sample: Sample) -> Result<Self::S, Self::Err>;
// }

#[derive(Debug)]
pub enum SourceError<E> {
    Fatal(E),
    Once(E),
}

impl<E> SourceError<E> {
    pub(crate) fn to_string_error(self) -> SourceError<String>
    where
        E: Display,
    {
        match self {
            SourceError::Once(err) => SourceError::Once(err.to_string()),
            SourceError::Fatal(err) => SourceError::Once(err.to_string()),
        }
    }
}

pub trait Source {
    type Err: Display;
    fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<Self::Err>>;
}

pub trait State<A, E> {
    type TErr: Display;
    type IErr: Display;
    fn transform(&mut self, event: E) -> Result<(), Self::TErr>;
    fn initialize(route: &str, arguments: A) -> Result<Self, Self::IErr>
    where
        Self: Sized;
}

#[derive(Debug)]
pub enum InstrumentError {
    DeserializationError(LuaError),
    Custom(String),
}

impl Display for InstrumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstrumentError::DeserializationError(err) => {
                write!(f, "Error deserializing value into expected: {err}")
            }
            InstrumentError::Custom(err) => write!(f, "Error reported from instrument: {err}"),
        }
    }
}

pub(crate) trait PlunderInstrument: fmt::Debug {
    fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<String>>;
    fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError>;
    fn help(&self) -> String;
}

/// An Instrument is any pairing of a [sample-source](Source) and a [State machine](State)
pub trait Instrument<A, E>: State<A, E> + Source {
    fn help(&self) -> String;
}

pub struct ToPlunderInstrument<A, E, T>(T, PhantomData<(A, E)>);

impl<A, E, T> From<T> for ToPlunderInstrument<A, E, T> {
    fn from(value: T) -> Self {
        ToPlunderInstrument(value, PhantomData)
    }
}

impl<A, E, T> fmt::Debug for ToPlunderInstrument<A, E, T>
where
    E: DeserializeOwned,
    T: Instrument<A, E>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.help())
    }
}

impl<A, E, T> PlunderInstrument for ToPlunderInstrument<A, E, T>
where
    E: DeserializeOwned,
    T: Instrument<A, E>,
{
    fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<String>> {
        self.0.next_sample().map_err(|err| err.to_string_error())
    }

    fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError> {
        self.0
            .transform(
                E::deserialize(Deserializer::new(lua_value))
                    .map_err(InstrumentError::DeserializationError)?,
            )
            .map_err(|err| InstrumentError::Custom(err.to_string()))
    }

    fn help(&self) -> String {
        self.0.help()
    }
}

// impl<A, E, T> PlunderInstrument for ToPlunderInstrument<A, E, T>
// where
//     E: DeserializeOwned,
//     A: DeserializeOwned,
//     T: Instrument<A, E>,
// {
//     fn next_sample_8(&mut self) -> Result<Option<i8>, String> {
//         match (
//             // lossless
//             <T as Source<1>>::ENABLED,
//             // lossy
//             <T as Source<8>>::ENABLED,
//             <T as Source<4>>::ENABLED,
//             <T as Source<3>>::ENABLED,
//             <T as Source<2>>::ENABLED,
//         ) {
//             (true, _, _, _, _) => self.next_sample_lossless::<1, _>(),
//             (false, true, _, _, _) => self.next_sample_lossy::<8, _>(),
//             (false, false, true, _, _) => self.next_sample_lossy::<4, _>(),
//             (false, false, false, true, _) => self.next_sample_lossy::<3, _>(),
//             (false, false, false, false, true) => self.next_sample_lossy::<2, _>(),
//             (false, false, false, false, false) => Err("Instrument provides 0 sources".to_string()),
//         }
//     }

//     fn next_sample_16(&mut self) -> Result<Option<i16>, String> {
//         match (
//             // lossless
//             <T as Source<2>>::ENABLED,
//             // lossy
//             <T as Source<8>>::ENABLED,
//             <T as Source<4>>::ENABLED,
//             <T as Source<3>>::ENABLED,
//             <T as Source<1>>::ENABLED,
//         ) {
//             (true, _, _, _, _) => self.next_sample_lossless::<2, _>(),
//             (false, true, _, _, _) => self.next_sample_lossy::<8, _>(),
//             (false, false, true, _, _) => self.next_sample_lossy::<4, _>(),
//             (false, false, false, true, _) => self.next_sample_lossy::<3, _>(),
//             (false, false, false, false, true) => self.next_sample_lossy::<1, _>(),
//             (false, false, false, false, false) => Err("Instrument provides 0 sources".to_string()),
//         }
//     }

//     fn next_sample_24(&mut self) -> Result<Option<(i8, i8, i8)>, String> {
//         match (
//             // lossless
//             <T as Source<3>>::ENABLED,
//             // lossy
//             <T as Source<8>>::ENABLED,
//             <T as Source<4>>::ENABLED,
//             <T as Source<2>>::ENABLED,
//             <T as Source<1>>::ENABLED,
//         ) {
//             (true, _, _, _, _) => self.next_sample_lossless::<3, _>(),
//             (false, true, _, _, _) => self.next_sample_lossy::<8, _>(),
//             (false, false, true, _, _) => self.next_sample_lossy::<4, _>(),
//             (false, false, false, true, _) => self.next_sample_lossy::<2, _>(),
//             (false, false, false, false, true) => self.next_sample_lossy::<1, _>(),
//             (false, false, false, false, false) => Err("Instrument provides 0 sources".to_string()),
//         }
//     }

//     fn next_sample_32(&mut self) -> Result<Option<i32>, String> {
//         match (
//             // lossless
//             <T as Source<4>>::ENABLED,
//             // lossy
//             <T as Source<8>>::ENABLED,
//             <T as Source<3>>::ENABLED,
//             <T as Source<2>>::ENABLED,
//             <T as Source<1>>::ENABLED,
//         ) {
//             (true, _, _, _, _) => self.next_sample_lossless::<4, _>(),
//             (false, true, _, _, _) => self.next_sample_lossy::<8, _>(),
//             (false, false, true, _, _) => self.next_sample_lossy::<3, _>(),
//             (false, false, false, true, _) => self.next_sample_lossy::<2, _>(),
//             (false, false, false, false, true) => self.next_sample_lossy::<1, _>(),
//             (false, false, false, false, false) => Err("Instrument provides 0 sources".to_string()),
//         }
//     }

//     fn next_sample_64(&mut self) -> Result<Option<i64>, String> {
//         match (
//             // lossless
//             <T as Source<8>>::ENABLED,
//             // lossy
//             <T as Source<4>>::ENABLED,
//             <T as Source<3>>::ENABLED,
//             <T as Source<2>>::ENABLED,
//             <T as Source<1>>::ENABLED,
//         ) {
//             (true, _, _, _, _) => self.next_sample_lossless::<8, _>(),
//             (false, true, _, _, _) => self.next_sample_lossy::<4, _>(),
//             (false, false, true, _, _) => self.next_sample_lossy::<3, _>(),
//             (false, false, false, true, _) => self.next_sample_lossy::<2, _>(),
//             (false, false, false, false, true) => self.next_sample_lossy::<1, _>(),
//             (false, false, false, false, false) => Err("Instrument provides 0 sources".to_string()),
//         }
//     }

//     fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError> {
//         self.0
//             .transform(
//                 E::deserialize(Deserializer::new(lua_value))
//                     .map_err(InstrumentError::DeserializationError)?,
//             )
//             .map_err(|err| InstrumentError::Custom(err.to_string()))
//     }

//     fn help(&mut self) -> String {
//         self.0.help()
//     }
// }

// impl PlunderInstrument for Box<dyn PlunderInstrument> {
//     fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<String>> {
//         self.deref_mut().next_sample()
//     }

//     fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError> {
//         self.deref_mut().transform(lua_value)
//     }

//     fn help(&mut self) -> String {
//         self.deref_mut().help()
//     }
// }

// Discuss whether to have this be in rust or just a transparent table in lua
// pub type InstrumentAndEvent<const S: usize, E, T> = (SharedPtr<T>, E, PhantomData<>);
#[derive(Debug)]
pub(crate) struct InstrumentAndEvent {
    instrument: SharedPtr<dyn PlunderInstrument>,
    event: LuaValue,
}

impl InstrumentAndEvent {
    pub fn emit(&mut self) -> Result<(), String> {
        self.instrument
            .write()
            .map_err(|_| "concurrency error accessing PackagedInstrument".to_string())?
            .transform(self.event.clone())
            .map_err(|err| err.to_string())
    }

    fn help(&mut self) -> String {
        self.instrument.write().unwrap().help()
    }
}

#[derive(Debug, Clone)]
pub struct EmittableUserData(pub(crate) SharedPtr<InstrumentAndEvent>);
impl EmittableUserData {
    pub fn help(&self) -> String {
        self.0.write().unwrap().help()
    }
}

impl LuaUserData for EmittableUserData {}

// obj (instrument type from class erased)
#[derive(Debug, Clone)]
pub struct PackagedInstrument {
    pub(crate) factory: SharedPtr<dyn PlunderInstrument>,
    pub manual: Arc<str>,
}

impl Display for PackagedInstrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.factory.write().map_err(|_| fmt::Error)?.help()
        )
    }
}

// class
#[derive(Debug)]
pub struct PackagedInstrumentFactory<A, E, T> {
    manual: Arc<str>,
    t: PhantomData<(A, E, T)>,
}

impl<A, E, T> LuaUserData for PackagedInstrumentFactory<A, E, T>
where
    A: DeserializeOwned + 'static,
    E: DeserializeOwned + 'static,
    T: Instrument<A, E> + 'static,
{
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Index, |lua, this, key: LuaValue| {
            let manual = this.manual.clone();
            let initializer = lua.create_function(
                move |_, arguments: LuaValue| -> LuaResult<PackagedInstrument> {
                    match T::initialize(
                        &key.as_string_lossy().ok_or(LuaError::runtime(
                            "Instruments can only be initialized using valid strings",
                        ))?,
                        A::deserialize(Deserializer::new(arguments))
                            .map_err(|err| LuaError::runtime(format!("\n~~~> plunder <~~~: Error while providing this value to the instrument: {err}")))?,
                    ) {
                        Ok(instrument) => Ok(PackagedInstrument {
                            factory: Arc::new(RwLock::new(ToPlunderInstrument::from(instrument))),
                            manual: manual.clone(),
                        }),
                        Err(err) => Err(LuaError::runtime(format!("\n{err:#}"))),
                    }
                },
            )?;
            Ok(LuaValue::Function(initializer))
        });

        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, _: LuaMultiValue| {
            Ok(this.manual.to_string())
        });
    }
}

impl LuaUserData for PackagedInstrument {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            LuaMetaMethod::Index,
            |_, this, event: LuaValue| -> LuaResult<EmittableUserData> {
                Ok(EmittableUserData(Arc::new(RwLock::new(
                    InstrumentAndEvent {
                        instrument: this.factory.clone(),
                        event,
                    },
                ))))
            },
        );
        // methods.add_meta_method(LuaMetaMethod::Call, |_, this, event: LuaValue| {
        //     Ok(InstrumentAndEvent {
        //         instrument: this.factory.clone(),
        //         event,
        //     })
        // });
    }
}

pub fn package_instrument<T, A, E>(lua: &Lua, manual: String) -> LuaResult<LuaValue>
where
    A: DeserializeOwned + 'static,
    E: DeserializeOwned + 'static,
    T: Instrument<A, E> + 'static,
{
    PackagedInstrumentFactory::<A, E, T> {
        manual: Arc::from(manual),
        t: PhantomData,
    }
    .into_lua(lua)
}

/*
mod lossless_casts {
    use crate::Sample;

    pub trait LosslessCast<const S: usize> {
        fn cast(input: Sample<S>) -> Self;
    }

    impl LosslessCast<1> for i8 {
        fn cast(input: Sample<1>) -> i8 {
            input.0[0] as i8
        }
    }

    impl LosslessCast<2> for i16 {
        fn cast(input: Sample<2>) -> i16 {
            i16::from_be_bytes(input.0)
        }
    }

    impl LosslessCast<3> for (i8, i8, i8) {
        fn cast(input: Sample<3>) -> (i8, i8, i8) {
            (input.0[0] as i8, input.0[1] as i8, input.0[2] as i8)
        }
    }

    impl LosslessCast<4> for i32 {
        fn cast(_input: Sample<4>) -> i32 {
            todo!()
        }
    }

    impl LosslessCast<8> for i64 {
        fn cast(_input: Sample<8>) -> Self {
            todo!()
        }
    }
}

mod lossy_casts {
    use crate::Sample;

    pub trait LossyCast<const S: usize> {
        fn cast(input: Sample<S>) -> Self;
    }

    // to 8-bit
    impl LossyCast<2> for i8 {
        fn cast(_input: Sample<2>) -> i8 {
            todo!("downsample")
        }
    }
    impl LossyCast<3> for i8 {
        fn cast(_input: Sample<3>) -> i8 {
            todo!("downsample")
        }
    }
    impl LossyCast<4> for i8 {
        fn cast(_input: Sample<4>) -> i8 {
            todo!("downsample")
        }
    }
    impl LossyCast<8> for i8 {
        fn cast(_input: Sample<8>) -> i8 {
            todo!("downsample")
        }
    }

    // to 16-bit
    impl LossyCast<1> for i16 {
        fn cast(_input: Sample<1>) -> i16 {
            todo!("upsample")
        }
    }
    impl LossyCast<3> for i16 {
        fn cast(_input: Sample<3>) -> i16 {
            todo!("downsample")
        }
    }
    impl LossyCast<4> for i16 {
        fn cast(_input: Sample<4>) -> i16 {
            todo!("downsample")
        }
    }
    impl LossyCast<8> for i16 {
        fn cast(_input: Sample<8>) -> i16 {
            todo!("downsample")
        }
    }

    // to 24-bit
    impl LossyCast<1> for (i8, i8, i8) {
        fn cast(_input: Sample<1>) -> (i8, i8, i8) {
            todo!("upsample")
        }
    }
    impl LossyCast<2> for (i8, i8, i8) {
        fn cast(_input: Sample<2>) -> (i8, i8, i8) {
            todo!("downsample")
        }
    }
    impl LossyCast<4> for (i8, i8, i8) {
        fn cast(_input: Sample<4>) -> (i8, i8, i8) {
            todo!("downsample")
        }
    }
    impl LossyCast<8> for (i8, i8, i8) {
        fn cast(_input: Sample<8>) -> (i8, i8, i8) {
            todo!("downsample")
        }
    }

    // to 32-bit
    impl LossyCast<1> for i32 {
        fn cast(_input: Sample<1>) -> i32 {
            todo!("downsample")
        }
    }
    impl LossyCast<2> for i32 {
        fn cast(_input: Sample<2>) -> i32 {
            todo!("downsample")
        }
    }
    impl LossyCast<3> for i32 {
        fn cast(_input: Sample<3>) -> i32 {
            todo!("downsample")
        }
    }
    impl LossyCast<8> for i32 {
        fn cast(_input: Sample<8>) -> i32 {
            todo!("downsample")
        }
    }

    // to 64-bit
    impl LossyCast<1> for i64 {
        fn cast(_input: Sample<1>) -> i64 {
            todo!("downsample")
        }
    }
    impl LossyCast<2> for i64 {
        fn cast(_input: Sample<2>) -> i64 {
            todo!("downsample")
        }
    }
    impl LossyCast<3> for i64 {
        fn cast(_input: Sample<3>) -> i64 {
            todo!("downsample")
        }
    }
    impl LossyCast<4> for i64 {
        fn cast(_input: Sample<4>) -> i64 {
            todo!("downsample")
        }
    }
}*/
