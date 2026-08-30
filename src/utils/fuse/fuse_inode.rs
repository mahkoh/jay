use crate::utils::fuse::fuse_dir::FuseDirent;
use crate::utils::fuse::fuse_dir::FuseDirents;
use crate::utils::fuse::fuse_mgr::FuseIno;
use crate::utils::fuse::fuse_sys::fuse_attr;
use crate::utils::liveness::Liveness;
use crate::utils::liveness::LivenessView;
use crate::utils::str_fmt::StrCtx;
use crate::utils::uid::gid;
use crate::utils::uid::uid;
use std::any::TypeId;
use std::ops::Deref;
use std::rc::Rc;
use std::rc::Weak;
use uapi::c;
use uapi::pod_zeroed;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FuseInodeTy {
    Regular,
    Directory,
    Symlink,
}

#[derive(Copy, Clone)]
pub struct FuseInodeProps {
    pub ty: FuseInodeTy,
    pub writable: bool,
}

#[derive(Copy, Clone)]
pub struct FuseInodePropsPti {
    pub(super) props: FuseInodeProps,
    pub(super) liveness: LivenessView,
    pub(super) type_id: TypeId,
}

impl Deref for FuseInodePropsPti {
    type Target = FuseInodeProps;

    fn deref(&self) -> &Self::Target {
        &self.props
    }
}

pub trait FuseInodeBase: 'static {
    fn liveness(&self) -> &Liveness
    where
        Self: Sized;
    fn props(&self, key: u64) -> FuseInodeProps
    where
        Self: Sized;
    fn lookup(self: Rc<Self>, key: u64, name: &str) -> Option<FuseDirent> {
        let _ = key;
        let _ = name;
        None
    }
    fn getdents(self: Rc<Self>, key: u64, dirents: &mut FuseDirents) {
        let _ = key;
        let _ = dirents;
    }
    fn read(&self, key: u64, buf: &mut String, ctx: &StrCtx) {
        let _ = key;
        let _ = buf;
        let _ = ctx;
    }
    fn readlink(&self, key: u64, depth: u64, buf: &mut String) {
        let _ = key;
        let _ = depth;
        let _ = buf;
    }
    fn downgrade(self: &Rc<Self>) -> Weak<Self>
    where
        Self: Sized,
    {
        Rc::downgrade(self)
    }
}

pub trait FuseInode: FuseInodeBase {
    fn props_pti(&self, key: u64) -> FuseInodePropsPti;
}

impl<T> FuseInode for T
where
    T: FuseInodeBase,
{
    fn props_pti(&self, key: u64) -> FuseInodePropsPti {
        FuseInodePropsPti {
            type_id: TypeId::of::<Self>(),
            props: self.props(key),
            liveness: self.liveness().view(),
        }
    }
}

pub struct FuseInodeWithKey {
    pub inode: Rc<dyn FuseInode>,
    pub key: u64,
}

#[expect(unused)]
pub trait FuseInodeExt {
    fn with_key(self: Rc<Self>, key: u64) -> FuseInodeWithKey
    where
        Self: Sized;

    fn without_key(self: Rc<Self>) -> FuseInodeWithKey
    where
        Self: Sized,
    {
        self.with_key(0)
    }
}

impl<T> FuseInodeExt for T
where
    T: FuseInode + ?Sized,
{
    fn with_key(self: Rc<Self>, key: u64) -> FuseInodeWithKey
    where
        Self: Sized,
    {
        FuseInodeWithKey { inode: self, key }
    }
}

impl dyn FuseInode {
    #[expect(unused)]
    pub fn with_key(self: Rc<Self>, key: u64) -> FuseInodeWithKey {
        FuseInodeWithKey { inode: self, key }
    }

    #[expect(unused)]
    pub fn without_key(self: Rc<Self>) -> FuseInodeWithKey {
        self.with_key(0)
    }
}

impl FuseInodeProps {
    #[expect(unused)]
    pub fn dir() -> Self {
        Self {
            ty: FuseInodeTy::Directory,
            writable: false,
        }
    }

    #[expect(unused)]
    pub fn reg() -> Self {
        Self {
            ty: FuseInodeTy::Regular,
            writable: false,
        }
    }

    #[expect(unused)]
    pub fn link() -> Self {
        Self {
            ty: FuseInodeTy::Symlink,
            writable: false,
        }
    }

    pub(super) fn mode(&self) -> c::mode_t {
        let mut mode = 0o444;
        match self.ty {
            FuseInodeTy::Regular => {
                mode |= c::S_IFREG;
            }
            FuseInodeTy::Directory => {
                mode |= c::S_IFDIR;
                mode |= 0o111;
            }
            FuseInodeTy::Symlink => {
                mode |= c::S_IFLNK;
            }
        };
        if self.writable {
            mode |= 0o200;
        }
        mode
    }

    pub(super) fn attr(&self, ino: Option<FuseIno>) -> fuse_attr {
        fuse_attr {
            ino: ino.map(|i| i.0.get()).unwrap_or(0),
            mode: self.mode(),
            nlink: 1,
            uid: uid() as u32,
            gid: gid() as u32,
            ..pod_zeroed()
        }
    }
}
