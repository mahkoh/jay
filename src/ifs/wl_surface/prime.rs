use crate::allocator::AllocatorError;
use crate::allocator::BO_USE_SCANOUT;
use crate::allocator::BufferObject;
use crate::allocator::BufferUsage;
use crate::backend::DrmDeviceId;
use crate::copy_device::CopyDevice;
use crate::copy_device::CopyDeviceDstObject;
use crate::copy_device::CopyDeviceError;
use crate::copy_device::CopyDeviceSrcObject;
use crate::env::JAY_NO_CLIENT_PRIME;
use crate::gfx_api::BufferResv;
use crate::gfx_api::BufferResvUser;
use crate::gfx_api::FdSync;
use crate::gfx_api::GfxContext;
use crate::gfx_api::GfxError;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::LazyTexture;
use crate::gfx_api::TextureUse;
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_buffer::WlBufferDmabufStorage;
use crate::ifs::wl_buffer::WlBufferStorage;
use crate::ifs::wl_surface::WlSurface;
use crate::io_uring::PendingPoll;
use crate::io_uring::PollCallback;
use crate::rect::DynamicDamageQueue;
use crate::rect::DynamicDamageQueueElement;
use crate::rect::Rect;
use crate::rect::Region;
use crate::state::PrimeModifiers;
use crate::state::State;
use crate::udmabuf::UdmabufError;
use crate::udmabuf::UdmabufHolder;
use crate::utils::cell_ext::CellExt;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::numcell::NumCell;
use crate::utils::obj_and_id::ObjWithId;
use crate::utils::oserror::OsError;
use crate::utils::rc_eq::rc_opt_eq;
use crate::utils::smallmap::SmallMap;
use crate::utils::syncqueue::SyncQueue;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmabufCopy;
use arrayvec::ArrayVec;
use linearize::StaticMap;
use smallvec::SmallVec;
use std::cell::Cell;
use std::error::Error;
use std::ffi::c_short;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::rc::Rc;
use std::slice;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrimeError {
    #[error("Udmabuf is not available")]
    UdmabufNotAvailable,
    #[error("Could not create a udmabuf")]
    CreateUdmabuf(#[source] UdmabufError),
    #[error("Could not import udmabuf into client device")]
    ImportUdmabufDst(#[source] CopyDeviceError),
    #[error("Could not import udmabuf into render device")]
    ImportUdmabufSrc(#[source] CopyDeviceError),
    #[error("Could not create udmabuf texture")]
    CreateUdmabufTexture(#[source] GfxError),
    #[error("Could not create BO texture")]
    CreateBoTexture(#[source] GfxError),
    #[error("Could not create BO object")]
    CreateBoDst(#[source] CopyDeviceError),
    #[error("Could not allocate BO")]
    AllocatePrimeBo(#[source] AllocatorError),
    #[error("Could not create source object")]
    CreateSrcObject(#[source] CopyDeviceError),
}

pub struct SurfacePrimeState {
    pub buffer: CloneCell<Option<Rc<PrimeSurfaceBuffer>>>,
    inner: Rc<StateInner>,
    damage: Rc<DynamicDamageQueue>,
    udmabuf: CloneCell<Option<Rc<DmaBuf>>>,
    udmabuf_src_object: CloneCell<Option<Rc<CopyDeviceSrcObject>>>,
    udmabuf_dst_objects: CopyHashMap<DrmDeviceId, Rc<CopyDeviceDstObject>>,
}

struct StateInner {
    version: NumCell<u64>,
    storage: SyncQueue<Rc<PrimeStorage>>,
    state: Rc<State>,
    usage: StaticMap<TextureUse, Cell<u64>>,
}

pub struct PrimeSurfaceBuffer {
    inner: Rc<StateInner>,
    storage: Rc<PrimeStorage>,
    tex: Rc<dyn GfxTexture>,
    lazy_copies: Cell<Option<(ArrayVec<DmabufCopy, 2>, Region)>>,
    pending_lazy_copies: Cell<Option<PendingPoll>>,
}

struct PrimeStorage {
    version: u64,

    damage: DynamicDamageQueueElement,
    tex: CloneCell<Option<Rc<dyn GfxTexture>>>,

    udmabuf: CloneCell<Option<Rc<DmaBuf>>>,
    udmabuf_objects: CopyHashMap<DrmDeviceId, Rc<CopyDeviceDstObject>>,

    bo: CloneCell<Option<Rc<dyn BufferObject>>>,
    bo_object: CloneCell<Option<Rc<CopyDeviceDstObject>>>,
    bo_flags: BufferUsage,
    bo_modifiers: Option<PrimeModifiers>,

    syncs: SmallMap<BufferResvUser, FdSync, 2>,
    lazy_sync: CloneCell<Option<FdSync>>,
}

pub struct PrimeValidity {
    version: u64,
    inner: Rc<StateInner>,
}

pub fn no_client_prime(udmabuf: &UdmabufHolder) -> bool {
    if udmabuf.get().is_none() {
        log::warn!("Disabling client prime copies because udmabuf device is unavailable");
        true
    } else if *JAY_NO_CLIENT_PRIME {
        log::warn!(
            "Disabling client prime copies because {}",
            JAY_NO_CLIENT_PRIME.as_env(),
        );
        true
    } else {
        false
    }
}

impl PrimeValidity {
    pub fn valid(&self) -> bool {
        self.version == self.inner.version.get()
    }
}

impl SurfacePrimeState {
    pub fn new(state: &Rc<State>) -> Self {
        Self {
            buffer: Default::default(),
            inner: Rc::new(StateInner {
                version: Default::default(),
                storage: Default::default(),
                state: state.clone(),
                usage: Default::default(),
            }),
            damage: Default::default(),
            udmabuf: Default::default(),
            udmabuf_src_object: Default::default(),
            udmabuf_dst_objects: Default::default(),
        }
    }

    pub fn validity(&self) -> PrimeValidity {
        PrimeValidity {
            version: self.inner.version.get(),
            inner: self.inner.clone(),
        }
    }

    pub fn buffer(&self) -> Option<Rc<PrimeSurfaceBuffer>> {
        self.buffer.get()
    }

    pub fn reset(&self) -> bool {
        self.inner.version.fetch_add(1);
        self.inner.storage.clear();
        self.damage.clear_all();
        self.udmabuf.take();
        self.udmabuf_src_object.take();
        self.udmabuf_dst_objects.clear();
        self.buffer.take().is_some()
    }

    fn use_lazy_copies(&self) -> bool {
        const TWO_SECONDS: u64 = 2_000_000_000;
        let inner = &self.inner;
        let usage = &inner.usage;
        let now = inner.state.now_nsec();
        let render_age = now - usage[TextureUse::Render].get();
        let scanout_age = now - usage[TextureUse::Scanout].get();
        render_age > TWO_SECONDS && scanout_age <= TWO_SECONDS
    }
}

impl Debug for PrimeSurfaceBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimeSurfaceBuffer").finish_non_exhaustive()
    }
}

impl BufferResv for PrimeSurfaceBuffer {
    fn set_sync(&self, user: BufferResvUser, sync: &FdSync) {
        self.storage.syncs.insert(user, sync.clone());
    }
}

impl Drop for PrimeSurfaceBuffer {
    fn drop(&mut self) {
        let i = &self.inner;
        let s = &self.storage;
        if s.version != i.version.get() {
            return;
        }
        i.storage.push(s.clone());
        if let Some((_, damage)) = self.lazy_copies.take() {
            let damage = damage.extents();
            if damage.is_not_empty() {
                s.damage.damage_self(&[damage]);
            }
        }
        if let Some(sync) = s.lazy_sync.take() {
            let user = self.inner.state.lazy_prime_buffer_resv_user;
            s.syncs.insert(user, sync);
        }
    }
}

impl LazyTexture for PrimeSurfaceBuffer {
    fn record_use(&self, ty: TextureUse) {
        self.inner.usage[ty].set(self.inner.state.now_nsec());
    }

    fn perform_lazy_work(&self, sync: &mut Vec<FdSync>) -> Result<(), Box<dyn Error + Send>> {
        if let Some((copies, damage)) = self.lazy_copies.take() {
            let mut fd = None;
            for copy in copies {
                fd = copy
                    .execute(fd.as_ref(), Some(&damage))
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            }
            if let Some(fd) = &fd {
                let cb = fd
                    .signaled_external(&self.inner.state.ring, self.storage.clone())
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                self.pending_lazy_copies.set(cb);
            }
            self.storage.lazy_sync.set(fd);
        }
        if let Some(fd) = self.storage.lazy_sync.get() {
            sync.push(fd);
        }
        Ok(())
    }

    fn has_lazy_work(&self) -> bool {
        self.lazy_copies.is_some() || self.storage.lazy_sync.is_some()
    }
}

impl PollCallback for PrimeStorage {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        match res {
            Ok(_) => {
                self.lazy_sync.take();
            }
            Err(e) => {
                log::error!(
                    "Could not wait for lazy prime copy to complete: {}",
                    ErrorFmt(e),
                );
            }
        }
    }
}

impl PrimeSurfaceBuffer {
    pub fn clear_damage(&self) {
        self.storage.damage.clear();
    }

    pub fn tex(&self) -> &Rc<dyn GfxTexture> {
        &self.tex
    }

    pub fn take_sync(&self) -> SmallVec<[FdSync; 1]> {
        self.storage.syncs.take().into_iter().map(|v| v.1).collect()
    }

    pub fn damage(&self) -> Region {
        self.storage.damage.get()
    }
}

impl WlBuffer {
    pub fn needs_prime_copy(
        &self,
        ctx: &Rc<dyn GfxContext>,
        render_dev: Option<&Rc<CopyDevice>>,
    ) -> bool {
        let Some(WlBufferStorage::Dmabuf(storage)) = &mut *self.storage.borrow_mut() else {
            return false;
        };
        self.classify_prime_(ctx, render_dev, storage)
            .map(|s| s.is_some())
            .unwrap_or(true)
    }

    fn classify_prime_(
        &self,
        ctx: &Rc<dyn GfxContext>,
        render_dev: Option<&Rc<CopyDevice>>,
        storage: &mut WlBufferDmabufStorage,
    ) -> Result<Option<Rc<CopyDeviceSrcObject>>, PrimeError> {
        if self.client.state.no_client_prime {
            return Ok(None);
        }
        if storage.tex.is_some() {
            return Ok(None);
        }
        if self.client_dmabuf_device.is_none()
            && ctx.fast_ram_access()
            && storage.ensure_tex(self, ctx).is_ok()
        {
            return Ok(None);
        }
        storage
            .ensure_copy_object(self, render_dev)
            .map_err(PrimeError::CreateSrcObject)
    }
}

impl WlSurface {
    pub fn prepare_prime_copies(
        &self,
        ctx: &Rc<dyn GfxContext>,
        render_dev: Option<&Rc<CopyDevice>>,
        buf: &WlBuffer,
        storage: &mut WlBufferDmabufStorage,
        damage: &[Rect],
        allow_lazy: bool,
    ) -> Result<Option<(ArrayVec<DmabufCopy, 2>, Rc<PrimeSurfaceBuffer>)>, PrimeError> {
        let Some(src) = buf.classify_prime_(ctx, render_dev, storage)? else {
            return Ok(None);
        };
        let state = &self.client.state;
        let prime = &self.prime;
        let udmabuf_dev = state.udmabuf.get();
        let create_udmabuf = || {
            let Some(dev) = &udmabuf_dev else {
                return Err(PrimeError::UdmabufNotAvailable);
            };
            dev.create_dmabuf(&state.dma_buf_ids, buf.width, buf.height, buf.format)
                .map_err(PrimeError::CreateUdmabuf)
        };
        let client_copy_device = buf.client_copy_device();
        let mut direct_scanout_connector = self.fullscreen.get();
        if let Some(con) = &direct_scanout_connector
            && render_dev.map(|d| d.drm_device_id()) != con.drm_dev.id()
        {
            direct_scanout_connector = None;
        }
        let use_bo = render_dev.is_some()
            && (!ctx.fast_ram_access()
                || client_copy_device.is_none()
                || udmabuf_dev.is_none()
                || direct_scanout_connector.is_some());
        let mut bo_flags = BufferUsage::none();
        let mut bo_modifiers = None;
        if use_bo {
            if direct_scanout_connector.is_some() {
                bo_flags = BO_USE_SCANOUT;
            }
            bo_modifiers = state
                .render_ctx_prime_modifiers
                .get(&direct_scanout_connector.id());
        }
        let spt = loop {
            let Some(spt) = prime.inner.storage.pop() else {
                break Rc::new(PrimeStorage {
                    version: prime.inner.version.get(),
                    damage: prime.damage.add_element(),
                    tex: Default::default(),
                    udmabuf: Default::default(),
                    udmabuf_objects: Default::default(),
                    bo: Default::default(),
                    bo_object: Default::default(),
                    bo_flags,
                    bo_modifiers,
                    syncs: Default::default(),
                    lazy_sync: Default::default(),
                });
            };
            if use_bo {
                if spt.bo_flags != bo_flags {
                    continue;
                }
                if !rc_opt_eq(&bo_modifiers, &spt.bo_modifiers) {
                    continue;
                }
                let Some(bo) = spt.bo.get() else {
                    continue;
                };
                if incompatible(buf, bo.dmabuf()) {
                    continue;
                }
            } else {
                let Some(dmabuf) = spt.udmabuf.get() else {
                    continue;
                };
                if incompatible(buf, &dmabuf) {
                    continue;
                }
            }
            break spt;
        };
        let damage_full = || {
            spt.damage.clear_all();
            spt.damage.damage(slice::from_ref(&buf.rect));
        };
        let tex;
        if let Some(render_dev) = render_dev
            && use_bo
        {
            let bo = match spt.bo.get() {
                Some(bo) => {
                    spt.damage.damage(damage);
                    bo
                }
                _ => {
                    damage_full();
                    let modifiers = &mut *state.render_ctx_prime_modifiers_stash.borrow_mut();
                    modifiers.clear();
                    spt.bo_modifiers
                        .as_ref()
                        .and_then(|m| m.get(&buf.format.drm))
                        .map(|m| &**m)
                        .unwrap_or_default()
                        .iter()
                        .filter(|s| {
                            s.max_width >= buf.width as u32 && s.max_height >= buf.height as u32
                        })
                        .for_each(|s| modifiers.push(s.modifier));
                    let bo = ctx
                        .allocator()
                        .create_bo(
                            &state.dma_buf_ids,
                            buf.width,
                            buf.height,
                            buf.format,
                            modifiers,
                            spt.bo_flags,
                        )
                        .map_err(PrimeError::AllocatePrimeBo)?;
                    spt.bo.set(Some(bo.clone()));
                    bo
                }
            };
            tex = match spt.tex.get() {
                Some(t) => t,
                _ => {
                    let tex = ctx
                        .clone()
                        .dmabuf_tex(bo.dmabuf())
                        .map_err(PrimeError::CreateBoTexture)?;
                    spt.tex.set(Some(tex.clone()));
                    tex
                }
            };
            if spt.bo_object.is_none() {
                let obj = render_dev
                    .create_dst_object(bo.dmabuf())
                    .map_err(PrimeError::CreateBoDst)?;
                spt.bo_object.set(Some(Rc::new(obj)));
            }
        } else {
            prime.udmabuf.take();
            prime.udmabuf_src_object.take();
            prime.udmabuf_dst_objects.clear();
            let udmabuf = match spt.udmabuf.get() {
                Some(b) => {
                    spt.damage.damage(damage);
                    b
                }
                None => {
                    damage_full();
                    let b = create_udmabuf()?;
                    spt.udmabuf.set(Some(b.clone()));
                    b
                }
            };
            tex = match spt.tex.get() {
                Some(t) => t,
                _ => {
                    let tex = ctx
                        .clone()
                        .dmabuf_tex(&udmabuf)
                        .map_err(PrimeError::CreateUdmabufTexture)?;
                    spt.tex.set(Some(tex.clone()));
                    tex
                }
            };
        }
        let mut copies = ArrayVec::new();
        if let Some(client_dev) = client_copy_device {
            let client_id = client_dev.drm_device_id();
            if let Some(udmabuf) = spt.udmabuf.get() {
                let dst = match spt.udmabuf_objects.get(&client_id) {
                    Some(obj) => obj,
                    None => {
                        let obj = client_dev
                            .create_dst_object(&udmabuf)
                            .map(Rc::new)
                            .map_err(PrimeError::ImportUdmabufDst)?;
                        spt.udmabuf_objects.set(client_id, obj.clone());
                        obj
                    }
                };
                copies.push(DmabufCopy::AdHoc(src, dst));
            } else if let Some(render_dev) = render_dev
                && let Some(bo_obj) = spt.bo_object.get()
            {
                let mut udmabuf_opt = prime.udmabuf.get();
                if let Some(udmabuf) = &udmabuf_opt
                    && incompatible(buf, udmabuf)
                {
                    udmabuf_opt = None;
                    prime.udmabuf.take();
                    prime.udmabuf_src_object.take();
                    prime.udmabuf_dst_objects.clear();
                }
                let udmabuf = match udmabuf_opt {
                    Some(b) => b,
                    None => {
                        let udmabuf = create_udmabuf()?;
                        prime.udmabuf.set(Some(udmabuf.clone()));
                        udmabuf
                    }
                };
                let udmabuf_dst_obj = match prime.udmabuf_dst_objects.get(&client_id) {
                    Some(obj) => obj,
                    _ => {
                        let obj = client_dev
                            .create_dst_object(&udmabuf)
                            .map_err(PrimeError::ImportUdmabufDst)
                            .map(Rc::new)?;
                        prime.udmabuf_dst_objects.set(client_id, obj.clone());
                        obj
                    }
                };
                let udmabuf_src_obj = match prime.udmabuf_src_object.get() {
                    Some(obj) => obj,
                    None => {
                        let obj = render_dev
                            .create_src_object(&udmabuf)
                            .map_err(PrimeError::ImportUdmabufSrc)
                            .map(Rc::new)?;
                        prime.udmabuf_src_object.set(Some(obj.clone()));
                        obj
                    }
                };
                copies.push(DmabufCopy::AdHoc(src.clone(), udmabuf_dst_obj));
                copies.push(DmabufCopy::AdHoc(udmabuf_src_obj, bo_obj));
            }
        } else if let Some(bo_obj) = spt.bo_object.get() {
            copies.push(DmabufCopy::AdHoc(src, bo_obj));
        } else {
            unreachable!();
        }
        let mut lazy_copies = None;
        if allow_lazy && prime.use_lazy_copies() {
            lazy_copies = Some((copies, spt.damage.get()));
            copies = ArrayVec::new();
            spt.damage.clear();
        }
        let buffer = Rc::new(PrimeSurfaceBuffer {
            inner: prime.inner.clone(),
            storage: spt,
            tex,
            lazy_copies: Cell::new(lazy_copies),
            pending_lazy_copies: Default::default(),
        });
        Ok(Some((copies, buffer)))
    }
}

fn incompatible(buf: &WlBuffer, dmabuf: &DmaBuf) -> bool {
    (dmabuf.format, dmabuf.width, dmabuf.height) != (buf.format, buf.width, buf.height)
}
