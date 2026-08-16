use crate::utils::binary_search_map::BinarySearchMapDyn;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fuse::fuse_dir::FUSE_SHORT_TIMEOUT;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_view::FuseView;
use crate::utils::get_inner::GetInner;
use crate::utils::liveness::GetLiveness;
use crate::utils::markers::JayHash;
use crate::utils::reset::Reset;
use crate::utils::str_fmt::StrCtx;
use hashbrown::HashMap;
use std::borrow::Borrow;
use std::hash::BuildHasher;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;
use uapi::c;

#[expect(unused)]
pub trait FuseRegView<T>: 'static {
    fn read(t: &T, key: u64, buf: &mut String, ctx: &StrCtx);

    fn add_newline(key: u64) -> bool {
        let _ = key;
        true
    }
}

#[expect(unused)]
pub struct FuseReg<V>(PhantomData<fn() -> V>);

mod fuse_reg {
    use crate::utils::fuse::fuse_inode::FuseInodeProps;
    use crate::utils::fuse::fuse_view::FuseView;
    use crate::utils::fuse::fuse_views::FuseReg;
    use crate::utils::fuse::fuse_views::FuseRegView;
    use crate::utils::str_fmt::StrCtx;
    use crate::utils::str_fmt::StrFmtFmt;

    impl<T, V> FuseView<T> for FuseReg<V>
    where
        V: FuseRegView<T>,
    {
        fn props(_t: &T, _key: u64) -> FuseInodeProps {
            FuseInodeProps::reg()
        }

        fn read(t: &T, key: u64, buf: &mut String, ctx: &StrCtx) {
            V::read(t, key, buf, ctx);
            if ctx.fmt == StrFmtFmt::Human && V::add_newline(key) {
                buf.push_str("\n");
            }
        }
    }
}

#[expect(unused)]
pub trait FuseLinkView<T>: 'static {
    fn readlink(t: &T, key: u64, depth: u64, buf: &mut String);
}

#[expect(unused)]
pub struct FuseLink<V>(PhantomData<fn() -> V>);

mod fuse_link {
    use crate::utils::fuse::fuse_inode::FuseInodeProps;
    use crate::utils::fuse::fuse_view::FuseView;
    use crate::utils::fuse::fuse_views::FuseLink;
    use crate::utils::fuse::fuse_views::FuseLinkView;

    impl<T, V> FuseView<T> for FuseLink<V>
    where
        V: FuseLinkView<T>,
    {
        fn props(_t: &T, _key: u64) -> FuseInodeProps {
            FuseInodeProps::link()
        }

        fn readlink(t: &T, key: u64, depth: u64, buf: &mut String) {
            V::readlink(t, key, depth, buf);
        }
    }
}

#[expect(unused)]
pub trait IterDirView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;

    fn iter(t: &T, key: u64, f: impl FnMut(&str, &Rc<Self::Value>));
    fn get(t: &T, key: u64, name: &str) -> Option<Rc<Self::Value>>;
}

#[expect(unused)]
pub struct IterDir<V>(PhantomData<fn() -> V>);

mod iter_dir {
    use crate::utils::fuse::fuse_dir::FuseDirent;
    use crate::utils::fuse::fuse_dir::FuseDirentName;
    use crate::utils::fuse::fuse_dir::FuseDirents;
    use crate::utils::fuse::fuse_inode::FuseInodeProps;
    use crate::utils::fuse::fuse_view::FuseView;
    use crate::utils::fuse::fuse_views::IterDir;
    use crate::utils::fuse::fuse_views::IterDirView;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::type_view::TypeViewExt1;
    use std::rc::Rc;

    impl<T, V> FuseView<T> for IterDir<V>
    where
        T: GetLiveness,
        V: IterDirView<T>,
    {
        fn props(_t: &T, _key: u64) -> FuseInodeProps {
            FuseInodeProps::dir()
        }

        fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {
            Some(FuseDirent {
                inode: V::get(&t, key, name)?.tv_wrap_rc::<V::View>(),
                key: 0,
                static_name: None,
                timeout_ns: V::TIMEOUT_NS,
            })
        }

        fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {
            V::iter(&t, key, |name, e| {
                dirents.add(
                    V::TIMEOUT_NS,
                    e.tv_wrap_rc_ref::<V::View>(),
                    0,
                    FuseDirentName::Dynamic(name),
                );
            });
        }
    }
}

#[expect(unused)]
pub trait IterDirKeyedView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;

    fn iter(t: Rc<T>, key: u64, f: impl FnMut(&str, &Rc<Self::Value>, u64));
    fn get(t: Rc<T>, key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)>;
}

