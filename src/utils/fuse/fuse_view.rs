use crate::utils::fuse::fuse_dir::FuseDirent;
use crate::utils::fuse::fuse_dir::FuseDirents;
use crate::utils::fuse::fuse_inode::FuseInodeBase;
use crate::utils::fuse::fuse_inode::FuseInodeProps;
use crate::utils::liveness::GetLiveness;
use crate::utils::liveness::Liveness;
use crate::utils::str_fmt::StrCtx;
use crate::utils::type_view::TypeView;
use crate::utils::type_view::TypeViewExt2;
use crate::utils::type_view::tv_unwrap_rc_ref;
use crate::utils::type_view::tv_wrap_weak;
use std::rc::Rc;
use std::rc::Weak;

#[cfg_attr(not(test), expect(unused))]
pub trait FuseView<T>: 'static
where
    T: ?Sized,
{
    fn props(t: &T, key: u64) -> FuseInodeProps;
    fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {
        let _ = t;
        let _ = key;
        let _ = name;
        None
    }
    fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {
        let _ = t;
        let _ = key;
        let _ = dirents;
    }
    fn read(t: &T, key: u64, buf: &mut String, ctx: &StrCtx) {
        let _ = t;
        let _ = key;
        let _ = buf;
        let _ = ctx;
    }
    fn readlink(t: &T, key: u64, depth: u64, buf: &mut String) {
        let _ = t;
        let _ = key;
        let _ = depth;
        let _ = buf;
    }
}

impl<T, V> FuseInodeBase for TypeView<T, V>
where
    T: GetLiveness + 'static,
    V: FuseView<T> + 'static,
{
    fn liveness(&self) -> &Liveness {
        self.tv_unwrap_ref().get_liveness()
    }

    fn props(&self, key: u64) -> FuseInodeProps {
        V::props(self, key)
    }

    fn lookup(self: Rc<Self>, key: u64, name: &str) -> Option<FuseDirent> {
        V::lookup(self.tv_unwrap_rc(), key, name)
    }

    fn getdents(self: Rc<Self>, key: u64, dirents: &mut FuseDirents) {
        V::getdents(self.tv_unwrap_rc(), key, dirents)
    }

    fn read(&self, key: u64, buf: &mut String, ctx: &StrCtx) {
        V::read(self, key, buf, ctx)
    }

    fn readlink(&self, key: u64, depth: u64, buf: &mut String) {
        V::readlink(self, key, depth, buf)
    }

    fn downgrade(self: &Rc<Self>) -> Weak<Self>
    where
        Self: Sized,
    {
        tv_wrap_weak(Rc::downgrade(tv_unwrap_rc_ref(self)))
    }
}
