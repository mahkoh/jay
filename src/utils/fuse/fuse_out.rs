use crate::io_uring::WritevData;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_outs::BytesOut;
use crate::utils::fuse::fuse_outs::EmptyOut;
use crate::utils::fuse::fuse_outs::ErrorOut;
use crate::utils::fuse::fuse_outs::FileReadOut;
use crate::utils::fuse::fuse_sys::fuse_attr_out;
use crate::utils::fuse::fuse_sys::fuse_entry_out;
use crate::utils::fuse::fuse_sys::fuse_init_out;
use crate::utils::fuse::fuse_sys::fuse_open_out;
use crate::utils::fuse::fuse_sys::fuse_out_header;
use crate::utils::fuse::fuse_sys::fuse_statfs_out;
use crate::utils::stack::Stack;
use jay_algorithms::oserror::OsError;
use std::any::type_name;
use std::cell::Cell;
use std::marker::PhantomPinned;
use std::ptr;
use std::rc::Rc;
use uapi::Pod;
use uapi::c;
use uapi::c::iovec;

const MAX_NUM_IOVEC: usize = 2;

fn iovec_from_ref<T>(t: &T) -> iovec {
    iovec {
        iov_base: ptr::from_ref(t).cast_mut().cast(),
        iov_len: size_of_val(t) as _,
    }
}

macro_rules! cache {
    ($($field:ident: $ty:ty,)*) => {
        #[derive(Default)]
        pub struct OutCache {
            cleared: Cell<bool>,
            $(
                pub $field: Stack<Box<Fov<$ty>>>,
            )*
        }

        impl OutCache {
            pub fn clear(&self) {
                self.cleared.set(true);
                $(
                    self.$field.clear();
                )*
            }

            $(
                pub fn $field(self: &Rc<Self>) -> Box<Fov<$ty>> {
                    self.$field.pop().unwrap_or_else(|| self.create())
                }
            )*
        }

        $(
            impl FuseOutCache for $ty {
                fn cache(cache: &OutCache) -> &Stack<Box<Fov<Self>>> {
                    &cache.$field
                }
            }
        )*
    };
}

impl OutCache {
    fn create<T>(self: &Rc<Self>) -> Box<Fov<T>>
    where
        T: FuseOut,
    {
        let mut v = Box::new(Fov {
            cache: self.clone(),
            _pinned: Default::default(),
            header: uapi::pod_zeroed(),
            iovecs: [iovec {
                iov_base: ptr::null_mut(),
                iov_len: 0,
            }; MAX_NUM_IOVEC],
            t: T::new(),
        });
        v.iovecs[0] = iovec_from_ref(&v.header);
        v.iovecs[1] = iovec_from_ref(&v.t);
        T::init(&mut v);
        v
    }
}

cache! {
    error: ErrorOut,
    init: fuse_init_out,
    attr: fuse_attr_out,
    entry: fuse_entry_out,
    open: fuse_open_out,
    statfs: fuse_statfs_out,
    bytes: BytesOut,
    file_read: FileReadOut,
    empty: EmptyOut,
}

pub trait FuseOutNew: Sized {
    fn new() -> Self;
}

pub trait FuseOutCache: Sized {
    fn cache(cache: &OutCache) -> &Stack<Box<Fov<Self>>>;
}

pub unsafe trait FuseOut: FuseOutNew + FuseOutCache + 'static {
    const NUM_IOVECS: usize;
    fn init(v: &mut Box<Fov<Self>>) {
        let _ = v;
    }
    fn fini(v: &mut Box<Fov<Self>>) {
        let _ = v;
    }
    fn reset(&mut self) {
        // nothing
    }
}

pub struct Fov<T> {
    cache: Rc<OutCache>,
    _pinned: PhantomPinned,
    pub header: fuse_out_header,
    pub iovecs: [iovec; MAX_NUM_IOVEC],
    pub t: T,
}

unsafe impl<T> WritevData for Fov<T>
where
    T: FuseOut,
{
    fn iovecs(&self) -> &[iovec] {
        &self.iovecs[..T::NUM_IOVECS]
    }

    fn done(mut self: Box<Self>, res: Result<usize, OsError>) {
        if let Err(e) = res
            && e.0 != c::ENOENT
        {
            write_error(type_name::<T>(), e);
        }
        if !self.cache.cleared.get() {
            self.t.reset();
            T::cache(&self.cache.clone()).push(self);
        }
    }
}

impl<T> Fov<T>
where
    T: FuseOut,
{
    pub fn set_len(self: &mut Box<Self>) {
        T::fini(self);
        let mut len = 0;
        for v in &self.iovecs[..T::NUM_IOVECS] {
            len += v.iov_len as u32;
        }
        self.header.len = len;
    }
}

fn write_error(name: &str, e: OsError) {
    log::error!("Could not write data of type {name}: {}", ErrorFmt(e));
}

impl<T> FuseOutNew for T
where
    T: Pod,
{
    fn new() -> Self {
        uapi::pod_zeroed()
    }
}
