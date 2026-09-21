/*
dir wl_buffer {
    @inherit dfs_object,
    ty: reg,
    destroyed: reg,
    width: reg,
    height: reg,
    rect: reg,
    format: reg,
    color: reg (opt),
    render_ctx_version: reg,
    had_buffer_texture: reg,
    dmabuf_device: reg (opt, no_timeout),
    dmabuf_device_is_exclusive: reg (opt, no_timeout),
    storage: reg,
    shm_storage: view (opt, key = 0, no_timeout),
    dmabuf_storage: view (opt, key = 0, no_timeout),
    client_dmabuf: reg (opt, no_timeout),
}

dir shm_storage {
    offset: reg,
    size: reg,
    stride: reg,
    sealed_memfd: reg,
    has_udmabuf: reg,
    udmabuf_offset: reg,
    udmabuf_size: reg,
    udmabuf_impossible: reg,
    has_host_buffer: reg,
    host_buffer_impossible: reg,
    has_texture: reg,
    texture_impossible: reg,
}

dir dmabuf_storage {
    dmabuf: reg,
    has_texture: reg,
    has_framebuffer: reg,
    copy_object: reg,
}
 */
use crate::ifs::wl_buffer::Ty;
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_buffer::WlBufferStorage;
use crate::ifs::wl_buffer::wl_buffer_dfs_g_fuse::generated::dmabuf_storage;
use crate::ifs::wl_buffer::wl_buffer_dfs_g_fuse::generated::shm_storage;
use crate::ifs::wl_buffer::wl_buffer_dfs_g_fuse::generated::wl_buffer;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::major_minor::major_minor;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use std::rc::Rc;

impl WlBuffer {
    pub(super) fn debugfs(self: Rc<Self>) -> FuseInodeWithKey {
        self.tv_wrap_rc::<wl_buffer::View>().without_key()
    }
}

impl wl_buffer::Dir for WlBuffer {
    type ViewShmStorage = shm_storage::View;
    type ViewDmabufStorage = dmabuf_storage::View;

    fn read_ty(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let ty = match self.ty {
            Ty::Shm => "shm",
            Ty::DmaBuf => "dmabuf",
            Ty::Spb => "single_pixel",
        };
        ty.str_fmt(buf, ctx);
    }

    fn read_destroyed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.destroyed.get().str_fmt(buf, ctx);
    }

    fn read_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.width.str_fmt(buf, ctx);
    }

    fn read_height(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.height.str_fmt(buf, ctx);
    }

    fn read_rect(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.rect.str_fmt(buf, ctx);
    }

    fn read_format(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.format.str_fmt(buf, ctx);
    }

    fn has_color(&self) -> bool {
        self.color.is_some()
    }

    fn read_color(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = &self.color {
            v.str_fmt(buf, ctx);
        }
    }

    fn read_render_ctx_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.render_ctx_version.get().str_fmt(buf, ctx);
    }

    fn read_had_buffer_texture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.had_buffer_texture.get().str_fmt(buf, ctx);
    }

    fn has_dmabuf_device(&self) -> bool {
        self.client_dmabuf_device.is_some()
    }

    fn read_dmabuf_device(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(dev) = &self.client_dmabuf_device {
            major_minor(dev.dev_t).str_fmt(buf, ctx);
        }
    }

    fn has_dmabuf_device_is_exclusive(&self) -> bool {
        self.client_dmabuf.is_some()
    }

    fn read_dmabuf_device_is_exclusive(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.exclusive_device.is_some().str_fmt(buf, ctx);
    }

    fn read_storage(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let storage = match &*self.storage.borrow() {
            None => "none",
            Some(WlBufferStorage::Shm { .. }) => "shm",
            Some(WlBufferStorage::Dmabuf(_)) => "dmabuf",
        };
        storage.str_fmt(buf, ctx);
    }

    fn has_shm_storage(&self, _key: u64) -> bool {
        matches!(&*self.storage.borrow(), Some(WlBufferStorage::Shm { .. }))
    }

    fn has_dmabuf_storage(&self, _key: u64) -> bool {
        matches!(&*self.storage.borrow(), Some(WlBufferStorage::Dmabuf(_)))
    }

    fn has_client_dmabuf(&self) -> bool {
        self.client_dmabuf.is_some()
    }

    fn read_client_dmabuf(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = &self.client_dmabuf {
            v.str_fmt(buf, ctx);
        }
    }
}

macro_rules! shm {
    ($slf:expr, $mem:pat, $stride:pat, $params:pat, $body:expr) => {
        if let Some(WlBufferStorage::Shm {
            mem: $mem,
            stride: $stride,
            dmabuf_buffer_params: $params,
        }) = &*$slf.storage.borrow()
        {
            $body
        }
    };
}

impl shm_storage::Dir for WlBuffer {
    fn read_offset(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, mem, _, _, mem.offset().str_fmt(buf, ctx));
    }

    fn read_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, _, params, params.size.str_fmt(buf, ctx));
    }

    fn read_stride(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, stride, _, stride.str_fmt(buf, ctx));
    }

    fn read_sealed_memfd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(
            self,
            mem,
            _,
            _,
            mem.pool().is_sealed_memfd().str_fmt(buf, ctx)
        );
    }

    fn read_has_udmabuf(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(
            self,
            _,
            _,
            params,
            params.udmabuf.is_some().str_fmt(buf, ctx)
        );
    }

    fn read_udmabuf_offset(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, _, params, params.udmabuf_offset.str_fmt(buf, ctx));
    }

    fn read_udmabuf_size(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, _, params, params.udmabuf_size.str_fmt(buf, ctx));
    }

    fn read_udmabuf_impossible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(
            self,
            _,
            _,
            params,
            params.udmabuf_impossible.str_fmt(buf, ctx)
        );
    }

    fn read_has_host_buffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(
            self,
            _,
            _,
            params,
            params.host_buffer.is_some().str_fmt(buf, ctx)
        );
    }

    fn read_host_buffer_impossible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(
            self,
            _,
            _,
            params,
            params.host_buffer_impossible.str_fmt(buf, ctx)
        );
    }

    fn read_has_texture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, _, params, params.tex.is_some().str_fmt(buf, ctx));
    }

    fn read_texture_impossible(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        shm!(self, _, _, params, params.tex_impossible.str_fmt(buf, ctx));
    }
}

macro_rules! dmabuf_storage {
    ($slf:expr, $storage:pat, $body:expr) => {
        if let Some(WlBufferStorage::Dmabuf($storage)) = &*$slf.storage.borrow() {
            $body
        }
    };
}

impl dmabuf_storage::Dir for WlBuffer {
    fn read_dmabuf(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dmabuf_storage!(self, s, s.dmabuf.str_fmt(buf, ctx));
    }

    fn read_has_texture(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dmabuf_storage!(self, s, s.tex.is_some().str_fmt(buf, ctx));
    }

    fn read_has_framebuffer(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dmabuf_storage!(self, s, s.fb.is_some().str_fmt(buf, ctx));
    }

    fn read_copy_object(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        dmabuf_storage!(
            self,
            s,
            match &s.copy_obj {
                None => "not queried",
                Some(None) => "none",
                Some(Some(_)) => "some",
            }
            .str_fmt(buf, ctx)
        );
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8b91a6dac50768623cab19ffe8ee101561f7aa63e020634152381b318f7e2326.rs",
));
// FUSE GENERATED STOP
