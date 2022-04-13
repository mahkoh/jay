use crate::format::Format;

pub mod dmabuf;
pub mod drm;
pub mod gbm;

pub type Modifier = u64;

pub const INVALID_MODIFIER: Modifier = 0x00ff_ffff_ffff_ffff;

#[derive(Copy, Clone)]
pub struct ModifiedFormat {
    pub format: &'static Format,
    pub modifier: Modifier,
}
