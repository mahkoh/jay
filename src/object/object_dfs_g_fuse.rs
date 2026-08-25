/*
dir dfs_object (abstract, global) {
    object_id: reg,
    object_interface: reg,
    object_version: reg,
}

dir generic_object {
    object_id: view (no_timeout),
    object_interface: view (no_timeout),
    object_version: view (no_timeout),
}
 */
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsObjectLink;
use crate::object::Object;
use crate::object::object_dfs_g_fuse::generated::generic_object;
use crate::utils::fuse::fuse_globals::dfs_object;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::FuseReg;
use crate::utils::fuse::fuse_views::FuseRegView;
use crate::utils::liveness::GetLiveness;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::wire::ObjectId;
use std::rc::Rc;

impl Client {
    pub fn generic_object_view(self: &Rc<Self>, id: ObjectId) -> FuseInodeWithKey {
        self.tv_wrap_rc_ref_clone::<generic_object::View>()
            .with_key(id.raw() as u64)
    }

    pub fn object_link(self: &Rc<Self>, id: ObjectId) -> FuseInodeWithKey {
        self.tv_wrap_rc_ref_clone::<FuseLink<DfsObjectLink>>()
            .with_key(id.raw() as u64)
    }
}

impl<T> dfs_object::Dir for T
where
    T: Object + GetLiveness,
{
    fn read_object_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id().raw().str_fmt(buf, ctx);
    }

    fn read_object_interface(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.interface().name().str_fmt(buf, ctx);
    }

    fn read_object_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.version().0.str_fmt(buf, ctx);
    }
}

impl generic_object::Dir for Client {
    type ViewObjectId = FuseReg<GenericObjectId>;
    type ViewObjectInterface = FuseReg<GenericObjectInterface>;
    type ViewObjectVersion = FuseReg<GenericObjectVersion>;

    fn keyof_object_id(&self, key: u64) -> u64 {
        key
    }

    fn keyof_object_interface(&self, key: u64) -> u64 {
        key
    }

    fn keyof_object_version(&self, key: u64) -> u64 {
        key
    }
}

struct GenericObjectId;

impl FuseRegView<Client> for GenericObjectId {
    fn read(_t: &Client, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        key.str_fmt(buf, ctx);
    }
}

struct GenericObjectInterface;

impl FuseRegView<Client> for GenericObjectInterface {
    fn read(t: &Client, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        let Ok(obj) = t.objects.get_obj(ObjectId::from_raw(key)) else {
            return;
        };
        obj.interface().name().str_fmt(buf, ctx);
    }
}

struct GenericObjectVersion;

impl FuseRegView<Client> for GenericObjectVersion {
    fn read(t: &Client, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        let Ok(obj) = t.objects.get_obj(ObjectId::from_raw(key)) else {
            return;
        };
        obj.version().0.str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_28877255ddf122e7e59f2320d1b07ed8c48ad1eb24663c25f1bd9062e6c2405b.rs",
));
// FUSE GENERATED STOP