#[expect(unused)]
pub struct IterDirKeyed<V>(PhantomData<fn() -> V>);

mod iter_dir_keyed {
    use crate::utils::fuse::fuse_dir::FuseDirent;
    use crate::utils::fuse::fuse_dir::FuseDirentName;
    use crate::utils::fuse::fuse_dir::FuseDirents;
    use crate::utils::fuse::fuse_inode::FuseInodeProps;
    use crate::utils::fuse::fuse_view::FuseView;
    use crate::utils::fuse::fuse_views::IterDirKeyed;
    use crate::utils::fuse::fuse_views::IterDirKeyedView;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::type_view::TypeViewExt1;
    use std::rc::Rc;

    impl<T, V> FuseView<T> for IterDirKeyed<V>
    where
        T: GetLiveness,
        V: IterDirKeyedView<T>,
    {
        fn props(_t: &T, _key: u64) -> FuseInodeProps {
            FuseInodeProps::dir()
        }

        fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {
            let (value, key) = V::get(t, key, name)?;
            Some(FuseDirent {
                inode: value.tv_wrap_rc_ref_clone::<V::View>(),
                key,
                static_name: None,
                timeout_ns: V::TIMEOUT_NS,
            })
        }

        fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {
            V::iter(t, key, |name, e, key| {
                dirents.add(
                    V::TIMEOUT_NS,
                    e.tv_wrap_rc_ref::<V::View>(),
                    key,
                    FuseDirentName::Dynamic(name),
                );
            });
        }
    }
}

#[expect(unused)]
pub trait IterDirDynView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    fn iter(t: &Rc<T>, key: u64, f: impl FnMut(&str, FuseInodeWithKey));
    fn get(t: &Rc<T>, key: u64, name: &str) -> Option<FuseInodeWithKey>;
}

#[expect(unused)]
pub struct IterDirDyn<V>(PhantomData<fn() -> V>);

mod iter_dir_dyn {
    use crate::utils::fuse::fuse_dir::FuseDirent;
    use crate::utils::fuse::fuse_dir::FuseDirentName;
    use crate::utils::fuse::fuse_dir::FuseDirents;
    use crate::utils::fuse::fuse_inode::FuseInodeProps;
    use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
    use crate::utils::fuse::fuse_view::FuseView;
    use crate::utils::fuse::fuse_views::IterDirDyn;
    use crate::utils::fuse::fuse_views::IterDirDynView;
    use crate::utils::liveness::GetLiveness;
    use std::rc::Rc;

    impl<T, V> FuseView<T> for IterDirDyn<V>
    where
        T: GetLiveness,
        V: IterDirDynView<T>,
    {
        fn props(_t: &T, _key: u64) -> FuseInodeProps {
            FuseInodeProps::dir()
        }

        fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {
            let FuseInodeWithKey { inode, key } = V::get(&t, key, name)?;
            Some(FuseDirent {
                inode,
                key,
                static_name: None,
                timeout_ns: V::TIMEOUT_NS,
            })
        }

        fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {
            V::iter(&t, key, |name, inode| {
                dirents.add_dyn(V::TIMEOUT_NS, inode, FuseDirentName::Dynamic(name));
            });
        }
    }
}

pub trait CopyHashMapDirView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type Key: Eq + JayHash;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;
    type StringBuf: Reset + Default;

    fn get(t: &T, key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>>;
    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str;
    fn parse_name(key: &str) -> Option<Self::Key>;
}

#[expect(unused)]
pub type CopyHashMapDir<T> = IterDir<copy_hash_map_dir::Dir<T>>;

mod copy_hash_map_dir {
    use crate::utils::fuse::fuse_views::CopyHashMapDirView;
    use crate::utils::fuse::fuse_views::IterDirView;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::reset::Reset;
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub struct Dir<T>(PhantomData<fn() -> T>);

    impl<T, V> IterDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: CopyHashMapDirView<T>,
    {
        const TIMEOUT_NS: u64 = V::TIMEOUT_NS;
        type Value = V::Value;
        type View = V::View;

        fn iter(t: &T, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
            let mut buf = V::StringBuf::default();
            for (k, v) in V::get(t, key).lock().iter() {
                buf.reset();
                let name = V::format_key(&mut buf, k);
                f(name, v);
            }
        }

        fn get(t: &T, key: u64, name: &str) -> Option<Rc<Self::Value>> {
            let id = V::parse_name(name)?;
            V::get(t, key).get(&id)
        }
    }
}

#[expect(unused)]
pub trait HashMapDirView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type BuildHasher: BuildHasher;
    type Key: Eq + JayHash;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;
    type StringBuf: Reset + Default;

    fn get(
        t: &T,
        key: u64,
    ) -> impl GetInner<HashMap<Self::Key, Rc<Self::Value>, Self::BuildHasher>>;
    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str;
    fn parse_name(name: &str) -> Option<Self::Key>;
}

