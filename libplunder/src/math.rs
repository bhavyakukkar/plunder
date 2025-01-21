use crate::types::SampleDepth;

impl SampleDepth for i8 {
    const MAX: i8 = i8::MAX;
    const MIN: i8 = i8::MIN;
    const MID: i8 = 0;
}
impl SampleDepth for i16 {
    const MAX: i16 = i16::MAX;
    const MIN: i16 = i16::MIN;
    const MID: i16 = 0;
}
impl SampleDepth for i32 {
    const MAX: i32 = i32::MAX;
    const MIN: i32 = i32::MIN;
    const MID: i32 = 0;
}
impl SampleDepth for f32 {
    const MAX: f32 = f32::MAX;
    const MIN: f32 = f32::MIN;
    const MID: f32 = 0.;
}
