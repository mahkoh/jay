pub mod dmabuf;
pub mod drm;
pub mod gbm;

pub type Modifier = u64;

pub const INVALID_MODIFIER: Modifier = 0x00ff_ffff_ffff_ffff;
pub const LINEAR_MODIFIER: Modifier = 0;

// This is required by AMD and therefore everyone else uses this too.
pub const LINEAR_STRIDE_ALIGN: u64 = 256;
