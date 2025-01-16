use std::{
    fs::File,
    io::BufReader,
    marker::PhantomData,
    path::Path,
    sync::{Arc, RwLock},
};

use mlua::{FromLua, MetaMethod, UserData, UserDataMethods, Value};

use crate::{
    types::SampleDepth,
    utils::{self, Log},
};

// TODO remove Event and fn emit from trait Instrument
// Instruments can decide how they want their events to be extracted
// All that's needed is that those extracted events can be invoked (`FnMut()` will suffice)
pub trait Instrument: Iterator<Item = Result<<Self as Instrument>::Depth, hound::Error>> {
    type Event: Clone + FromLua;
    type Depth: SampleDepth;

    fn emit(&mut self, event: <Self as Instrument>::Event);
    // fn get_event(methods: mlua::UserDataMethods<Self>, fields: mlua::UserDataFields<Self>);
}

// impl<I> UserData for InstrumentUserData<I> {
//     fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {}

//     fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {}

//     fn register(registry: &mut mlua::UserDataRegistry<Self>) {
//         Self::add_fields(registry);
//         Self::add_methods(registry);
//     }
// }

pub struct InstrumentUserData<I>(pub Arc<RwLock<I>>);

pub struct EmittableInstrument<I: Instrument> {
    pub instrument: Arc<RwLock<I>>,
    pub event: I::Event,
}

impl<I> UserData for InstrumentUserData<I>
where
    I: Instrument + 'static,
{
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        use mlua::IntoLua;

        // get emittable instrument containing event decoded from the value used to index the
        // instrument
        methods.add_meta_method_mut(
            MetaMethod::Index,
            |lua, InstrumentUserData(instrument), event_lua: Value| -> mlua::Result<Value> {
                let event_name = event_lua.to_string()?;
                match I::Event::from_lua(event_lua, lua) {
                    Ok(event) => {
                        utils::lua_debug(
                            lua,
                            &format!("extracted event {} from instrument", event_name),
                        )?;

                        let emittable = EmittableInstrument {
                            instrument: instrument.clone(),
                            event,
                        };
                        let dyn_emittable_box: Box<dyn Emittable> = Box::new(emittable);
                        Ok(dyn_emittable_box.into_lua(lua)?)
                    }
                    Err(_) => {
                        utils::lua_warn(
                            lua,
                            &format!("event '{}' does not exist on this instrument", event_name),
                        )?;
                        Ok(Value::Nil)
                    }
                }
            },
        );

        // TODO pairs metamethod to get all available events
        // methods.add_meta_method(MetaMethod::Pairs, |lua,InstrumentUserData(instrument), _: ()| 2);
    }
}

pub trait Emittable: EmittableClone {
    fn emit(&mut self) -> Result<(), ()>;
}

pub trait EmittableClone {
    fn clone_box(&self) -> Box<dyn Emittable>;
}

impl<E> EmittableClone for E
where
    E: Emittable + 'static + Clone,
{
    fn clone_box(&self) -> Box<dyn Emittable> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Emittable> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl<I> EmittableClone for EmittableInstrument<I>
where
    I: Instrument + 'static,
{
    fn clone_box(&self) -> Box<(dyn Emittable)> {
        Box::new(Self {
            instrument: self.instrument.clone(),
            event: self.event.clone(),
        })
    }
}

impl<I> Emittable for EmittableInstrument<I>
where
    I: Instrument + 'static,
{
    fn emit(&mut self) -> Result<(), ()> {
        self.instrument
            .try_write()
            .map_err(|err| {
                mlua::Error::runtime(format!(
                    "concurrency error emitting event to the instrument: {}",
                    err
                ));
                todo!()
            })?
            .emit(self.event.clone());
        Ok(())
    }
}

impl UserData for Box<dyn Emittable> {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method_mut(MetaMethod::Call, |_, emittable, _: ()| {
            emittable.emit().map_err(|_| {
                mlua::Error::runtime("error emitting event to instrument".to_string())
            })?;
            Ok(())
        });
    }
}

