use std::{
    any::{self, Any},
    fmt::{self, Display},
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use mlua::{prelude::*, serde::Deserializer};
use serde::de::DeserializeOwned;

use crate::{
    instrument_and_event::{DownInstrumentDownEvent, InstrumentAndEvent},
    Sample, SharedPtr,
};

// fn magic<T, A, E>(before: SharedPtr<T>) -> SharedPtr<dyn PlunderInstrument>
// where
//     T: Instrument<A, E> + 'static,
//     E: DeserializeOwned + 'static,
//     A: 'static,
// {
//     Arc::new(RwLock::new(ToPlunderInstrument::from(
//         before.into_inner().unwrap(),
//     )))
// }

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

impl<E> fmt::Display for SourceError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Fatal(err) => write!(f, "once emit error: {err}"),
            SourceError::Once(err) => write!(f, "once emit error: {err}"),
        }
    }
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

// TODO: it is ok to impl Source for SharedPtr<T: Source>, but change the Err assoc-type to a union that includes error that occurred is rwlock poisoned
// impl Source for SharePtr of Source
// impl<T> Source for SharedPtr<T>
// where
//     T: Source,
// {
//     type Err = T::Err;

//     fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<Self::Err>> {
//         self.write().unwrap().next_sample()
//     }
// }

// impl Source for RwLock of Source
// impl<T> Source for RwLock<T>
// where
//     T: Source,
// {
//     type Err = T::Err;
//     fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<Self::Err>> {
//         self.write().unwrap().next_sample()
//     }
// }

pub trait State<A, E> {
    type TErr: Display;
    type IErr: Display;
    fn transform(&mut self, event: E) -> Result<(), Self::TErr>;
    fn initialize(route: &str, arguments: A) -> Result<Self, Self::IErr>
    where
        Self: Sized;
}

// impl State for SharePtr of State
// impl<T, A, E> State<A, E> for SharedPtr<T>
// where
//     T: State<A, E>,
// {
//     type TErr = T::TErr;
//     type IErr = T::IErr;

//     fn transform(&mut self, event: E) -> Result<(), Self::TErr> {
//         self.write().unwrap().transform(event)
//     }

//     fn initialize(route: &str, arguments: A) -> Result<Self, Self::IErr>
//     where
//         Self: Sized,
//     {
//         T::initialize(route, arguments).map(|state| Arc::new(RwLock::new(state)))
//     }
// }

// impl State for RwLock of State
// impl<T, A, E> State<A, E> for RwLock<T>
// where
//     T: State<A, E>,
// {
//     type TErr = T::TErr;
//     type IErr = T::IErr;

//     fn transform(&mut self, event: E) -> Result<(), Self::TErr> {
//         self.write().unwrap().transform(event)
//     }

//     fn initialize(route: &str, arguments: A) -> Result<Self, Self::IErr>
//     where
//         Self: Sized,
//     {
//         T::initialize(route, arguments).map(|state| RwLock::new(state))
//     }
// }

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

/// A hidden trait for Instruments that is less fancier than [`Instrument`](Instrument) and is dyn-compatible
///
/// NOTE: PlunderInstrument should only be implemented for shared-pointers to Instruments
/// This makes it less tedious to downcast as the origin instrument in [`InstrumentAndEvent`](InstrumentAndEvent)
pub trait PlunderInstrument: fmt::Debug + Any + Sync + Send {
    fn next_sample(&self) -> Result<Option<Sample>, SourceError<String>>;
    fn transform(&self, lua_value: LuaValue) -> Result<(), InstrumentError>;
    fn help(&self) -> String;
}

/// An Instrument is any pairing of a [sample-source](Source) and a [state-machine](State)
pub trait Instrument<A, E>: State<A, E> + Source {
    fn help(&self) -> String;
}

// impl Instrument for SharedPtr of Instrument
// impl<T, A, E> Instrument<A, E> for SharedPtr<T>
// where
//     T: Instrument<A, E>,
// {
//     fn help(&self) -> String {
//         self.read().unwrap().help()
//     }
// }

// impl Instrument for RwLock of Instrument
// impl<T, A, E> Instrument<A, E> for RwLock<T>
// where
//     T: Instrument<A, E>,
// {
//     fn help(&self) -> String {
//         self.read().unwrap().help()
//     }
// }

/// A wrapper for an [`Instrument`](Instrument) to make it dyn-compatible
pub struct ToPlunderInstrument<A, E, T> {
    pub(crate) instrument: SharedPtr<T>,
    p: PhantomData<(A, E)>,
}

impl<A, E, T> From<SharedPtr<T>> for ToPlunderInstrument<A, E, T> {
    fn from(instrument: SharedPtr<T>) -> Self {
        ToPlunderInstrument {
            instrument,
            p: PhantomData,
        }
    }
}

