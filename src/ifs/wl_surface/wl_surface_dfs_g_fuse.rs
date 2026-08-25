/*
dir wl_surface {
    @inherit dfs_node,
    @inherit dfs_object,
    role: reg,
    role_obj: link (opt),
    buffer: link (opt),
    subsurfaces: view (key = 0),
    input_popups: view (key = 0),
}
*/
use crate::client::Client;
use crate::dfs::dfs_helpers::DfsObjectLink;
use crate::dfs::dfs_helpers::format_object_link;
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::wl_surface_dfs_g_fuse::generated::wl_surface;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::fuse::fuse_views::IterDirKeyedView;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use crate::wire::WlSurfaceId;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

impl WlSurface {
    pub(super) fn debugfs_obj(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<wl_surface::View>().without_key()
    }
}

impl wl_surface::Dir for WlSurface {
    type ViewSubsurfaces = IterDirKeyed<Subsurfaces>;
    type ViewInputPopups = IterDirKeyed<InputPopups>;

    fn read_role(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.role.get().name().str_fmt(buf, ctx);
    }

    fn has_role_obj(&self) -> bool {
        self.ext.get().object_id().is_some()
    }

    fn readlink_role_obj(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.ext.get().object_id() {
            format_object_link(buf, depth, self.client.id, t);
        }
    }

    fn has_buffer(&self) -> bool {
        self.buffer.is_some()
    }

    fn readlink_buffer(&self, depth: u64, buf: &mut String) {
        if let Some(t) = self.buffer.get() {
            format_object_link(buf, depth, self.client.id, t.buffer.buf.id);
        }
    }
}

struct Subsurfaces;

impl IterDirKeyedView<WlSurface> for Subsurfaces {
    type Value = Client;
    type View = FuseLink<DfsObjectLink>;

    fn iter(t: Rc<WlSurface>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let children = t.children.borrow();
        let Some(children) = children.deref() else {
            return;
        };
        let mut buf = itoa::Buffer::new();
        for (id, child) in &children.subsurfaces {
            f(buf.format(id.raw()), &child.surface.client, id.raw() as _);
        }
    }

    fn get(t: Rc<WlSurface>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let id = u64::from_str(name).ok()?;
        let children = t.children.borrow();
        let child = children
            .deref()
            .as_ref()?
            .subsurfaces
            .get(&WlSurfaceId::from_raw(id))?;
        Some((child.surface.client.clone(), id))
    }
}

struct InputPopups;

impl IterDirKeyedView<WlSurface> for InputPopups {
    type Value = Client;
    type View = FuseLink<DfsObjectLink>;

    fn iter(t: Rc<WlSurface>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let mut buf = itoa::Buffer::new();
        for (_, con) in &t.text_input_connections {
            for (_, popup) in con.input_method.popups() {
                f(
                    buf.format(popup.surface.id.raw()),
                    &popup.surface.client,
                    popup.surface.id.raw() as _,
                );
            }
        }
    }

    fn get(t: Rc<WlSurface>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        let id = u64::from_str(name).ok()?;
        for (_, con) in &t.text_input_connections {
            for (_, popup) in con.input_method.popups() {
                if popup.surface.id.raw() == id {
                    return Some((popup.surface.client.clone(), id));
                }
            }
        }
        None
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_3d078f88ba584bcd1f20e8fb395ab65a37f2f2f050ce5dd2cc7c22f3f7efe731.rs",
));
// FUSE GENERATED STOP
