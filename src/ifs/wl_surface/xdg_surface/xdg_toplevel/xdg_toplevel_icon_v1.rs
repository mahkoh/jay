use {
    crate::{
        client::{Client, ClientError},
        cmm::{cmm_eotf::Eotf, cmm_render_intent::RenderIntent},
        gfx_api::{
            AcquireSync, AlphaMode, AsyncShmGfxTextureCallback, CopyTexture, FramebufferRect,
            GfxApiOp, GfxContext, GfxError, GfxRenderPass, GfxTexture, PendingShmTransfer,
            ReleaseSync, STAGING_UPLOAD, SampleRect,
        },
        ifs::{
            wl_buffer::{WlBuffer, WlBufferStorage},
            wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        },
        io_uring::{PendingPoll, PollCallback},
        leaks::Tracker,
        object::{Object, Version},
        rect::{Rect, Region},
        scale::Scale,
        state::State,
        theme::Color,
        utils::{
            clonecell::UnsafeCellCloneSafe, copyhashmap::CopyHashMap, errorfmt::ErrorFmt,
            numcell::NumCell, obj_and_id::ObjWithId, oserror::OsError, smallmap::SmallMap,
        },
        video::dmabuf::PlaneVec,
        wire::{XdgToplevelIconV1Id, XdgToplevelId, xdg_toplevel_icon_v1::*},
    },
    ahash::{AHashMap, AHashSet},
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        ffi::c_short,
        rc::{Rc, Weak},
    },
    thiserror::Error,
};

linear_ids!(ToplevelIconIds, ToplevelIconId, u64);

pub struct XdgToplevelIconV1 {
    pub id: XdgToplevelIconV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub immutable: Cell<bool>,
    pub toplevel_icon_id: ToplevelIconId,
    pub toplevels: CopyHashMap<XdgToplevelId, Rc<XdgToplevel>>,
    considered_sizes: Cell<Option<(i32, u64)>>,
    buffers: CopyHashMap<BufferKey, Rc<WlBuffer>>,
    pending: CopyHashMap<BufferKey, AsyncOp>,
    buf_key_to_icon_key: RefCell<AHashMap<BufferKey, SmallVec<[IconKey; 2]>>>,
    icons: CopyHashMap<IconKey, ToplevelIcon>,
}

pub struct ToplevelIconUser {
    size: Cell<i32>,
    icons: SmallMap<Scale, ToplevelIcon, 2>,
}

#[derive(Clone)]
pub enum ToplevelIcon {
    Srgb(Color),
    Tex(Rc<dyn GfxTexture>),
}

unsafe impl UnsafeCellCloneSafe for ToplevelIcon {}

impl ToplevelIconUser {
    pub fn new(size: i32) -> Self {
        Self {
            size: Cell::new(size),
            icons: Default::default(),
        }
    }

    pub fn clear(&self) {
        self.icons.clear();
    }

    pub fn set_size(&self, size: i32) -> bool {
        self.size.replace(size) != size
    }

    pub fn get(&self, scale: Scale) -> Option<ToplevelIcon> {
        self.icons.get(&scale)
    }
}

impl State {
    pub fn toplevel_icon_user(&self) -> ToplevelIconUser {
        ToplevelIconUser::new(self.theme.title_icon_size())
    }
}

impl ObjWithId for Rc<XdgToplevelIconV1> {
    type Id = ToplevelIconId;

