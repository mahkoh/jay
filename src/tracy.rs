#[cfg(feature = "tracy")]
#[macro_use]
mod tracy_impl;

#[cfg(feature = "tracy")]
use tracy_impl as imp;

#[cfg(not(feature = "tracy"))]
#[macro_use]
mod tracy_noop;

pub use imp::FrameName;
pub use imp::ZoneName;
pub use imp::enable_profiler;
#[cfg(not(feature = "tracy"))]
use tracy_noop as imp;