// impl<I: Instrument> UserData for EmittableInstrument<I> {
//     fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
//         methods.add_meta_method_mut(
//             MetaMethod::Call,
//             |_, EmittableInstrument { instrument, event }, _: ()| {
//                 instrument
//                     .try_write()
//                     .map_err(|err| {
//                         mlua::Error::runtime(format!(
//                             "concurrency error emitting event to the instrument: {}",
//                             err
//                         ))
//                     })?
//                     .emit(event.clone());
//                 Ok(())
//             },
//         );
//     }
// }
//
pub mod wav_instrument {
    use super::*;
    use hound::WavReader;
    use mlua::Value;

    #[derive(Debug, Clone)]
    pub enum ClipEvent {
        Play,
        Pause,
        Stop,
        Resume,
        Multiple(Vec<ClipEvent>),
    }

    impl PartialEq for ClipEvent {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (ClipEvent::Play, ClipEvent::Play)
                | (ClipEvent::Pause, ClipEvent::Pause)
                | (ClipEvent::Stop, ClipEvent::Stop)
                | (ClipEvent::Resume, ClipEvent::Resume) => true,
                (ClipEvent::Multiple(clip_events1), ClipEvent::Multiple(clip_events2)) => {
                    clip_events1.len().eq(&clip_events2.len())
                        && clip_events1
                            .iter()
                            .zip(clip_events2.iter())
                            .map(|(clip_event1, clip_event2)| clip_event1.eq(&clip_event2))
                            .fold(true, |a, b| a && b)
                }
                _ => false,
            }
        }
    }

    impl FromLua for ClipEvent {
        fn from_lua(value: Value, lua: &mlua::Lua) -> mlua::Result<Self> {
            match value {
                Value::Boolean(b) => Ok(if b {
                    ClipEvent::Resume
                } else {
                    ClipEvent::Pause
                }),
                Value::Integer(n) => match n {
                    1 => Ok(ClipEvent::Play),
                    2 => Ok(ClipEvent::Resume),
                    3 => Ok(ClipEvent::Pause),
                    4 => Ok(ClipEvent::Stop),
                    n => Err(mlua::Error::runtime(format!(
                        "unknown clip-event index: {n}"
                    ))),
                },
                Value::String(s) => match s.to_string_lossy().as_str() {
                    "play" | "Play" | "||>" => Ok(ClipEvent::Play),
                    "pause" | "Pause" | "||" => Ok(ClipEvent::Pause),
                    "stop" | "Stop" | "|]" | "[|" | "o" => Ok(ClipEvent::Stop),
                    "resume" | "Resume" | "|>" => Ok(ClipEvent::Resume),
                    s => Err(mlua::Error::runtime(format!("unknown clip-event: {s}"))),
                },
                Value::Table(table) => Ok(ClipEvent::Multiple(
                    table
                        .pairs::<Value, Value>()
                        .filter_map(|pair| pair.ok())
                        .map(|(_, value)| ClipEvent::from_lua(value, lua))
                        // collect here is converting an iterator of results to a result of a vector
                        // similar to haskell's sequence
                        // https://doc.rust-lang.org/src/core/result.rs.html#1936
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                //TODO better errors man
                _ => Err(mlua::Error::runtime(format!("bad type"))),
            }
        }
    }

    /// An instrument that's a buffered WAV reader
    pub struct Wav<T> {
        reader: WavReader<BufReader<File>>,
        outputting: bool,
        t: PhantomData<T>,
    }

    impl<T> Wav<T> {
        /// Load the WAV reader from a file
        pub fn load<P>(path: P) -> Result<Self, hound::Error>
        where
            P: AsRef<Path>,
            T: SampleDepth,
        {
            WavReader::open(path).map(|reader| Self {
                reader,
                outputting: false,
                t: PhantomData,
            })
        }
    }

    impl<T: SampleDepth> Iterator for Wav<T> {
        type Item = Result<T, hound::Error>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.outputting {
                self.reader.samples().next()
            } else {
                None
            }
        }
    }

    impl<T> Instrument for Wav<T>
    where
        T: SampleDepth,
    {
        type Event = ClipEvent;

        type Depth = T;

        fn emit(&mut self, event: <Self as Instrument>::Event) {
            use ClipEvent::*;
            match event {
                Play => {
                    self.reader
                        .seek(0)
                        .expect("Can't seek to beginning of file");
                    self.outputting = true;
                }
                Pause => self.outputting = false,
                Stop => {
                    self.reader
                        .seek(0)
                        .expect("Can't seek to beginning of file");
                    self.outputting = false;
                }
                Resume => self.outputting = true,
                Multiple(clip_events) => {
                    for clip_event in clip_events {
                        self.emit(clip_event);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{ClipEvent, Instrument, SampleDepth, Wav};

        // TODO Failing because why?
        #[test]
        fn test_wav_f32() {
            let mut wav: Wav<f32> = Wav::load("./bt.wav").unwrap();
            wav.emit(ClipEvent::Resume);
            assert!(wav.next().map(|sample| sample.is_err()).unwrap());
        }

        #[test]
        fn test_wav_i32() {
            let wav: Wav<i32> = Wav::load("./bt.wav").unwrap();
            test_wav_instrument_events(wav);
        }

        #[test]
        fn test_wav_i16() {
            let wav: Wav<i16> = Wav::load("./bt.wav").unwrap();
            test_wav_instrument_events(wav);
        }

        // Failing because TooWide
        #[test]
        fn test_wav_i8() {
            let mut wav: Wav<i8> = Wav::load("./bt.wav").unwrap();
            wav.emit(ClipEvent::Resume);
            assert!(wav.next().map(|sample| sample.is_err()).unwrap());
        }

        fn test_wav_instrument_events<T>(mut wav: Wav<T>)
        where
            T: SampleDepth + std::fmt::Debug + PartialEq + Into<f64>,
        {
            use ClipEvent::*;

            for _ in 0..1000 {
                assert_eq!(wav.next().map(|err| err.unwrap()), None);
            }

            // Play() -> seek to 0 and enable outputting
            wav.emit(Play);
            for i in 0..500 {
                assert_eq!(
                    wav.next().map(|err| err.unwrap()).unwrap().into(),
                    FIRST_1000_SAMPLES[i]
                );
            }

            // Pause() -> just disable outputting
            wav.emit(Pause);
            for _ in 0..500 {
                assert_eq!(wav.next().map(|err| err.unwrap()), None);
            }

            // Resume() -> just enable outputting
            wav.emit(Resume);
            for i in 500..1000 {
                assert_eq!(
                    wav.next().map(|err| err.unwrap()).unwrap().into(),
                    FIRST_1000_SAMPLES[i]
                );
            }

            // Stop() -> seek to 0 and disable outputting
            wav.emit(Stop);
            for _ in 0..500 {
                assert_eq!(wav.next().map(|err| err.unwrap()), None);
            }
            wav.emit(Resume);
            for i in 0..1000 {
                assert_eq!(
                    wav.next().map(|err| err.unwrap()).unwrap().into(),
                    FIRST_1000_SAMPLES[i]
                );
            }
        }

        const FIRST_1000_SAMPLES: [f64; 1000] = [
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., -1., -17., -26., -175., -278.,
            -723., -1218., -1442., -2451., -1954., -2364., -2904., -1470., -3788., -1402., -4261.,
            -1843., -5106., -3275., -4846., -4359., -4975., -4075., -5853., -4503., -6237., -5206.,
            -7248., -6282., -8699., -8152., -9094., -8661., -8550., -8243., -8323., -8237., -7140.,
            -8123., -6510., -7825., -7070., -8273., -6862., -8683., -7205., -8692., -7963., -8341.,
            -7309., -7214., -7334., -7276., -7472., -7986., -6278., -6924., -5420., -5418., -5608.,
            -5387., -3608., -4339., -3146., -4229., -3271., -3278., -1584., -2387., 212., -1796.,
            1171., -1103., 2394., -175., 3068., 655., 2399., 388., 2550., 489., 4550., 2216.,
            6906., 4800., 7663., 4780., 7880., 4916., 7887., 4422., 7002., 2958., 6902., 2645.,
            8152., 4298., 9677., 5726., 10265., 5951., 10361., 6065., 9513., 5911., 8262., 5883.,
            7354., 5595., 8822., 6768., 10984., 9482., 12944., 11670., 13667., 11984., 14435.,
            12252., 13452., 11993., 12650., 11400., 13584., 11653., 14514., 12326., 14061., 11368.,
            13308., 9862., 12482., 9630., 12591., 9669., 12346., 9394., 11691., 8349., 10573.,
            7139., 10438., 6942., 10613., 7613., 10871., 8329., 10883., 7939., 10486., 7276.,
            8517., 5510., 7185., 4810., 7632., 5060., 8024., 5593., 8128., 6024., 7079., 4818.,
            5332., 3771., 3840., 2443., 2688., 1899., 2070., 1328., 2683., 2542., 1530., 1435.,
            608., 356., 314., -303., -900., -2048., -3658., -4495., -5376., -5658., -5427., -5044.,
            -5766., -5491., -6223., -6055., -7939., -7763., -9895., -9874., -11458., -10613.,
            -11971., -11831., -12827., -13160., -14066., -13964., -13258., -13198., -11676.,
            -12875., -10817., -12726., -11140., -13472., -11861., -14386., -13998., -15426.,
            -15302., -15893., -14250., -14377., -13890., -13367., -14742., -14097., -15871.,
            -15951., -17291., -17047., -19807., -18376., -20577., -19101., -18941., -18965.,
            -18803., -18507., -18657., -17840., -18409., -16973., -17892., -16686., -17005.,
            -16637., -15973., -15604., -16683., -16056., -16836., -15838., -15645., -15453.,
            -14630., -14003., -14884., -13405., -15290., -12840., -14837., -12524., -13211.,
            -12443., -12861., -12323., -12952., -11996., -11720., -10864., -10750., -10240.,
            -11459., -10544., -12154., -11597., -13003., -11830., -12970., -11403., -11975.,
            -11044., -11256., -10234., -10580., -9158., -9693., -8195., -8865., -7972., -8620.,
            -6758., -8011., -5797., -7214., -5251., -5887., -4584., -3874., -2263., -1380., 406.,
            183., 1399., 1055., 3086., 1917., 2902., 2420., 2574., 2181., 2809., 3104., 3377.,
            4187., 4389., 4366., 5279., 5256., 5918., 6379., 6957., 6912., 8066., 7269., 8884.,
            7725., 8439., 6715., 7580., 4830., 6058., 5100., 5455., 6648., 7303., 6948., 7860.,
            7454., 8685., 7930., 9331., 8736., 10045., 10799., 11535., 12608., 13783., 14465.,
            16779., 16234., 18866., 17985., 20973., 18544., 21883., 18547., 21990., 18977., 23223.,
            20125., 24676., 22993., 27056., 24982., 28948., 25875., 29941., 25862., 29728., 25493.,
            29355., 24882., 28625., 24248., 27986., 23107., 26663., 22795., 26160., 22262., 25111.,
            20489., 23062., 17504., 20289., 14438., 17249., 12102., 15785., 9708., 14172., 8809.,
            12219., 7696., 10518., 5934., 9000., 4010., 7284., 2582., 5039., 2009., 3897., 1684.,
            2993., 577., 1949., -2610., -569., -3823., -2990., -2942., -2325., -4240., -2989.,
            -6709., -4787., -8114., -6220., -8788., -6504., -8445., -5104., -8611., -5121.,
            -10754., -7387., -12422., -8371., -12253., -8127., -14099., -9973., -14969., -11987.,
            -15796., -13229., -16086., -13935., -16520., -14788., -16717., -15287., -16860.,
            -15091., -15488., -14117., -14994., -13842., -14080., -12520., -12869., -11518.,
            -11705., -10734., -10860., -10520., -9091., -8353., -8262., -6386., -8296., -6006.,
            -6829., -6483., -4856., -5975., -3087., -3618., -3633., -3126., -5241., -5389., -5483.,
            -6989., -2312., -4125., -493., -2526., -1078., -3383., -1242., -3534., -947., -2319.,
            -257., -1654., 533., -1184., 1188., -716., 606., -703., 554., 465., 565., 87., -506.,
            -2144., -1615., -2968., -1527., -2323., -887., -3441., -73., -3520., -1072., -3650.,
            -1425., -3355., 359., -2131., 1047., -2468., 1096., -2054., 1578., -1188., 1933., 1.,
            2073., 436., 1517., -30., 954., -181., 1143., 547., 1651., 210., 1224., -1568., 1702.,
            -1469., 1644., -2044., 939., -1918., 15., -1826., 377., -2426., 995., -2629., 358.,
            -2951., -608., -3542., -1473., -4096., -2318., -4657., -3996., -6970., -4696., -7537.,
            -4840., -8615., -4505., -9256., -3834., -9094., -5191., -9000., -5441., -9430., -4168.,
            -9743., -3580., -9560., -3190., -8846., -2891., -7832., -3749., -7471., -3677., -6564.,
            -3322., -5967., -3867., -6133., -2613., -5356., -1339., -4812., -1370., -4786., -276.,
            -4078., 649., -2336., 538., -1891., 1626., -675., 3687., 1471., 3427., 1725., 2839.,
            727., 2778., 1153., 1479., -13., 653., -1296., 629., -236., 482., -52., 427., -1553.,
        ];
    }
}

pub mod midi_instrument {
    use super::*;
    use mlua::Value;

    pub struct MidiPlayer<T>(PhantomData<T>);

    impl<T> MidiPlayer<T> {
        pub fn new<P>(_path: P) -> Result<Self, std::io::Error>
        where
            P: AsRef<Path>,
        {
            // TODO
            Ok(MidiPlayer(PhantomData))
        }
    }

    #[rustfmt::skip]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Key { C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Note(pub Key, pub(crate) usize);

    impl Note {
        pub fn from_spanned_str(s: &[(usize, char)]) -> Result<Option<Self>, String> {
            // let s = s.chars().collect::<Vec<_>>();
            Ok(Some(match s.len() {
                2 => Note(
                    match s[0].1 {
                        'C' | 'c' => Key::C,
                        'D' | 'd' => Key::D,
                        'E' | 'e' => Key::E,
                        'F' | 'f' => Key::F,
                        'G' | 'g' => Key::G,
                        'A' | 'a' => Key::A,
                        'B' | 'b' => Key::B,
                        _ => return Err(format!("at {}: invalid key", s[0].0)),
                    },
                    String::from(s[1].1)
                        .parse()
                        .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
                ),
                3 => Note(
                    match (s[0].1, s[1].1) {
                        ('C', '#') | ('c', '#') | ('D', 'b') | ('d', 'b') => Key::Db,
                        ('D', '#') | ('d', '#') | ('E', 'b') | ('e', 'b') => Key::Eb,
                        ('F', '#') | ('f', '#') | ('G', 'b') | ('g', 'b') => Key::Gb,
                        ('G', '#') | ('g', '#') | ('A', 'b') | ('a', 'b') => Key::Ab,
                        ('A', '#') | ('a', '#') | ('B', 'b') | ('b', 'b') => Key::Bb,

                        ('C', _) | ('c', _) => Key::C,
                        ('D', _) | ('d', _) => Key::D,
                        ('E', _) | ('e', _) => Key::E,
                        ('F', _) | ('f', _) => Key::F,
                        ('G', _) | ('g', _) => Key::G,
                        ('A', _) | ('a', _) => Key::A,
                        ('B', _) | ('b', _) => Key::B,
                        _ => return Err(format!("at {}: invalid key", s[0].0)),
                    },
                    String::from(s[2].1)
                        .parse()
                        .map_err(|_| format!("at {}: invalid octave number", s[1].0))?,
                ),
                0 => {
                    return Ok(None);
                }
                _ => {
                    return Err(format!(
                        "at {}: note string may only be or 2 or 3 characters long",
                        s[0].0
                    ));
                }
            }))
        }
    }

    impl FromLua for Note {
        fn from_lua(value: Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
            use mlua::Error;

            let spanned_str: Vec<_> = value
                .as_string()
                .ok_or(Error::runtime("invalid string: not UTF-8"))?
                .to_str()?
                .chars()
                .enumerate()
                .collect();

            Ok(Note::from_spanned_str(&spanned_str)
                .map_err(|err| Error::runtime(err))?
                .ok_or(Error::runtime(
                    "cannot make Note out of empty string".to_string(),
                ))?)
        }
    }

    impl<T: SampleDepth> Iterator for MidiPlayer<T> {
        type Item = Result<T, hound::Error>;

        fn next(&mut self) -> Option<Self::Item> {
            todo!()
        }
    }

    impl<T: SampleDepth> Instrument for MidiPlayer<T> {
        type Event = Note;
        type Depth = T;

        fn emit(&mut self, event: <Self as Instrument>::Event) {
            todo!()
        }
    }
}
