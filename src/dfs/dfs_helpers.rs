use crate::client::Client;
use crate::client::ClientId;
use crate::globals::GlobalName;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceNode;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::copyhashmap::LockableRandomState;
use crate::utils::fuse::fuse_views::FuseLinkView;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::liveness::GetLiveness;
use crate::utils::markers::JayHash;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::wire::ObjectId;
use std::rc::Rc;

pub fn format_object_link(buf: &mut String, depth: u64, client: ClientId, id: impl Into<ObjectId>) {
    format_object_link_(buf, depth, client, id.into());
}

fn format_object_link_(buf: &mut String, depth: u64, client: ClientId, id: ObjectId) {
    format_client_link(buf, depth, client);
    buf.push_str("/objects/");
    buf.push_str(itoa::Buffer::new().format(id.raw()));
}

pub fn format_output_link(buf: &mut String, depth: u64, global: &WlOutputGlobal) {
    format_path_link(buf, depth, "outputs", &global.connector.name);
}

pub fn format_workspace_link(buf: &mut String, depth: u64, ws: &WorkspaceNode) {
    format_path_link(buf, depth, "workspaces", &ws.name);
}

pub fn format_client_link(buf: &mut String, depth: u64, id: ClientId) {
    write_root_link(buf, depth);
    buf.push_str("clients/");
    buf.push_str(itoa::Buffer::new().format(id.raw()));
}

pub fn format_path_link(buf: &mut String, depth: u64, category: &str, name: &str) {
    write_root_link(buf, depth);
    buf.push_str(category);
    buf.push_str("/");
    buf.push_str(name);
}

pub fn write_root_link(buf: &mut String, depth: u64) {
    for _ in 1..depth {
        buf.push_str("../");
    }
}

pub struct DfsObjectLink;

impl FuseLinkView<Client> for DfsObjectLink {
    fn readlink(t: &Client, key: u64, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, t.id, ObjectId::from_raw(key));
    }
}

pub fn write_global_link(buf: &mut String, depth: u64, name: GlobalName) {
    write_root_link(buf, depth);
    buf.push_str("globals/registry/");
    buf.push_str(itoa::Buffer::new().format(name.raw()));
}

pub struct DfsGlobalLink;

impl FuseLinkView<()> for DfsGlobalLink {
    fn readlink(_t: &(), key: u64, depth: u64, buf: &mut String) {
        write_global_link(buf, depth, GlobalName::from_raw(key as u32));
    }
}

pub trait DfsObjectLinkDirView<T>: 'static
where
    T: GetLiveness,
{
    fn client(t: &Rc<T>) -> &Rc<Client>;
    fn iter(t: &Rc<T>, key: u64, f: impl FnMut(ObjectId));
    fn contains(t: &T, key: u64, id: ObjectId) -> bool;
}

pub type DfsObjectLinkDir<V> = IterDirKeyed<dfs_object_link_dir::Dir<V>>;

mod dfs_object_link_dir {
    use crate::client::Client;
    use crate::dfs::dfs_helpers::DfsObjectLink;
    use crate::dfs::dfs_helpers::DfsObjectLinkDirView;
    use crate::utils::fuse::fuse_views::FuseLink;
    use crate::utils::fuse::fuse_views::IterDirKeyedView;
    use crate::utils::liveness::GetLiveness;
    use crate::wire::ObjectId;
    use std::marker::PhantomData;
    use std::rc::Rc;
    use std::str::FromStr;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<T, V> IterDirKeyedView<T> for Dir<V>
    where
        T: GetLiveness,
        V: DfsObjectLinkDirView<T>,
    {
        type Value = Client;
        type View = FuseLink<DfsObjectLink>;

        fn iter(t: Rc<T>, key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
            let client = V::client(&t);
            let mut buf = itoa::Buffer::new();
            V::iter(&t, key, |id| {
                f(buf.format(id.raw()), client, id.raw());
            });
        }

        fn get(t: Rc<T>, key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
            let id = u64::from_str(name).ok()?;
            let ok = V::contains(&t, key, ObjectId::from_raw(id));
            if !ok {
                return None;
            }
            Some((V::client(&t).clone(), id))
        }
    }
}

pub trait DfsObjectCopyHashMap {
    type Key: Copy + Eq + JayHash + Into<ObjectId> + From<ObjectId>;
    type Value;
    type RandomState: LockableRandomState;

    fn get(&self) -> &CopyHashMap<Self::Key, Self::Value, Self::RandomState>;
}

