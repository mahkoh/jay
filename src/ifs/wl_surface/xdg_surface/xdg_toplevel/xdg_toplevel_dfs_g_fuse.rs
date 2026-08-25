/*
dir xdg_toplevel {
    @inherit dfs_toplevel_node,
    @inherit dfs_object,
    tag: reg,
    states: reg,
    decoration: reg,
    is_mapped: reg,
    committed: reg,
    extents_set: reg,
    allow_fixed_size: reg,
    has_drag: reg,
    has_dialog: reg,
    num_children: reg,
    has_parent: reg,
    xdg_surface: link,
    surface: link,
    parent: link (opt, key = 0),
    children: view (key = 0),
}
 */
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsObjectLinkDir;
use crate::dfs::dfs_helpers::DfsObjectLinkDirView;
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::Decoration;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_dfs_g_fuse::generated::xdg_toplevel;
use crate::object::Object;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::wire::ObjectId;
use crate::wire::XdgToplevelId;
use std::rc::Rc;

impl XdgToplevel {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<xdg_toplevel::View>().without_key()
    }
}

impl xdg_toplevel::Dir for XdgToplevel {
    type ViewChildren = DfsObjectLinkDir<Children>;

    fn read_tag(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data.tag.borrow().as_str().str_fmt(buf, ctx);
    }

    fn read_states(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.states.get().str_fmt(buf, ctx);
    }

    fn read_decoration(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let decoration = match self.decoration.get() {
            Decoration::Client => "client",
            Decoration::Server => "server",
        };
        decoration.str_fmt(buf, ctx);
    }

    fn read_is_mapped(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.is_mapped.get().str_fmt(buf, ctx);
    }

    fn read_committed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.committed.get().str_fmt(buf, ctx);
    }

    fn read_extents_set(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.extents_set.get().str_fmt(buf, ctx);
    }

    fn read_allow_fixed_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.allow_fixed_size.get().str_fmt(buf, ctx);
    }

    fn read_has_drag(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.drag.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_dialog(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dialog.get().is_some().str_fmt(buf, ctx);
    }

    fn read_num_children(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.children.borrow().len().str_fmt(buf, ctx);
    }

    fn read_has_parent(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.parent.get().is_some().str_fmt(buf, ctx);
    }

    fn readlink_xdg_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.xdg.surface.client.id, self.xdg.id());
    }

    fn readlink_surface(&self, depth: u64, buf: &mut String) {
        format_object_link(buf, depth, self.xdg.surface.client.id, self.xdg.surface.id);
    }

    fn has_parent(&self) -> bool {
        self.parent.is_some()
    }

    fn readlink_parent(&self, depth: u64, buf: &mut String) {
        if let Some(parent) = self.parent.get() {
            format_object_link(buf, depth, parent.xdg.surface.client.id, parent.id);
        }
    }
}

struct Children;

impl DfsObjectLinkDirView<XdgToplevel> for Children {
    fn client(t: &Rc<XdgToplevel>) -> &Rc<Client> {
        &t.xdg.surface.client
    }

    fn iter(t: &Rc<XdgToplevel>, _key: u64, mut f: impl FnMut(ObjectId)) {
        for id in t.children.borrow().keys() {
            f((*id).into());
        }
    }

    fn contains(t: &XdgToplevel, _key: u64, id: ObjectId) -> bool {
        t.children
            .borrow()
            .contains_key::<XdgToplevelId>(&id.into())
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_2af527bea63d0dae841a59ac3baf16b86b57fd8cbe208989ef0c93d08c6333ad.rs",
));
// FUSE GENERATED STOP
