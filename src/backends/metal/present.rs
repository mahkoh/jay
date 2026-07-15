use crate::backend::BackendDrmDevice;
use crate::backend::BackendGammaLut;
use crate::backend::BackendGammaLutId;
use crate::backend::Connector;
use crate::backends::metal::MetalError;
use crate::backends::metal::allocator::RenderBuffer;
use crate::backends::metal::allocator::RenderBufferCopy;
use crate::backends::metal::transaction::DrmConnectorState;
use crate::backends::metal::transaction::DrmPlaneState;
use crate::backends::metal::video::MetalConnector;
use crate::backends::metal::video::MetalCrtc;
use crate::backends::metal::video::MetalHardwareCursorChange;
use crate::backends::metal::video::MetalPlane;
use crate::backends::metal::video::metal_cm::MetalCmProgramming;
use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::ColorDescriptionId;
use crate::cmm::cmm_render_intent::RenderIntent;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::BufferResv;
use crate::gfx_api::CopyTexture;
use crate::gfx_api::DirectScanoutPosition;
use crate::gfx_api::GfxRenderPass;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::LazyTexture;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::SyncFile;
use crate::gfx_api::TextureUse;
use crate::gfx_api::create_render_pass;
use crate::ifs::wl_output::BlendSpace;
use crate::rect::Region;
use crate::time::Time;
use crate::tracy::FrameName;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::obj_and_id::ObjWithId;
use crate::utils::oserror::OsError;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufId;
use crate::video::drm::DRM_MODE_ATOMIC_NONBLOCK;
use crate::video::drm::DRM_MODE_PAGE_FLIP_ASYNC;
use crate::video::drm::DRM_MODE_PAGE_FLIP_EVENT;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmError;
use crate::video::drm::DrmFb;
use crate::video::drm::DrmFramebuffer;
use crate::video::drm::DrmObject;
use crate::video::drm::DrmPlane;
use arrayvec::ArrayVec;
use jay_proc::jay_hash;
use std::rc::Rc;
use std::rc::Weak;
use uapi::OwnedFd;
use uapi::c;

struct Latched {
    pass: GfxRenderPass,
    damage_count: u64,
    damage: Region,
    locked: bool,
}

#[derive(Debug)]
pub struct DirectScanoutCache {
    dmabuf: Weak<DmaBuf>,
    fb: Option<Rc<DrmFramebuffer>>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub struct DirectScanoutKey {
    dma_buf_id: DmaBufId,
    plane: DrmPlane,
    crtc: DrmCrtc,
    src: ColorDescriptionId,
    dst: ColorDescriptionId,
    intent: RenderIntent,
    gamma_lut: Option<BackendGammaLutId>,
    has_cursor_plane: bool,
    use_plane_color_pipelines: bool,
}

#[derive(Debug)]
struct DirectScanoutData {
    fb_cd: Rc<ColorDescription>,
    fb_intent: RenderIntent,
    key: DirectScanoutKey,
    cm_programming: MetalCmProgramming,
}

#[derive(Debug)]
struct DirectScanoutDataCore {
    tex: Rc<dyn GfxTexture>,
    tex_resv: Option<Rc<dyn BufferResv>>,
    acquire_sync: AcquireSync,
    release_sync: ReleaseSync,
    _fb_resv: Option<Rc<dyn BufferResv>>,
    lazy: Option<Rc<dyn LazyTexture>>,
    fb: Rc<DrmFramebuffer>,
    position: DirectScanoutPosition,
}

struct PresentFb {
    fb_intent: RenderIntent,
    copy: RenderBufferCopy,
    cm_programming: MetalCmProgramming,
    direct_scanout_key: Option<DirectScanoutKey>,
    core: PresentFbCore,
}

pub struct PresentFbCore {
    fb: Rc<DrmFramebuffer>,
    fb_cd: Rc<ColorDescription>,
    tex: Rc<dyn GfxTexture>,
    direct_scanout_data: Option<DirectScanoutDataCore>,
    pub locked: bool,
}

#[derive(Debug)]
struct CursorProgramming {
    plane: Rc<MetalPlane>,
    ty: CursorProgrammingType,
}

#[derive(Debug)]
enum CursorProgrammingType {
    Enable {
        fb: Rc<DrmFramebuffer>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        swap: bool,
    },
    Disable,
}

struct ChangedPlane {
    plane: Rc<MetalPlane>,
    state: DrmPlaneState,
}

pub const DEFAULT_PRE_COMMIT_MARGIN: u64 = 16_000_000; // 16ms
pub const DEFAULT_POST_COMMIT_MARGIN: u64 = 1_500_000; // 1.5ms;
pub const POST_COMMIT_MARGIN_DELTA: u64 = 500_000; // 500us

#[derive(Copy, Clone)]
enum PresentFbWait {
    Render,
    Scanout,
}

impl MetalConnector {
    pub fn schedule_present(&self) {
        self.present_trigger.trigger();
    }

