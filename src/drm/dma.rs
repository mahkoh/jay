use crate::format::Format;
use uapi::OwnedFd;

pub struct DmaBufPlane {
    pub offset: u32,
    pub stride: u32,
    pub fd: OwnedFd,
}

pub struct DmaBuf {
    pub width: i32,
    pub height: i32,
    pub format: &'static Format,
    pub modifier: u64,
    pub planes: Vec<DmaBufPlane>,
}