impl<A, E, T> fmt::Debug for ToPlunderInstrument<A, E, T>
where
    E: DeserializeOwned,
    T: Instrument<A, E>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // write!(f, "{}", self.instrument.help())
        write!(f, "ToPlunderInstrument( {} )", any::type_name::<T>())
    }
}

// impl<A, E, T> PlunderInstrument for ToPlunderInstrument<A, E, T, InstrumentMarker>
// where
//     E: DeserializeOwned,
//     T: Instrument<A, E>,
// {
//     fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<String>> {
//         self.instrument
//             .next_sample()
//             .map_err(|err| err.to_string_error())
//     }

//     fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError> {
//         self.instrument
//             .transform(
//                 E::deserialize(Deserializer::new(lua_value))
//                     .map_err(InstrumentError::DeserializationError)?,
//             )
//             .map_err(|err| InstrumentError::Custom(err.to_string()))
//     }

//     fn help(&self) -> String {
//         self.instrument.help()
//     }
// }

impl<A, E, T> PlunderInstrument for ToPlunderInstrument<A, E, T>
where
    A: Send + Sync + 'static,
    E: DeserializeOwned + Send + Sync + 'static,
    T: Instrument<A, E> + Send + Sync + 'static,
{
    fn next_sample(&self) -> Result<Option<Sample>, SourceError<String>> {
        self.instrument
            .write()
            .unwrap()
            .next_sample()
            .map_err(|err| err.to_string_error())
    }

    fn transform(&self, lua_value: LuaValue) -> Result<(), InstrumentError> {
        self.instrument
            .write()
            .unwrap()
            .transform(
                E::deserialize(Deserializer::new(lua_value))
                    .map_err(InstrumentError::DeserializationError)?,
            )
            .map_err(|err| InstrumentError::Custom(err.to_string()))
    }

    fn help(&self) -> String {
        self.instrument.read().unwrap().help()
    }
}

// impl PlunderInstrument for SharedPtr<dyn PlunderInstrument> {
//     fn next_sample(&mut self) -> Result<Option<Sample>, SourceError<String>> {
//         self.write().unwrap().next_sample()
//     }

//     fn transform(&mut self, lua_value: LuaValue) -> Result<(), InstrumentError> {
//         self.write().unwrap().transform(lua_value)
//     }

//     fn help(&self) -> String {
//         self.write().unwrap().help()
//     }
// }

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

pub trait Emit {
    fn emit(&mut self) -> Result<(), String>;
    fn instrument_help(&self) -> String;
}

/*
impl<T, A, E> Emit for InstrumentAndEvent2<SharedPtr<T>, A, E, E>
where
    E: Clone,
    T: Instrument<A, E>,
{
    type Err = T::TErr;

    fn emit(&mut self) -> Option<T::TErr> {
        self.instrument
            .write()
            .unwrap()
            .transform(self.event.clone())
            .err()
    }
}

impl<T, A, E> Emit for InstrumentAndEvent2<SharedPtr<T>, A, E, LuaValue>
where
    T: Instrument<A, E>,
{
    type Err = T::TErr;

    fn emit(&mut self) -> Option<T::TErr> {
        self.0.write().unwrap().transform(self.1.clone()).err()
    }
}*/
// Discuss whether to have this be in rust or just a transparent table in lua
// pub type InstrumentAndEvent<const S: usize, E, T> = (SharedPtr<T>, E, PhantomData<>);
// #[derive(Debug)]
// pub struct InstrumentAndEvent {
//     instrument: SharedPtr<dyn PlunderInstrument>,
//     event: LuaValue,
// }

type PlunderInstrumentAndEvent =
    InstrumentAndEvent<SharedPlunderInstrument, (), (), LuaValue, DownInstrumentDownEvent>;

// #[derive(Debug)]
// pub struct ToInstrumentAndEvent<'a, T, A, E> {
//     pub instrument: T,
//     pub event: E,
//     pub lua: &'a Lua,
//     p: PhantomData<A>,
// }

// impl<'a, T, A, E> ToInstrumentAndEvent<'a, T, A, E> {
//     pub fn new(instrument: T, event: E, lua: &'a Lua) -> Self {
//         ToInstrumentAndEvent {
//             instrument,
//             event,
//             lua,
//             p: PhantomData,
//         }
//     }
// }