    pub async fn present_loop(self: Rc<Self>) {
        #[cfg_attr(not(feature = "tracy"), expect(unused_variables))]
        let frame_name = FrameName::get(&self.kernel_id().to_string());
        let mut cur_sec = 0;
        let mut max = 0;
        loop {
            self.present_trigger.triggered().await;
            if !self.buffers_idle.get() || !self.crtc_idle.get() {
                continue;
            }
            let Some(crtc) = self.crtc.get() else {
                continue;
            };
            let Some(node) = self.state.root.outputs.get(&self.connector_id) else {
                continue;
            };
            let version = self.version.get();
            let mut expected_sequence = crtc.sequence.get() + 1;
            let mut start = Time::now_unchecked();
            let use_frame_scheduling = !self.try_async_flip();
            if use_frame_scheduling {
                let next_present = self
                    .next_vblank_nsec
                    .get()
                    .saturating_sub(self.pre_commit_margin.get())
                    .saturating_sub(self.post_commit_margin.get());
                if start.nsec() < next_present {
                    self.state.ring.timeout(next_present).await.unwrap();
                    start = Time::now_unchecked();
                } else {
                    expected_sequence += 1;
                }
            }
            frame!(frame_name);
            {
                let now = start.nsec();
                let flip = match self.try_async_flip() {
                    true => now,
                    false => self.next_vblank_nsec.get(),
                };
                node.before_latch(flip).await;
            }
            if version != self.version.get() {
                self.present_trigger.trigger();
                continue;
            }
            if let Err(e) = self.present_once(&node, &crtc).await {
                log::error!("Could not present: {}", ErrorFmt(e));
                continue;
            }
            if use_frame_scheduling {
                self.expected_sequence.set(Some(expected_sequence));
            }
            self.state.set_backend_idle(false);
            let duration = start.elapsed();
            max = max.max(duration.as_nanos() as _);
            if start.0.tv_sec != cur_sec {
                cur_sec = start.0.tv_sec;
                self.pre_commit_margin_decay.add(max);
                self.pre_commit_margin
                    .set(self.pre_commit_margin_decay.get());
                max = 0;
            }
        }
    }

