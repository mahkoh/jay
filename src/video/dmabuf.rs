use {crate::format::Format, std::rc::Rc, uapi::OwnedFd};

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
    pub modifier: u64,
    pub planes: Vec<DmaBufPlane>,
}
