pub type GridIndex = usize;

pub trait SampleDepth: hound::Sample + std::ops::AddAssign + std::fmt::Debug {
    const MAX: Self;
    const MIN: Self;
    const MID: Self;
}