    async fn present_once(
        self: &Rc<Self>,
        node: &Rc<OutputNode>,
        crtc: &Rc<MetalCrtc>,
    ) -> Result<(), MetalError> {
        let version = self.version.get();
        if !self.buffers_idle.get() || !self.crtc_idle.get() {
            return Ok(());
        }
        if !self.backend.check_render_context(&self.dev) {
            return Ok(());
        }
        if !crtc.drm_state.borrow().active.value {
            return Ok(());
        }
        let plane = match self.primary_plane.get() {
            Some(p) => p,
            _ => return Ok(()),
        };
        let buffers = match self.buffers.get() {
            Some(b) => b,
            _ => return Ok(()),
        };
        let mut connector_drm_state = self.display.borrow().drm_state.clone();
        let next_buffer_idx = ((connector_drm_state.fb_idx + 1) % buffers.len() as u64) as usize;
        let buffer = &buffers[next_buffer_idx];

        let ons = &node.node_state[RenderTL];
        let gamma_lut = self.gamma_lut.get();
        let cd = ons.color_description.get();
        let linear_cd = ons.linear_color_description.get();
        let blend_cd = match node.global.persistent.blend_space.get() {
            BlendSpace::Linear => &linear_cd,
            BlendSpace::Srgb => self.state.color_manager.srgb_gamma22(),
        };

        if self.has_damage.get() > 0 || self.cursor_damage.get() {
            node.schedule.commit_cursor();
        }
        self.latch_cursor(&node, &connector_drm_state, &cd)?;
        let cursor_programming = self.compute_cursor_programming(&connector_drm_state);
        let latched = self.latch(&node, buffer);
        node.latched(self.try_async_flip());

        if cursor_programming.is_none() && latched.is_none() {
            return Ok(());
        }

        let mut present_fb = None;
        let mut direct_scanout_key = None;
        if let Some(latched) = &latched {
            let fb = self.prepare_present_fb(
                gamma_lut.as_ref(),
                &cd,
                blend_cd,
                buffer,
                &plane,
                &crtc,
                latched,
                true,
            )?;
            direct_scanout_key = fb.direct_scanout_key;
            present_fb = Some(fb);
        }
        self.await_present_fb(present_fb.as_mut(), PresentFbWait::Render)
            .await;
        // perform_screencopies should return a sync file that we wait on before
        // presentation since during screencopy the buffer layout might be mutated which
        // could interfere with scanout. However, perform_screencopies just uses the
        // current PresentFb if present_fb is None, potentially mutating the fb that is
        // currently being scanned out, which would render such a wait absurd.
        self.perform_screencopies(&present_fb, &node);
        if let Some(sync) = self.cursor_sync.take() {
            sync.signaled(&self.state.ring, "cursor").await;
        }
        self.await_present_fb(present_fb.as_mut(), PresentFbWait::Scanout)
            .await;
        let mut changed_planes = ArrayVec::new();
        let mut res = self.program_connector(
            version,
            &crtc,
            &plane,
            cursor_programming.as_ref(),
            present_fb.as_ref(),
            &mut changed_planes,
            &mut connector_drm_state,
        );
        if res.is_err()
            && let Some(dsk) = direct_scanout_key
        {
            let fb = self.prepare_present_fb(
                gamma_lut.as_ref(),
                &cd,
                blend_cd,
                buffer,
                &plane,
                &crtc,
                latched.as_ref().unwrap(),
                false,
            )?;
            present_fb = Some(fb);
            self.await_present_fb(present_fb.as_mut(), PresentFbWait::Scanout)
                .await;
            res = self.program_connector(
                version,
                &crtc,
                &plane,
                cursor_programming.as_ref(),
                present_fb.as_ref(),
                &mut changed_planes,
                &mut connector_drm_state,
            );
            if res.is_ok() {
                self.scanout_impossible_cache.insert(dsk, ());
            }
        }
        let reset_damage = || {
            for buffer in &*buffers {
                buffer.damage_queue.clear();
            }
            buffers[0].damage_full();
        };
        if let Err(e) = res {
            reset_damage();
            if let MetalError::Commit(DrmError::Atomic(OsError(c::EACCES))) = e {
                log::debug!(
                    "Could not perform atomic commit, likely because we're no longer the DRM master"
                );
                return Ok(());
            }
            Err(e)
        } else {
            crtc.pending_flip.set(Some(self.clone()));
            self.crtc_idle.set(false);
            self.color_description.set(cd);
            self.display.borrow_mut().drm_state = connector_drm_state;
            for plane in changed_planes {
                *plane.plane.drm_state.borrow_mut() = plane.state;
            }
            if let Some(fb) = present_fb {
                self.fb_color_description.set(fb.core.fb_cd.clone());
                self.fb_render_intent.set(fb.fb_intent);
                self.presentation_is_zero_copy
                    .set(fb.core.direct_scanout_data.is_some());
                if fb.core.direct_scanout_data.is_none() {
                    buffer.damage_queue.clear();
                } else {
                    reset_damage();
                }
                buffer.locked.set(fb.core.locked);
                self.cm.apply(&fb.cm_programming);
                self.next_framebuffer.set(Some(fb.core));
            }
            if let Some(programming) = cursor_programming
                && let CursorProgrammingType::Enable { swap: true, .. } = &programming.ty
            {
                self.cursor_swap_buffer.set(false);
            }
            self.buffers_idle.set(false);
            if let Some(latched) = latched {
                self.has_damage.fetch_sub(latched.damage_count);
            }
            self.cursor_changed.set(false);
            Ok(())
        }
    }