// impl<'a, T, A, E> TryFrom<ToInstrumentAndEvent<'a, T, A, E>> for PlunderInstrumentAndEvent
// where
//     A: 'static,
//     E: serde::Serialize + DeserializeOwned + 'static,
//     T: Instrument<A, E> + 'static,
// {
//     type Error = LuaError;
//     fn try_from(value: ToInstrumentAndEvent<T, A, E>) -> Result<Self, Self::Error> {
//         Ok(PlunderInstrumentAndEvent {
//             instrument: Arc::new(RwLock::new(ToPlunderInstrument::from(value.instrument))),
//             event: value
//                 .event
//                 .serialize(mlua::serde::Serializer::new(value.lua))?,
//             p: PhantomData,
//         })
//     }
// }

/*pub struct InstrumentAndEventBuilder<T, A, E> {
    instrument: Option<SharedPtr<T>>,
    event: LuaValue,
    d: PhantomData<A>,
}

pub enum InstrumentAndEventBuilderError {
    NoInstrument,
    NoEvent,
}

impl<T, A, E> InstrumentAndEventBuilder<T, A, E> {
    fn new() -> Self {
        InstrumentAndEventBuilder {
            instrument: None,
            event: None,
            d: PhantomData,
        }
    }

    fn instrument(self, instrument: SharedPtr<T>) -> Self {
        InstrumentAndEventBuilder {
            instrument: Some(instrument),
            ..self
        }
    }

    fn event(self, event: E) -> Self {
        InstrumentAndEventBuilder {
            event: Some(event),
            ..self
        }
    }

    fn build(self) -> Result<InstrumentAndEvent, InstrumentAndEventBuilderError>
    where
        E: DeserializeOwned,
        T: Instrument<A, E>,
    {
        Ok(InstrumentAndEvent {
            instrument: self
                .instrument
                .ok_or(InstrumentAndEventBuilderError::NoInstrument)?,
            event: self.event.ok_or(InstrumentAndEventBuilderError::NoEvent)?,
        })
    }
}*/

// impl PlunderInstrumentAndEvent {
//     pub fn emit(&mut self) -> Result<(), String> {
//         // check if maybe self.event is actually UserData
//         // self.event.as_userdata().map(|ud| LuaUserDataRef<usize>)
//         self.instrument
//             .0
//             .write()
//             .map_err(|_| "concurrency error accessing PackagedInstrument".to_string())?
//             .transform(self.event.clone())
//             .map_err(|err| err.to_string())
//     }

//     fn help(&mut self) -> String {
//         self.instrument.0.write().unwrap().help()
//     }
// }

#[derive(Clone)]
pub struct EmittableUserData(pub SharedPtr<dyn Emit>);
impl EmittableUserData {
    pub fn help(&self) -> String {
        self.0.write().unwrap().instrument_help()
    }
}

impl fmt::Debug for EmittableUserData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.read().unwrap().instrument_help())
    }
}

impl<T> From<T> for EmittableUserData
where
    T: Emit + 'static,
{
    fn from(value: T) -> Self {
        EmittableUserData(Arc::new(RwLock::new(value)))
    }
}

// impl From<PlunderInstrumentAndEvent> for EmittableUserData {
//     fn from(value: PlunderInstrumentAndEvent) -> Self {
//         EmittableUserData(Arc::new(RwLock::new(value)))
//     }
// }

impl LuaUserData for EmittableUserData {}

#[derive(Debug, Clone)]
pub struct SharedPlunderInstrument(pub Arc<dyn PlunderInstrument + Send + Sync>);

// obj (instrument type from class erased)
#[derive(Debug, Clone)]
pub struct PackagedInstrument {
    pub factory: SharedPlunderInstrument,
    pub manual: Arc<str>,
}

impl Display for PackagedInstrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.factory.0.help())
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
    A: DeserializeOwned + Send + Sync + 'static,
    E: DeserializeOwned + Send + Sync + 'static,
    T: Instrument<A, E> + Send + Sync + 'static,
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
                        Ok(instrument) => {
                            let shared_instrument = Arc::new(ToPlunderInstrument::<A, E, T>::from(Arc::new(RwLock::new(instrument))));

                            Ok(PackagedInstrument {
                              factory: SharedPlunderInstrument(shared_instrument),
                               manual: manual.clone(),
                                                })
                        },
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

impl From<(&PackagedInstrument, LuaValue)> for EmittableUserData {
    fn from((instrument, event): (&PackagedInstrument, LuaValue)) -> Self {
        EmittableUserData(Arc::new(RwLock::new(PlunderInstrumentAndEvent {
            instrument: instrument.factory.clone(),
            event,
            p: PhantomData,
        })))
    }
}

impl LuaUserData for PackagedInstrument {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            LuaMetaMethod::Index,
            |_, this, event: LuaValue| -> LuaResult<EmittableUserData> { Ok((this, event).into()) },
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
    A: DeserializeOwned + Sync + Send + 'static,
    E: DeserializeOwned + Sync + Send + 'static,
    T: Instrument<A, E> + Sync + Send + 'static,
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
