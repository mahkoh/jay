#[cfg(feature = "tracy")]
#[macro_use]
mod tracy_impl;

#[cfg(feature = "tracy")]
use tracy_impl as imp;

#[cfg(not(feature = "tracy"))]
#[macro_use]
mod tracy_noop;

#[cfg(not(feature = "tracy"))]
use tracy_noop as imp;

pub use imp::{enable_profiler, FrameName, ZoneName};