    async fn await_present_fb(&self, new_fb: Option<&mut PresentFb>, wait: PresentFbWait) {
        use PresentFbWait as W;
        let Some(fb) = new_fb else {
            return;
        };
        let field = match wait {
            W::Render => &mut fb.copy.render_block,
            W::Scanout => &mut fb.copy.present_block,
        };
        let Some(sync) = field.take() else {
            return;
        };
        let name = match wait {
            W::Render => "render",
            W::Scanout => "scanout",
        };
        sync.signaled(&self.state.ring, name).await;
    }

    fn try_async_flip(&self) -> bool {
        self.display.borrow().persistent.state.borrow().tearing && self.dev.supports_async_commit
    }

    fn program_connector(
        &self,
        version: u64,
        crtc: &Rc<MetalCrtc>,
        plane: &Rc<MetalPlane>,
        cursor: Option<&CursorProgramming>,
        new_fb: Option<&PresentFb>,
        changed_planes: &mut ArrayVec<ChangedPlane, 2>,
        connector_drm_state: &mut DrmConnectorState,
    ) -> Result<(), MetalError> {
        zone!("program_connector");
        let mut changes = self.master.change();
        let mut try_async_flip = self.try_async_flip();
        let mut drm_state = plane.drm_state.borrow().clone();
        changed_planes.clear();
        let mut connector_state = connector_drm_state.clone();
        if let Some(fb) = new_fb {
            let (crtc_x, crtc_y, crtc_w, crtc_h, src_width, src_height) =
                match &fb.core.direct_scanout_data {
                    None => {
                        let plane_w = plane.mode_w.get();
                        let plane_h = plane.mode_h.get();
                        (0, 0, plane_w, plane_h, plane_w, plane_h)
                    }
                    Some(dsd) => {
                        let p = &dsd.position;
                        (
                            p.crtc_x,
                            p.crtc_y,
                            p.crtc_width,
                            p.crtc_height,
                            p.src_width,
                            p.src_height,
                        )
                    }
                };
            self.cm.prepare(&fb.cm_programming, &mut changes, None);
            changes.change_object(plane.id, |c| {
                c.change(drm_state.fb_id.id, fb.core.fb.id());
                drm_state.fb_id.value = fb.core.fb.id();
                connector_state.fb = fb.core.fb.id();
                connector_state.locked = fb.core.locked;
                if fb.core.direct_scanout_data.is_none() {
                    connector_state.fb_idx += 1;
                }
                macro_rules! change {
                    ($prop:ident, $new:expr) => {{
                        if drm_state.$prop.value != $new {
                            c.change(drm_state.$prop.id, $new as u64);
                            try_async_flip = false;
                            drm_state.$prop.value = $new;
                        }
                        connector_state.$prop = drm_state.$prop.value;
                    }};
                }
                change!(src_w, (src_width as u32) << 16);
                change!(src_h, (src_height as u32) << 16);
                change!(crtc_x, crtc_x);
                change!(crtc_y, crtc_y);
                change!(crtc_w, crtc_w);
                change!(crtc_h, crtc_h);
            });
            changed_planes.push(ChangedPlane {
                plane: plane.clone(),
                state: drm_state,
            });
        }
        if let Some(cursor) = cursor {
            let plane = &cursor.plane;
            let mut drm_state = plane.drm_state.borrow().clone();
            try_async_flip = false;
            changes.change_object(plane.id, |c| {
                macro_rules! change {
                    ($prop:ident, $new:expr) => {{
                        c.change(drm_state.$prop.id, $new);
                        drm_state.$prop.value = $new;
                    }};
                }
                match &cursor.ty {
                    CursorProgrammingType::Enable {
                        fb,
                        x,
                        y,
                        width,
                        height,
                        swap,
                    } => {
                        connector_state.cursor_fb = fb.id();
                        if *swap {
                            connector_state.cursor_fb_idx += 1;
                        }
                        connector_state.cursor_x = *x;
                        connector_state.cursor_y = *y;
                        change!(fb_id, fb.id());
                        change!(crtc_id, crtc.id);
                        change!(crtc_x, *x);
                        change!(crtc_y, *y);
                        change!(crtc_w, *width);
                        change!(crtc_h, *height);
                        change!(src_x, 0);
                        change!(src_y, 0);
                        change!(src_w, (*width as u32) << 16);
                        change!(src_h, (*height as u32) << 16);
                        if !self.dev.vendor.is_nvidia
                            && let Some(sf) = self.backend.signaled_sync_file.get()
                        {
                            c.change(plane.in_fence_fd, sf.0.raw() as u64);
                        }
                    }
                    CursorProgrammingType::Disable => {
                        connector_state.cursor_fb = DrmFb::NONE;
                        change!(fb_id, DrmFb::NONE);
                        change!(crtc_id, DrmCrtc::NONE);
                    }
                }
            });
            changed_planes.push(ChangedPlane {
                plane: plane.clone(),
                state: drm_state,
            });
        }
        let mut out_fd: c::c_int = -1;
        if version != self.version.get() {
            return Err(MetalError::OutOfDate);
        }
        let mut res;
        'commit: {
            const FLAGS: u32 = DRM_MODE_ATOMIC_NONBLOCK | DRM_MODE_PAGE_FLIP_EVENT;
            if try_async_flip {
                res = changes.commit(FLAGS | DRM_MODE_PAGE_FLIP_ASYNC, 0);
                if res.is_ok() {
                    self.presentation_is_sync.set(false);
                    break 'commit;
                }
            }
            self.presentation_is_sync.set(true);
            if !self.dev.vendor.is_nvidia {
                if new_fb.is_some()
                    && let Some(sf) = self.backend.signaled_sync_file.get()
                {
                    changes.change_object(plane.id, |c| {
                        c.change(plane.in_fence_fd, sf.0.raw() as u64);
                    });
                }
                changes.change_object(crtc.id, |c| {
                    c.change(crtc.out_fence_ptr, &raw mut out_fd as u64);
                });
            }
            res = changes.commit(FLAGS, 0);
        }
        if res.is_ok() {
            connector_state.out_fd =
                (out_fd != -1).then(|| SyncFile(Rc::new(OwnedFd::new(out_fd))));
            *connector_drm_state = connector_state;
        }
        res.map_err(MetalError::Commit)
    }

