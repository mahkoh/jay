use {
    crate::{format::Format, video::Modifier},
    arrayvec::ArrayVec,
    std::rc::Rc,
    uapi::OwnedFd,
};

#[derive(Clone)]
pub struct DmaBufPlane {
    pub offset: u32,
    pub stride: u32,
    pub fd: Rc<OwnedFd>,
}

#[derive(Clone)]
pub struct DmaBuf {
    pub width: i32,
    pub height: i32,
    pub format: &'static Format,
    pub modifier: Modifier,
    pub planes: PlaneVec<DmaBufPlane>,
}

pub const MAX_PLANES: usize = 4;

pub type PlaneVec<T> = ArrayVec<T, MAX_PLANES>;
