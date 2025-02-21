use std::{any::Any, marker::PhantomData, sync::Arc};

use log::info;
use mlua::{prelude::*, serde::Deserializer};
use serde::de::DeserializeOwned;

use crate::instrument::{
    Emit, Instrument, InstrumentError, SharedPlunderInstrument, ToPlunderInstrument,
};

#[derive(Debug)]
pub struct DownInstrumentDownEvent;
#[derive(Debug)]
pub struct DownInstrumentUpEvent;
#[derive(Debug)]
pub struct UpInstrumentDownEvent;
#[derive(Debug)]
pub struct UpInstrumentUpEvent;

/// T is the type of the instrument being carried by this "InstrumentAndEvent"
/// V is the type of the event being carried by this "InstrumentAndEvent"
/// Incase T hasn't been downgraded/erased into a PlunderInstrument, it may be an `Instrument` with A and E
/// M is a marker being used to avoid conflicting implementation of Emit on various pairs of types of instruments and events in InstrumentAndEvent
#[derive(Debug)]
pub struct InstrumentAndEvent<T, A, E, V, M> {
    pub instrument: T,
    pub(super) event: V,
    pub(super) p: PhantomData<(A, E, M)>,
}

impl<T, A, E, V, M> InstrumentAndEvent<T, A, E, V, M> {
    pub fn new(instrument: T, event: V) -> Self {
        InstrumentAndEvent {
            instrument,
            event,
            p: PhantomData,
        }
    }
}

impl Emit
    for InstrumentAndEvent<SharedPlunderInstrument, (), (), LuaValue, DownInstrumentDownEvent>
{
    fn emit(&mut self) -> Result<(), String> {
        self.instrument
            .0
            .transform(self.event.clone())
            .map_err(|err| err.to_string())
    }

    fn instrument_help(&self) -> String {
        self.instrument.0.help()
    }
}

impl<I, A, E> Emit for InstrumentAndEvent<I, A, E, E, UpInstrumentUpEvent>
where
    E: Clone,
    I: Instrument<A, E>,
{
    fn emit(&mut self) -> Result<(), String> {
        self.instrument
            .transform(self.event.clone())
            .map_err(|err| err.to_string())
    }

    fn instrument_help(&self) -> String {
        self.instrument.help()
    }
}

impl<I, A, E> Emit
    for InstrumentAndEvent<SharedPlunderInstrument, (I, A), E, E, DownInstrumentUpEvent>
where
    A: Send + Sync + 'static,
    E: DeserializeOwned + Clone + Send + Sync + 'static,
    I: Instrument<A, E> + Send + Sync + 'static,
{
    fn emit(&mut self) -> Result<(), String> {
        let any: Arc<dyn Any + Send + Sync> = self.instrument.0.clone();

        let instrument = any
            .downcast::<ToPlunderInstrument<A, E, I>>()
            .map_err(|_| {
                "Instrument mismatch: Not the instrument corresponding to this plunder-instrument"
                    .to_string()
            })?;

        instrument
            .instrument
            .write()
            .unwrap()
            .transform(self.event.clone())
            .map_err(|err| err.to_string())?;

        info!("We Did It!");
        Ok(())
        // if any.is::<SharedPtr<ToPlunderInstrument<A, E, I, InstrumentMarker>>>() {
        //     warn!("success SharedPtr<dyn PlunderInstrument>");
        // } else {
        //     warn!("failed  SharedPtr<dyn PlunderInstrument>");
        // }

        /*
        any.downcast_mut::<SharedPtr<ToPlunderInstrument<A, E, I, InstrumentMarker>>>()
        .map(|shared_instrument| {
            info!("successfully cast SharedPlunderInstrument as SharedPtr<Instrument>");
            shared_instrument
                .write()
                .unwrap()
                .instrument
                .transform(self.event.clone())
        })
        .or(
            // else if can downcast as Instrument
            any.downcast_mut::<ToPlunderInstrument<A, E, I, InstrumentMarker>>()
                .map(|shared_instrument| {
                    info!("successfully cast SharedPlunderInstrument as Instrument");
                    shared_instrument.instrument.transform(self.event.clone())
                }),
        )
        // else fail
        .ok_or(
            "Instrument mismatch: Not the instrument corresponding to this plunder-instrument"
                .to_string(),
        )?
        .map_err(|err| err.to_string())
        */
    }

    fn instrument_help(&self) -> String {
        self.instrument.0.help()
    }
}

impl<I, A, E> Emit for InstrumentAndEvent<I, A, E, LuaValue, UpInstrumentDownEvent>
where
    E: DeserializeOwned + Clone,
    I: Instrument<A, E>,
{
    fn emit(&mut self) -> Result<(), String> {
        self.instrument
            .transform(
                E::deserialize(Deserializer::new(self.event.clone()))
                    .map_err(|err| InstrumentError::DeserializationError(err).to_string())?,
            )
            .map_err(|err| err.to_string())
    }

    fn instrument_help(&self) -> String {
        self.instrument.help()
    }
}