    fn latch_cursor(
        &self,
        node: &Rc<OutputNode>,
        connector_drm_state: &DrmConnectorState,
        cd: &Rc<ColorDescription>,
    ) -> Result<(), MetalError> {
        if !self.cursor_damage.take() {
            return Ok(());
        }
        if self.cursor_plane.is_none() {
            return Ok(());
        }
        let buffers = self.cursor_buffers.get().unwrap();
        let buffer_idx = ((connector_drm_state.cursor_fb_idx + 1) % buffers.len() as u64) as usize;
        let mut c = MetalHardwareCursorChange {
            cursor_enabled: self.cursor_enabled.get(),
            cursor_swap_buffer: None,
            cursor_x: self.cursor_x.get(),
            cursor_y: self.cursor_y.get(),
            cursor_buffer: &buffers[buffer_idx],
            cursor_size: (self.dev.cursor_width as _, self.dev.cursor_height as _),
        };
        self.state.present_hardware_cursor(node, &mut c);
        let swap_buffers = c.cursor_swap_buffer.is_some();
        self.cursor_swap_buffer.set(swap_buffers);
        if let Some(sync) = c.cursor_swap_buffer.take() {
            let sync = c
                .cursor_buffer
                .copy_to_dev(cd, None, sync)
                .map_err(MetalError::CopyToDev)?
                .present_block;
            self.cursor_sync.set(sync);
        }
        let mut cursor_changed = false;
        cursor_changed |= self.cursor_enabled.replace(c.cursor_enabled) != c.cursor_enabled;
        cursor_changed |= swap_buffers;
        cursor_changed |= self.cursor_x.replace(c.cursor_x) != c.cursor_x;
        cursor_changed |= self.cursor_y.replace(c.cursor_y) != c.cursor_y;
        if cursor_changed {
            self.cursor_changed.set(true);
        }
        Ok(())
    }

