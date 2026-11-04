//! Value operators: arithmetic on single values. The compiler calls them once per sample, so
//! they work on audio signals and parameters alike, depending on what is connected.

pub mod arithmetic;
pub mod comparison;
pub mod mixing;