#[expect(unused)]
pub type HashMapDir<V> = IterDir<hash_map_dir::Dir<V>>;

mod hash_map_dir {
    use crate::utils::fuse::fuse_views::HashMapDirView;
    use crate::utils::fuse::fuse_views::IterDirView;
    use crate::utils::get_inner::GetInner;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::reset::Reset;
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<T, V> IterDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: HashMapDirView<T>,
    {
        const TIMEOUT_NS: u64 = V::TIMEOUT_NS;
        type Value = V::Value;
        type View = V::View;

        fn iter(t: &T, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
            let mut buf = V::StringBuf::default();
            for (k, v) in V::get(t, key).get_inner() {
                buf.reset();
                let name = V::format_key(&mut buf, k);
                f(name, v);
            }
        }

        fn get(t: &T, key: u64, name: &str) -> Option<Rc<Self::Value>> {
            let id = V::parse_name(name)?;
            V::get(t, key).get_inner().get(&id).cloned()
        }
    }
}

pub trait CopyHashMapDir2View<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type Key: Eq + Hash + Borrow<Self::KeyRef>;
    type KeyRef: JayHash + Eq + ?Sized;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;
    type StringBuf: Reset + Default;

    fn get(t: &T, key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>>;
    fn format_key(buf: &mut Self::StringBuf, key: &Self::Key, f: impl FnOnce(&str));
    fn parse_name(
        key: &str,
        f: impl FnOnce(&Self::KeyRef) -> Option<Rc<Self::Value>>,
    ) -> Option<Rc<Self::Value>>;
}

#[expect(unused)]
pub type CopyHashMapDir2<T> = IterDir<copy_hash_map_dir2::Dir<T>>;

mod copy_hash_map_dir2 {
    use crate::utils::fuse::fuse_views::CopyHashMapDir2View;
    use crate::utils::fuse::fuse_views::IterDirView;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::reset::Reset;
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub struct Dir<T>(PhantomData<fn() -> T>);

    impl<T, V> IterDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: CopyHashMapDir2View<T>,
    {
        const TIMEOUT_NS: u64 = V::TIMEOUT_NS;
        type Value = V::Value;
        type View = V::View;

        fn iter(t: &T, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
            let mut buf = V::StringBuf::default();
            for (k, v) in V::get(t, key).lock().iter() {
                buf.reset();
                V::format_key(&mut buf, k, |name| {
                    f(name, v);
                });
            }
        }

        fn get(t: &T, key: u64, name: &str) -> Option<Rc<Self::Value>> {
            V::parse_name(name, |id| V::get(t, key).get(id))
        }
    }
}

pub trait BinarySearchMapDirView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type Key: Ord;
    type Value: GetLiveness + 'static;
    type View: FuseView<Self::Value>;
    type StringBuf: Reset + Default;
    type Map: BinarySearchMapDyn<Self::Key, Rc<Self::Value>>;

    fn get(t: &T, key: u64) -> impl GetInner<Self::Map>;
    fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str;
    fn parse_name(name: &str) -> Option<Self::Key>;
}

#[expect(unused)]
pub type BinarySearchMapDir<V> = IterDir<binary_search_map_dir::Dir<V>>;

mod binary_search_map_dir {
    use crate::utils::fuse::fuse_views::BinarySearchMapDirView;
    use crate::utils::fuse::fuse_views::BinarySearchMapDyn;
    use crate::utils::fuse::fuse_views::IterDirView;
    use crate::utils::get_inner::GetInner;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::reset::Reset;
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<T, V> IterDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: BinarySearchMapDirView<T>,
    {
        const TIMEOUT_NS: u64 = V::TIMEOUT_NS;
        type Value = V::Value;
        type View = V::View;

        fn iter(t: &T, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>)) {
            let mut buf = V::StringBuf::default();
            for (k, v) in V::get(t, key).get_inner().iter() {
                buf.reset();
                let name = V::format_key(&mut buf, k);
                f(name, v);
            }
        }

        fn get(t: &T, key: u64, name: &str) -> Option<Rc<Self::Value>> {
            let id = V::parse_name(name)?;
            V::get(t, key).get_inner().get(&id).cloned()
        }
    }
}

pub trait DevTDirView<T>: 'static
where
    T: GetLiveness,
{
    const TIMEOUT_NS: u64 = FUSE_SHORT_TIMEOUT;
    type D: GetLiveness + 'static;
    type V: FuseView<Self::D>;

    fn devs(t: &T) -> &CopyHashMap<c::dev_t, Rc<Self::D>>;
}