    fn compute_cursor_programming(
        &self,
        connector_drm_state: &DrmConnectorState,
    ) -> Option<CursorProgramming> {
        if !self.cursor_changed.get() {
            return None;
        }
        let plane = self.cursor_plane.get()?;
        let ty = if self.cursor_enabled.get() {
            let swap = self.cursor_swap_buffer.get();
            let buffers = self.cursor_buffers.get().unwrap();
            let mut front_buffer = connector_drm_state.cursor_fb_idx;
            if swap {
                front_buffer += 1;
            }
            let buffer_idx = (front_buffer % buffers.len() as u64) as usize;
            let buffer = &buffers[buffer_idx];
            CursorProgrammingType::Enable {
                fb: buffer.drm.clone(),
                x: self.cursor_x.get(),
                y: self.cursor_y.get(),
                width: buffer.width,
                height: buffer.height,
                swap,
            }
        } else {
            CursorProgrammingType::Disable
        };
        Some(CursorProgramming { plane, ty })
    }

    fn latch(&self, node: &Rc<OutputNode>, buffer: &RenderBuffer) -> Option<Latched> {
        let damage_count = self.has_damage.get();
        if damage_count == 0 {
            return None;
        }
        node.global.connector.damaged.set(false);
        let damage = {
            node.add_visualizer_damage();
            let damage = &mut *node.global.connector.damage.borrow_mut();
            buffer.damage_queue.damage(damage);
            damage.clear();
            buffer.damage_queue.get()
        };
        let render_hw_cursor = !self.cursor_enabled.get();
        let mode = node.global.mode.get();
        let pass = create_render_pass(
            (mode.width, mode.height),
            &**node,
            &self.state,
            Some(node.node_state[RenderTL].pos.get()),
            node.node_state[RenderTL].scale.get(),
            node.global.persistent.scaling_filter.get(),
            true,
            render_hw_cursor,
            node.has_fullscreen(RenderTL),
            true,
            node.node_state[RenderTL].transform.get(),
            Some(&self.state.damage_visualizer),
            true,
        );
        Some(Latched {
            pass,
            damage_count,
            damage,
            locked: self.state.lock.locked[RenderTL].get(),
        })
    }

    fn trim_scanout_cache(&self) {
        self.scanout_buffers
            .borrow_mut()
            .retain(|_, buffer| buffer.dmabuf.strong_count() > 0);
    }

    fn prepare_direct_scanout(
        &self,
        pass: &GfxRenderPass,
        plane: &Rc<MetalPlane>,
        crtc: &Rc<MetalCrtc>,
        blend_cd: &Rc<ColorDescription>,
        gamma_lut: Option<&Rc<BackendGammaLut>>,
        cd: &Rc<ColorDescription>,
    ) -> Option<(DirectScanoutData, DirectScanoutDataCore)> {
        let (ct, position) = pass.prepare_direct_scanout(
            plane.mode_w.get(),
            plane.mode_h.get(),
            blend_cd,
            self.cursor_enabled.get(),
        )?;
        let fb_resv;
        let dmabuf;
        let release_sync;
        match &ct.client_buf {
            Some(buf) if buf.buffer.buf.exclusive_device == Some(self.dev.id) => {
                fb_resv = Some(buf.clone() as Rc<dyn BufferResv>);
                dmabuf = buf.buffer.buf.client_dmabuf.as_ref();
                release_sync = buf.release_sync;
            }
            _ if self.dev.is_render_device() => {
                if let Some(lazy) = &ct.lazy
                    && lazy.has_lazy_work()
                {
                    // if there is lazy work, the tex dmabuf is not up-to-date. going into
                    // the compositing path ensures that it's up-to-date on the next
                    // frame.
                    return None;
                }
                fb_resv = None;
                dmabuf = ct.tex.dmabuf();
                release_sync = ct.release_sync;
            }
            _ => {
                // at least on AMD, using a FB on a different device for rendering will fail
                // and destroy the render context. it's possible to work around this by waiting
                // until the FB is no longer being scanned out, but if a notification pops up
                // then we must be able to disable direct scanout immediately.
                // https://gitlab.freedesktop.org/drm/amd/-/issues/3186
                return None;
            }
        };
        let Some(dmabuf) = dmabuf else {
            // Shm buffers cannot be scanned out.
            return None;
        };
        let key = DirectScanoutKey {
            dma_buf_id: dmabuf.id,
            plane: plane.id,
            crtc: crtc.id,
            src: ct.cd.id,
            dst: cd.id,
            intent: ct.render_intent,
            gamma_lut: gamma_lut.id(),
            has_cursor_plane: self.cursor_enabled.get(),
            use_plane_color_pipelines: self.dev.use_plane_color_pipelines.get(),
        };
        if let Some(v) = self.scanout_impossible_cache.get(&key) {
            v.mark_used();
            return None;
        }
        let res = self.prepare_direct_scanout2(plane, crtc, gamma_lut, cd, ct, dmabuf);
        if res.is_none() {
            self.scanout_impossible_cache.insert(key, ());
        }
        res.map(|(fb, cm_programming)| {
            let data = DirectScanoutData {
                fb_cd: ct.cd.clone(),
                fb_intent: ct.render_intent,
                key,
                cm_programming,
            };
            let core = DirectScanoutDataCore {
                tex: ct.tex.clone(),
                tex_resv: ct.buffer_resv.clone(),
                acquire_sync: ct.acquire_sync.clone(),
                release_sync,
                _fb_resv: fb_resv,
                lazy: ct.lazy.clone(),
                fb,
                position,
            };
            (data, core)
        })
    }