    fn id(&self) -> Self::Id {
        self.toplevel_icon_id
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct IconKey {
    size: i32,
    scale: Scale,
}

struct AsyncData {
    key: BufferKey,
    parent: Weak<XdgToplevelIconV1>,
    tex: Rc<dyn GfxTexture>,
    poll_waits: NumCell<u8>,
}

enum AsyncOp {
    Polls(#[expect(dead_code)] PlaneVec<PendingPoll>),
    Transfer(#[expect(dead_code)] PendingShmTransfer),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct BufferKey {
    size: i32,
    scale: i32,
}

impl XdgToplevelIconV1 {
    pub fn new(id: XdgToplevelIconV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            immutable: Default::default(),
            toplevel_icon_id: client.state.toplevel_icon_ids.next(),
            considered_sizes: Default::default(),
            buffers: Default::default(),
            toplevels: Default::default(),
            pending: Default::default(),
            buf_key_to_icon_key: RefCell::new(Default::default()),
            icons: Default::default(),
        }
    }

    fn check_immutable(&self) -> Result<(), XdgToplevelIconV1Error> {
        if self.immutable.get() {
            return Err(XdgToplevelIconV1Error::Immutable);
        }
        Ok(())
    }

    pub fn handle_render_ctx_change(self: &Rc<Self>) {
        self.icons.clear();
        self.pending.clear();
        self.considered_sizes.take();
        self.update_sizes();
    }

    pub fn update_sizes(self: &Rc<Self>) {
        if !self.immutable.get() {
            return;
        }
        let state = &self.client.state;
        let Some(ctx) = state.render_ctx.get() else {
            return;
        };
        let th = state.theme.title_icon_size();
        let cs = Some((th, state.scales.version()));
        if self.considered_sizes.replace(cs) == cs {
            return;
        }
        self.icons.clear();
        self.pending.clear();
        if th > 0 {
            let buf_keys = self.compute_buf_keys();
            self.create_textures(&ctx, buf_keys);
        }
        self.pending_changed();
    }

    fn compute_buf_keys(&self) -> AHashSet<BufferKey> {
        let state = &self.client.state;
        let buf_to_icon = &mut *self.buf_key_to_icon_key.borrow_mut();
        buf_to_icon.clear();
        let mut buf_keys = AHashSet::new();
        let th = state.theme.title_icon_size();
        for &(scale, _) in &*state.scales.lock() {
            let [buffer_th] = scale.pixel_size([th]);
            let scalef = scale.to_f64();
            #[derive(Copy, Clone, PartialOrd, PartialEq)]
            enum Mismatch {
                Same,
                Greater(f64),
                Smaller(f64),
            }
            #[derive(Copy, Clone, PartialOrd, PartialEq)]
            struct Quality {
                size: Mismatch,
                scale: Mismatch,
            }
            let mut best_quality = None::<Quality>;
            let mut best = None::<BufferKey>;
            for &key in self.buffers.lock().keys() {
                let mut quality = Quality {
                    size: Mismatch::Same,
                    scale: Mismatch::Same,
                };
                if key.size > buffer_th {
                    quality.size = Mismatch::Greater((key.size - buffer_th) as f64);
                } else if key.size < buffer_th {
                    quality.size = Mismatch::Smaller((buffer_th - key.size) as f64);
                }
                let key_scale = key.scale as f64;
                if key_scale > scalef {
                    quality.scale = Mismatch::Greater((key_scale - scalef) as f64);
                } else if key_scale < scalef {
                    quality.scale = Mismatch::Smaller((scalef - key_scale) as f64);
                }
                if let Some(old) = best_quality
                    && old < quality
                {
                    continue;
                }
                best_quality = Some(quality);
                best = Some(key);
            }
            let Some(key) = best else {
                continue;
            };
            buf_keys.insert(key);
            buf_to_icon
                .entry(key)
                .or_default()
                .push(IconKey { size: th, scale });
        }
        buf_keys
    }

    fn create_textures(self: &Rc<Self>, ctx: &Rc<dyn GfxContext>, buf_keys: AHashSet<BufferKey>) {
        let state = &self.client.state;
        let fast_ram_access = ctx.fast_ram_access();
        'outer: for key in buf_keys {
            let Some(buf) = self.buffers.get(&key) else {
                continue;
            };
            let storage = &mut *buf.storage.borrow_mut();
            let Some(storage) = storage else {
                if let Some([r, g, b, a]) = buf.color {
                    let color = Color::from_u32(
                        Eotf::Gamma22,
                        AlphaMode::PremultipliedElectrical,
                        r,
                        g,
                        b,
                        a,
                    );
                    self.set_color(key, color);
                }
                continue;
            };
            let (mem, &mut stride, dmabuf_buffer_params) = match storage {
                WlBufferStorage::Dmabuf(storage) => {
                    let tex = match storage.ensure_tex(&buf, ctx) {
                        Ok(t) => t,
                        Err(e) => {
                            log::error!("Could not convert image to texture: {}", ErrorFmt(e));
                            continue;
                        }
                    };
                    let Some(dmabuf) = &buf.client_dmabuf else {
                        self.set_tex(ctx, key, &tex);
                        continue;
                    };
                    let pending = self.create_async_data(key, tex.clone());
                    let mut polls = PlaneVec::new();
                    for plane in &dmabuf.planes {
                        let res = self
                            .client
                            .state
                            .ring
                            .readable_external(&plane.fd, pending.clone());
                        match res {
                            Ok(p) => polls.push(p),
                            Err(e) => {
                                log::error!("Could not create poll: {}", ErrorFmt(e));
                                continue 'outer;
                            }
                        }
                        pending.poll_waits.fetch_add(1);
                        if dmabuf.is_one_file() {
                            break;
                        }
                    }
                    self.pending.set(key, AsyncOp::Polls(polls));
                    continue;
                }
                WlBufferStorage::Shm {
                    mem,
                    stride,
                    dmabuf_buffer_params,
                } => (mem, stride, dmabuf_buffer_params),
            };
            if fast_ram_access
                && let Some(tex) =
                    buf.import_udmabuf_texture(&ctx, mem, stride, dmabuf_buffer_params)
            {
                self.set_tex(ctx, key, &tex);
                continue;
            }
            let tex = ctx.clone().async_shmem_texture(
                buf.format,
                key.size,
                key.size,
                stride,
                &state.cpu_worker,
            );
            let tex = match tex {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Could not create async shm texture: {}", ErrorFmt(e));
                    continue;
                }
            };
            let pending = self.create_async_data(key, tex.clone());
            let damage = Region::new(buf.rect);
            let res = if let Some(hb) = buf.get_gfx_buffer(&ctx, mem, dmabuf_buffer_params) {
                tex.clone()
                    .async_upload_from_buffer(&hb, pending.clone(), damage)
            } else {
                let staging = ctx.create_staging_buffer(tex.staging_size(), STAGING_UPLOAD);
                tex.clone()
                    .async_upload(&staging, pending.clone(), mem.clone(), damage)
            };
            match res {
                Ok(p) => {
                    if let Some(p) = p {
                        self.pending.set(key, AsyncOp::Transfer(p));
                    } else {
                        let tex: Rc<dyn GfxTexture> = tex;
                        self.set_tex(ctx, key, &tex);
                    }
                }
                Err(e) => {
                    log::error!("Could not schedule icon upload: {}", ErrorFmt(e));
                }
            }
        }
    }

    fn create_async_data(
        self: &Rc<Self>,
        key: BufferKey,
        tex: Rc<dyn GfxTexture>,
    ) -> Rc<AsyncData> {
        Rc::new(AsyncData {
            key,
            parent: Rc::downgrade(self),
            tex,
            poll_waits: Default::default(),
        })
    }

    fn set_tex(&self, ctx: &Rc<dyn GfxContext>, key: BufferKey, source_tex: &Rc<dyn GfxTexture>) {
        let buf_to_icon = self.buf_key_to_icon_key.borrow();
        let Some(keys) = buf_to_icon.get(&key) else {
            return;
        };
        let srgb = self.client.state.color_manager.srgb_gamma22();
        let format = source_tex.format();
        let render_pass = GfxRenderPass {
            ops: vec![GfxApiOp::CopyTexture(CopyTexture {
                tex: source_tex.clone(),
                source: SampleRect::identity(),
                target: FramebufferRect {
                    x1: -1.0,
                    x2: 1.0,
                    y1: -1.0,
                    y2: 1.0,
                    output_transform: Default::default(),
                },
                buffer_resv: None,
                acquire_sync: AcquireSync::Unnecessary,
                release_sync: ReleaseSync::None,
                alpha: None,
                opaque: !format.has_alpha,
                render_intent: RenderIntent::Perceptual,
                cd: srgb.clone(),
                alpha_mode: AlphaMode::PremultipliedElectrical,
                grayscale: false,
                client_buf: None,
                lazy: None,
            })],
            clear: format.has_alpha.then_some(Color::TRANSPARENT),
            clear_cd: srgb.linear.clone(),
        };
        for &key in keys {
            let [size] = key.scale.pixel_size([key.size]);
            let res = ctx.clone().create_read_write_img(
                &self.client.state.dma_buf_ids,
                size,
                size,
                format,
            );
            let (fb, tex) = match res {
                Ok(res) => res,
                Err(e) => {
                    log::error!("Could not create read-write image: {}", ErrorFmt(e));
                    continue;
                }
            };
            let res = fb.perform_render_pass(
                AcquireSync::None,
                ReleaseSync::None,
                srgb,
                &render_pass,
                &Region::new(Rect::new_sized_saturating(0, 0, size, size)),
                None,
                &srgb,
            );
            if let Err(e) = res {
                log::error!("Could not render image: {}", ErrorFmt(e));
                continue;
            }
            self.icons.set(key, ToplevelIcon::Tex(tex));
        }
    }

    fn set_color(&self, key: BufferKey, color: Color) {
        let buf_to_icon = self.buf_key_to_icon_key.borrow();
        if let Some(keys) = buf_to_icon.get(&key) {
            for &key in keys {
                self.icons.set(key, ToplevelIcon::Srgb(color));
            }
        }
    }

    fn pending_done(&self, key: BufferKey) {
        self.pending.remove(&key);
        self.pending_changed();
    }

    fn pending_changed(&self) {
        if self.pending.is_not_empty() {
            return;
        }
        for tl in self.toplevels.lock().values() {
            tl.icon_changed();
        }
    }

    pub fn update_user(&self, user: &ToplevelIconUser) {
        user.icons.clear();
        for &(scale, _) in &*self.client.state.scales.lock() {
            let key = IconKey {
                size: user.size.get(),
                scale,
            };
            if let Some(tex) = self.icons.get(&key) {
                user.icons.insert(scale, tex);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    pub fn has_no_pending(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Drop for XdgToplevelIconV1 {
    fn drop(&mut self) {
        self.client
            .state
            .toplevel_icons
            .remove(&self.toplevel_icon_id);
    }
}

impl AsyncShmGfxTextureCallback for AsyncData {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>) {
        if let Err(e) = &res {
            log::error!("Upload failed: {}", ErrorFmt(e));
        }
        let Some(icon) = self.parent.upgrade() else {
            return;
        };
        if res.is_ok()
            && let Some(ctx) = icon.client.state.render_ctx.get()
        {
            icon.set_tex(&ctx, self.key, &self.tex);
        }
        icon.pending_done(self.key);
    }
}

impl PollCallback for AsyncData {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        if let Err(e) = res {
            log::error!("Poll failed: {}", ErrorFmt(e));
            if let Some(icon) = self.parent.upgrade() {
                icon.pending_done(self.key);
            }
            return;
        }
        if self.poll_waits.sub_fetch(1) > 0 {
            return;
        }
        let Some(icon) = self.parent.upgrade() else {
            return;
        };
        if let Some(ctx) = icon.client.state.render_ctx.get() {
            icon.set_tex(&ctx, self.key, &self.tex);
        }
        icon.pending_done(self.key);
    }
}

impl XdgToplevelIconV1RequestHandler for XdgToplevelIconV1 {
    type Error = XdgToplevelIconV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_name(&self, _req: SetName<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        Ok(())
    }

    fn add_buffer(&self, req: AddBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        let buffer = self.client.lookup(req.buffer)?;
        if buffer.rect.width() != buffer.rect.height() {
            return Err(XdgToplevelIconV1Error::NotSquare);
        }
        let key = BufferKey {
            size: buffer.rect.width(),
            scale: req.scale,
        };
        self.buffers.set(key, buffer);
        Ok(())
    }
}

object_base! {
    self = XdgToplevelIconV1;
    version = self.version;
}

impl Object for XdgToplevelIconV1 {
    fn break_loops(self: Rc<Self>) {
        self.toplevels.clear();
        self.pending.clear();
    }
}

dedicated_add_obj!(XdgToplevelIconV1, XdgToplevelIconV1Id, xdg_toplevel_icons);

#[derive(Debug, Error)]
pub enum XdgToplevelIconV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Toplevel icon is immutable")]
    Immutable,
    #[error("Buffer is not a square")]
    NotSquare,
}
efrom!(XdgToplevelIconV1Error, ClientError);