#[expect(unused)]
pub type DevTDir<V> = CopyHashMapDir<dev_t_dir::Dir<V>>;

mod dev_t_dir {
    use crate::utils::copyhashmap::CopyHashMap;
    use crate::utils::fuse::fuse_views::CopyHashMapDirView;
    use crate::utils::fuse::fuse_views::DevTDirView;
    use crate::utils::liveness::GetLiveness;
    use crate::utils::major_minor::MajorMinor;
    use crate::utils::major_minor::major_minor;
    use arrayvec::ArrayString;
    use std::marker::PhantomData;
    use std::rc::Rc;
    use std::str::FromStr;
    use uapi::c;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<T, V> CopyHashMapDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: DevTDirView<T>,
    {
        const TIMEOUT_NS: u64 = V::TIMEOUT_NS;
        type Key = c::dev_t;
        type Value = V::D;
        type View = V::V;
        type StringBuf = ArrayString<41>;

        fn get(t: &T, _key: u64) -> &CopyHashMap<Self::Key, Rc<Self::Value>> {
            V::devs(t)
        }

        fn format_key<'a>(buf: &'a mut Self::StringBuf, key: &Self::Key) -> &'a str {
            let mut tmp = itoa::Buffer::new();
            let MajorMinor { major, minor } = major_minor(*key);
            buf.push_str(tmp.format(major));
            buf.push_str(":");
            buf.push_str(tmp.format(minor));
            buf
        }

        fn parse_name(key: &str) -> Option<Self::Key> {
            let (major, minor) = key.split_once(":")?;
            let major = u64::from_str(major).ok()?;
            let minor = u64::from_str(minor).ok()?;
            Some(uapi::makedev(major, minor))
        }
    }
}

/*
pub struct FuseLinearInodeWrapper<L>(PhantomData<fn() -> L>);

#[expect(unused)]
pub trait FuseLinearInodeExt {
    fn create_linear_inode<L>(self: &Rc<Self>) -> &Rc<TypeView<Self, FuseLinearInodeWrapper<L>>>;
    fn into_linear_inode<L>(self: Rc<Self>) -> Rc<TypeView<Self, FuseLinearInodeWrapper<L>>>;
}

impl<T> FuseLinearInodeExt for T {
    fn create_linear_inode<L>(self: &Rc<Self>) -> &Rc<TypeView<Self, FuseLinearInodeWrapper<L>>> {
        self.tv_wrap_rc_ref()
    }

    fn into_linear_inode<L>(self: Rc<Self>) -> Rc<TypeView<Self, FuseLinearInodeWrapper<L>>> {
        self.tv_wrap_rc()
    }
}

#[cfg_attr(not(test), expect(unused))]
pub trait FuseLinearView<T>: Linearize + Sized {
    fn props(self, t: &T) -> FuseInodeProps;
    fn lookup(self, t: Rc<T>, name: &str) -> Option<FuseDirent> {
        let _ = t;
        let _ = name;
        None
    }
    fn getdents(self, t: Rc<T>, dirents: &mut FuseDirents) {
        let _ = t;
        let _ = dirents;
    }
    fn read(self, t: &T, buf: &mut String, ctx: &StrCtx) {
        let _ = t;
        let _ = buf;
        let _ = ctx;
    }
    fn readlink(self, t: &T, depth: u64, buf: &mut String) {
        let _ = t;
        let _ = depth;
        let _ = buf;
    }
}

impl<T, L> FuseView<T> for FuseLinearInodeWrapper<L>
where
    T: 'static,
    L: FuseLinearView<T> + 'static,
{
    fn props(t: &T, key: u64) -> FuseInodeProps {
        if let Some(l) = L::from_linear(key as usize) {
            l.props(t)
        } else {
            FuseInodeProps::reg()
        }
    }

    fn lookup(t: Rc<T>, key: u64, name: &str) -> Option<FuseDirent> {
        if let Some(l) = L::from_linear(key as usize) {
            l.lookup(t, name)
        } else {
            None
        }
    }

    fn getdents(t: Rc<T>, key: u64, dirents: &mut FuseDirents) {
        if let Some(l) = L::from_linear(key as usize) {
            l.getdents(t, dirents)
        }
    }

    fn read(t: &T, key: u64, buf: &mut String, ctx: &StrCtx) {
        if let Some(l) = L::from_linear(key as usize) {
            l.read(t, buf, ctx)
        }
    }

    fn readlink(t: &T, key: u64, depth: u64, buf: &mut String) {
        if let Some(l) = L::from_linear(key as usize) {
            l.readlink(t, depth, buf)
        }
    }
}
 */
