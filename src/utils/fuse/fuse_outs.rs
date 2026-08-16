use crate::utils::box_cache::BoxReset;
use crate::utils::box_cache::CachedBox;
use crate::utils::fuse::fuse_out::Fov;
use crate::utils::fuse::fuse_out::FuseOut;
use crate::utils::fuse::fuse_out::FuseOutNew;
use crate::utils::fuse::fuse_sys::fuse_attr_out;
use crate::utils::fuse::fuse_sys::fuse_entry_out;
use crate::utils::fuse::fuse_sys::fuse_init_out;
use crate::utils::fuse::fuse_sys::fuse_open_out;
use crate::utils::fuse::fuse_sys::fuse_statfs_out;
use jay_proc::Pod;
use std::rc::Rc;
use uapi::c::iovec;

#[derive(Pod)]
pub struct ErrorOut;

unsafe impl FuseOut for ErrorOut {
    const NUM_IOVECS: usize = 1;
}

unsafe impl FuseOut for fuse_init_out {
    const NUM_IOVECS: usize = 2;
}

unsafe impl FuseOut for fuse_attr_out {
    const NUM_IOVECS: usize = 2;
}

unsafe impl FuseOut for fuse_entry_out {
    const NUM_IOVECS: usize = 2;
}

unsafe impl FuseOut for fuse_open_out {
    const NUM_IOVECS: usize = 2;
}

unsafe impl FuseOut for fuse_statfs_out {
    const NUM_IOVECS: usize = 2;
}

#[derive(Pod)]
pub struct EmptyOut;

unsafe impl FuseOut for EmptyOut {
    const NUM_IOVECS: usize = 1;
}

#[derive(Default)]
pub struct BytesOut {
    pub buf: Vec<u8>,
}

impl FuseOutNew for BytesOut {
    fn new() -> Self {
        Self::default()
    }
}

unsafe impl FuseOut for BytesOut {
    const NUM_IOVECS: usize = 2;

    fn fini(v: &mut Box<Fov<Self>>) {
        v.iovecs[1] = iovec {
            iov_base: v.t.buf.as_mut_ptr().cast(),
            iov_len: v.t.buf.len(),
        };
    }

    fn reset(&mut self) {
        self.buf.clear();
    }
}

#[derive(Default)]
pub struct FileReadOut {
    pub data: Option<Rc<CachedBox<String, BoxReset>>>,
    pub offset: usize,
    pub len: usize,
}

impl FuseOutNew for FileReadOut {
    fn new() -> Self {
        Self::default()
    }
}

unsafe impl FuseOut for FileReadOut {
    const NUM_IOVECS: usize = 2;

    fn fini(v: &mut Box<Fov<Self>>) {
        let s = v.t.data.as_ref().map(|v| v.as_str()).unwrap_or_default();
        let offset = v.t.offset.min(s.len());
        let len = v.t.len.min(s.len() - offset);
        unsafe {
            v.iovecs[1] = iovec {
                iov_base: s.as_ptr().add(offset).cast_mut().cast(),
                iov_len: len as _,
            };
        }
    }

    fn reset(&mut self) {
        self.data.take();
    }
}
