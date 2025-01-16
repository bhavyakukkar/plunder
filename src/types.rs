// pub(crate) trait SampleDepth {}

// impl SampleDepth for i8 {}
// impl SampleDepth for i16 {}
// impl SampleDepth for i32 {}
// impl SampleDepth for i64 {}
// impl SampleDepth for u8 {}
// impl SampleDepth for u16 {}
// impl SampleDepth for u32 {}
// impl SampleDepth for u64 {}
// impl SampleDepth for f32 {}
// impl SampleDepth for f64 {}

pub trait SampleDepth: hound::Sample {}
impl<T: hound::Sample> SampleDepth for T {}

// pub fn clone_emittable(boxed_emit: &Box<dyn Emittable>) -> Box<dyn Emittable> {
//     todo!()
// }

// pub struct EmittableClone(Box<dyn Emittable>);

// impl<E> Clone for EmittableClone<E>
// where
//     E: Emittable + Clone,
// {
//     fn clone(&self) -> Self {
//         EmittableClone(self.0.clone())
//     }
// }

// pub struct EmittableClone<E, I>(E, I);

// impl<E, I> Emittable for EmittableClone<E, I>
// where
//     E: Emittable,
//     I: Instrument,
// {
//     type I = I;

//     fn emit(&mut self) -> Result<(), ()> {
//         self.0.emit()
//     }

//     fn get_instrument(&self) -> Arc<RwLock<Self::I>> {
//         todo!()
//     }

//     fn get_event(&self) -> <Self::I as Instrument>::Event {
//         todo!()
//     }
// }

// pub struct EmittableClone(Box<dyn Emittable>);

// impl Clone for EmittableClone {
//     fn clone(&self) -> Self {
//         Self(Box::new(EmittableInstrument))
//     }
// }

// impl<I: Instrument> Emittable for EmittableInstrument<I> {
//     fn emit(&mut self) -> Result<(), ()> {
//         todo!()
//     }
// }

pub type GridIndex = usize;