impl<K, V, R> DfsObjectCopyHashMap for CopyHashMap<K, V, R>
where
    K: Copy + Eq + JayHash + Into<ObjectId> + From<ObjectId>,
    R: LockableRandomState,
{
    type Key = K;
    type Value = V;
    type RandomState = R;

    #[inline(always)]
    fn get(&self) -> &CopyHashMap<Self::Key, Self::Value, Self::RandomState> {
        self
    }
}

pub trait DfsCopyHashMapObjectLinkDirView<T>: 'static
where
    T: GetLiveness,
{
    fn client(t: &T) -> &Rc<Client>;
    fn map(t: &T, key: u64) -> &impl DfsObjectCopyHashMap;
}

pub type DfsCopyHashMapObjectLinkDir<V> =
    DfsObjectLinkDir<dfs_copy_hash_map_object_link_dir::Dir<V>>;

mod dfs_copy_hash_map_object_link_dir {
    use crate::client::Client;
    use crate::dfs::dfs_helpers::DfsCopyHashMapObjectLinkDirView;
    use crate::dfs::dfs_helpers::DfsObjectCopyHashMap;
    use crate::dfs::dfs_helpers::DfsObjectLinkDirView;
    use crate::utils::liveness::GetLiveness;
    use crate::wire::ObjectId;
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<T, V> DfsObjectLinkDirView<T> for Dir<V>
    where
        T: GetLiveness,
        V: DfsCopyHashMapObjectLinkDirView<T>,
    {
        fn client(t: &Rc<T>) -> &Rc<Client> {
            V::client(t)
        }

        fn iter(t: &Rc<T>, key: u64, mut f: impl FnMut(ObjectId)) {
            for id in V::map(t, key).get().lock().keys() {
                f((*id).into());
            }
        }

        fn contains(t: &T, key: u64, id: ObjectId) -> bool {
            V::map(t, key).get().contains(&id.into())
        }
    }
}

pub fn dfs_split_view<T>(dst: &mut String, ctx: &StrCtx<'_>, mut f: impl FnMut(TreeTimeline) -> T)
where
    T: StrFmt,
{
    ctx.struct_prefix(dst);
    ctx.struct_field(dst, "live", &f(LiveTL), true);
    ctx.struct_field(dst, "rndr", &f(RenderTL), false);
    ctx.struct_suffix(dst);
}

pub trait ClientObjectDirView<T>: 'static
where
    T: GetLiveness,
{
    type ObjectId: JayHash + Eq + Copy + Into<ObjectId> + From<ObjectId>;
    type Value;
    type RandomState: LockableRandomState;

    fn get(t: &T) -> &CopyHashMap<(ClientId, Self::ObjectId), Rc<Self::Value>, Self::RandomState>;
    fn client(v: &Self::Value) -> &Rc<Client>;
}

pub type ClientObjectDir<V> = IterDirKeyed<client_object_dir::Dir<V>>;

mod client_object_dir {
    use crate::client::Client;
    use crate::client::ClientId;
    use crate::dfs::dfs_helpers::ClientObjectDirView;
    use crate::dfs::dfs_helpers::DfsObjectLink;
    use crate::utils::fuse::fuse_views::FuseLink;
    use crate::utils::fuse::fuse_views::IterDirKeyedView;
    use crate::utils::liveness::GetLiveness;
    use crate::wire::ObjectId;
    use arrayvec::ArrayString;
    use std::marker::PhantomData;
    use std::rc::Rc;
    use std::str::FromStr;

    pub struct Dir<V>(PhantomData<fn() -> V>);

    impl<V, T> IterDirKeyedView<T> for Dir<V>
    where
        T: GetLiveness,
        V: ClientObjectDirView<T>,
    {
        type Value = Client;
        type View = FuseLink<DfsObjectLink>;

        fn iter(t: Rc<T>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
            let mut tmp = itoa::Buffer::new();
            let mut buf = ArrayString::<41>::new();
            for ((a, b), c) in V::get(&t).lock().iter() {
                let b: ObjectId = (*b).into();
                buf.clear();
                buf.push_str(tmp.format(a.raw()));
                buf.push_str(":");
                buf.push_str(tmp.format(b.raw()));
                f(&buf, V::client(c), b.raw());
            }
        }

        fn get(t: Rc<T>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
            let (c, o) = name.split_once(":")?;
            let client = u64::from_str(c).ok()?;
            let object = u64::from_str(o).ok()?;
            let obj = V::get(&t).get(&(
                ClientId::from_raw(client),
                ObjectId::from_raw(object).into(),
            ))?;
            Some((V::client(&obj).clone(), object))
        }
    }
}
