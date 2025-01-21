use crate::{
    instrument::{BasicInstrument, Emittable, Source},
    types::{GridIndex, SampleDepth},
};
use std::{
    collections::HashSet,
    hash::Hash,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

struct ArcPtr<T: ?Sized>(Arc<T>);

impl<T: ?Sized> std::cmp::PartialEq for ArcPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        // TODO test in playground
        std::ptr::addr_eq(Arc::as_ptr(&self.0), Arc::as_ptr(&other.0))
    }
}

impl<T: ?Sized> Eq for ArcPtr<T> {}

impl<T: ?Sized> Hash for ArcPtr<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state)
    }
}

pub struct Engine<S> {
    active_instruments: HashSet<ArcPtr<RwLock<dyn BasicInstrument>>>,
    // lua: &'a mlua::Lua,
    s: PhantomData<S>,
}

impl<S> Engine<S>
where
    (dyn BasicInstrument + 'static): Source<S>,
{
    pub fn new(/*lua: &'a mlua::Lua*/) -> Self {
        Self {
            active_instruments: HashSet::new(),
            // lua,
            s: PhantomData,
        }
    }

    fn tick(&mut self, elem: &(GridIndex, Emittable), lua: &mlua::Lua) -> Result<(), anyhow::Error>
    where
        S: SampleDepth,
    {
        // transition the instrument using the event
        let instrument = match self
            .active_instruments
            .get(&ArcPtr(elem.1.instrument.clone()))
        {
            // if exists, get mutable reference to instrument
            Some(_) => elem.1.instrument.clone(),

            // if doesn't exist, insert and return mutable reference to instrument
            None => {
                self.active_instruments
                    .insert(ArcPtr(elem.1.instrument.clone()));
                elem.1.instrument.clone()
            }
        };

        let mut instrument = instrument.write().map_err(|err| {
            anyhow::anyhow!("concurrency error transitioning the instrument: {}", err)
        })?;
        // .transition(&elem.1.event, self.lua)
        instrument
            .transition(elem.1.event.clone(), lua)
            .map_err(|err| anyhow::anyhow!("transition error: {err}"))
        // Ok(())
    }

    pub fn render(
        mut self,
        lua: &mlua::Lua,
        event_list: &[&[(GridIndex, Emittable)]],
        len: GridIndex,
        interval: usize,
    ) -> Result<Vec<S>, anyhow::Error>
    where
        S: SampleDepth,
    {
        let mut samples = Vec::with_capacity(len * interval);
        let num_playlists = event_list.len();
        if num_playlists == 0 {
            return Ok(Vec::new());
        }
        let mut heads = vec![0; num_playlists];
        for j in 0..len {
            for i in 0..num_playlists {
                match event_list[i].get(heads[i]) {
                    Some(elem) => {
                        // println!("inserted 1 event at {j}");
                        if j == elem.0 {
                            self.tick(elem, lua)?;
                            heads[i] += 1;
                        }
                    }
                    None => (),
                }
            }

            // let mut num_instruments = 0;
            for _ in 0..interval {
                let mut sample = S::MID;
                // print!("(");
                for instrument in &self.active_instruments {
                    // num_instruments += 1;
                    sample += instrument
                        .0
                        .write()
                        .map_err(|err| {
                            anyhow::anyhow!("concurrency error accessing instrument: {}", err)
                        })?
                        .next()
                        .transpose()?
                        .unwrap_or(S::MID);
                    // print!("sample:{sample:?}, adding:{adding:?}\t", adding = adding);
                }
                // println!(")");
                samples.push(sample);
            }
        }

        Ok(samples)
    }
}
