use std::collections::HashSet;

use crate::instruments::Instrument;

struct Engine {
    active_instruments: HashSet<*const Box<dyn Instrument>>,
}