    fn prepare_direct_scanout2(
        &self,
        plane: &Rc<MetalPlane>,
        crtc: &Rc<MetalCrtc>,
        gamma_lut: Option<&Rc<BackendGammaLut>>,
        cd: &Rc<ColorDescription>,
        ct: &CopyTexture,
        dmabuf: &Rc<DmaBuf>,
    ) -> Option<(Rc<DrmFramebuffer>, MetalCmProgramming)> {
        let fb = self.prepare_direct_scanout3(plane, dmabuf)?;
        let cm_programming = self.cm.find_programming(
            &self.master,
            plane,
            crtc,
            &ct.cd,
            cd,
            ct.render_intent,
            gamma_lut,
            self.cursor_enabled.get(),
            self.dev.use_plane_color_pipelines.get(),
        )?;
        Some((fb, cm_programming))
    }

    fn prepare_direct_scanout3(
        &self,
        plane: &Rc<MetalPlane>,
        dmabuf: &Rc<DmaBuf>,
    ) -> Option<Rc<DrmFramebuffer>> {
        let mut cache = self.scanout_buffers.borrow_mut();
        if let Some(buffer) = cache.get(&dmabuf.id) {
            return buffer.fb.clone();
        }
        let format = 'format: {
            if let Some(f) = plane.formats.get(&dmabuf.format.drm) {
                break 'format f;
            }
            // Try opaque format if possible.
            if let Some(opaque) = dmabuf.format.opaque
                && let Some(f) = plane.formats.get(&opaque.drm)
            {
                break 'format f;
            }
            return None;
        };
        if !format.modifiers.contains(&dmabuf.modifier) {
            return None;
        }
        let fb = match self.dev.master.add_fb(dmabuf, Some(format.format)) {
            Ok(fb) => Some(Rc::new(fb)),
            Err(e) => {
                log::debug!(
                    "Could not import dmabuf for direct scanout: {}",
                    ErrorFmt(e)
                );
                None
            }
        };
        cache.insert(
            dmabuf.id,
            DirectScanoutCache {
                dmabuf: Rc::downgrade(dmabuf),
                fb: fb.clone(),
            },
        );
        fb
    }

    fn prepare_present_fb(
        &self,
        gamma_lut: Option<&Rc<BackendGammaLut>>,
        cd: &Rc<ColorDescription>,
        blend_cd: &Rc<ColorDescription>,
        buffer: &RenderBuffer,
        plane: &Rc<MetalPlane>,
        crtc: &Rc<MetalCrtc>,
        latched: &Latched,
        try_direct_scanout: bool,
    ) -> Result<PresentFb, MetalError> {
        self.trim_scanout_cache();
        let try_direct_scanout = try_direct_scanout && self.dev.direct_scanout_enabled();
        let mut direct_scanout_data = None;
        if try_direct_scanout {
            direct_scanout_data =
                self.prepare_direct_scanout(&latched.pass, plane, crtc, blend_cd, gamma_lut, cd);
        }
        let direct_scanout_active = direct_scanout_data.is_some();
        if self.direct_scanout_active.replace(direct_scanout_active) != direct_scanout_active {
            let change = match direct_scanout_active {
                true => "Enabling",
                false => "Disabling",
            };
            log::debug!("{} direct scanout on {}", change, self.kernel_id());
        }
        let copy;
        let fb;
        let fb_cd;
        let fb_intent;
        let tex;
        let direct_scanout_key;
        let cm_programming;
        let (dsd, mut dsd_core) = direct_scanout_data.unzip();
        match (dsd, &mut dsd_core) {
            (Some(dsd), Some(core)) => {
                if let Some(lazy) = &core.lazy {
                    lazy.record_use(TextureUse::Scanout);
                }
                let sync = match &core.acquire_sync {
                    AcquireSync::None => None,
                    AcquireSync::Implicit => None,
                    AcquireSync::FdSync(sync) => Some(sync.clone()),
                    AcquireSync::Unnecessary => None,
                };
                copy = RenderBufferCopy::for_both(sync);
                fb = core.fb.clone();
                fb_cd = dsd.fb_cd;
                fb_intent = dsd.fb_intent;
                direct_scanout_key = Some(dsd.key);
                tex = core.tex.clone();
                cm_programming = dsd.cm_programming;
            }
            _ => {
                let sf = buffer
                    .render
                    .fb
                    .perform_render_pass(
                        AcquireSync::Unnecessary,
                        ReleaseSync::Explicit,
                        cd,
                        &latched.pass,
                        &latched.damage,
                        buffer.blend_buffer.as_ref(),
                        blend_cd,
                    )
                    .map_err(MetalError::RenderFrame)?;
                copy = buffer
                    .copy_to_dev(cd, Some(&latched.damage), sf)
                    .map_err(MetalError::CopyToDev)?;
                fb = buffer.drm.clone();
                fb_cd = cd.clone();
                fb_intent = RenderIntent::Perceptual;
                direct_scanout_key = None;
                cm_programming = self
                    .cm
                    .find_programming(
                        &self.master,
                        plane,
                        crtc,
                        cd,
                        cd,
                        RenderIntent::Perceptual,
                        gamma_lut,
                        self.cursor_enabled.get(),
                        self.dev.use_plane_color_pipelines.get(),
                    )
                    .ok_or(MetalError::NoCmProgramming)?;
                tex = buffer.render.tex.clone();
            }
        };
        Ok(PresentFb {
            fb_intent,
            copy,
            cm_programming,
            direct_scanout_key,
            core: PresentFbCore {
                fb,
                fb_cd,
                tex,
                direct_scanout_data: dsd_core,
                locked: latched.locked,
            },
        })
    }

    fn perform_screencopies(&self, new_fb: &Option<PresentFb>, output: &OutputNode) {
        let active_fb;
        let fb = match &new_fb {
            Some(f) => &f.core,
            None => {
                active_fb = self.active_framebuffer.borrow();
                match &*active_fb {
                    None => return,
                    Some(f) => f,
                }
            }
        };
        let render_hardware_cursor = self.cursor_enabled.get();
        match &fb.direct_scanout_data {
            None => {
                output.perform_screencopies(
                    &fb.tex,
                    &fb.fb_cd,
                    None,
                    None,
                    &AcquireSync::Unnecessary,
                    ReleaseSync::None,
                    render_hardware_cursor,
                    0,
                    0,
                    None,
                );
            }
            Some(dsd) => {
                output.perform_screencopies(
                    &dsd.tex,
                    &fb.fb_cd,
                    dsd.tex_resv.as_ref(),
                    dsd.lazy.as_ref(),
                    &dsd.acquire_sync,
                    dsd.release_sync,
                    render_hardware_cursor,
                    dsd.position.crtc_x,
                    dsd.position.crtc_y,
                    Some((dsd.position.crtc_width, dsd.position.crtc_height)),
                );
            }
        }
    }
}
