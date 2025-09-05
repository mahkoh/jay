use {
    crate::{
        backend::Connector,
        backends::metal::{
            MetalError,
            transaction::{DrmConnectorState, DrmPlaneState},
            video::{
                MetalConnector, MetalCrtc, MetalHardwareCursorChange, MetalPlane, RenderBuffer,
            },
        },
        cmm::cmm_description::ColorDescription,
        gfx_api::{
            AcquireSync, BufferResv, GfxApiOpt, GfxRenderPass, GfxTexture, ReleaseSync, SyncFile,
            create_render_pass,
        },
        rect::Region,
        theme::Color,
        time::Time,
        tracy::FrameName,
        tree::OutputNode,
        utils::{errorfmt::ErrorFmt, oserror::OsError, transform_ext::TransformExt},
        video::{
            dmabuf::DmaBufId,
            drm::{
                DRM_MODE_ATOMIC_NONBLOCK, DRM_MODE_PAGE_FLIP_ASYNC, DRM_MODE_PAGE_FLIP_EVENT,
                DrmCrtc, DrmError, DrmFb, DrmFramebuffer, DrmObject,
            },
        },
    },
    arrayvec::ArrayVec,
    std::rc::{Rc, Weak},
    uapi::{OwnedFd, c},
};

struct Latched {
    pass: GfxRenderPass,
    damage_count: u64,
    damage: Region,
    locked: bool,
}

#[derive(Debug)]
pub struct DirectScanoutCache {
    tex: Weak<dyn GfxTexture>,
    fb: Option<Rc<DrmFramebuffer>>,
}

#[derive(Debug)]
pub struct DirectScanoutData {
    tex: Rc<dyn GfxTexture>,
    acquire_sync: AcquireSync,
    release_sync: ReleaseSync,
    resv: Option<Rc<dyn BufferResv>>,
    fb: Rc<DrmFramebuffer>,
    dma_buf_id: DmaBufId,
    position: DirectScanoutPosition,
}

#[derive(Debug)]
pub struct DirectScanoutPosition {
    pub src_width: i32,
    pub src_height: i32,
    pub crtc_x: i32,
    pub crtc_y: i32,
    pub crtc_width: i32,
    pub crtc_height: i32,
}

pub struct PresentFb {
    fb: Rc<DrmFramebuffer>,
    tex: Rc<dyn GfxTexture>,
    direct_scanout_data: Option<DirectScanoutData>,
    sync_file: Option<SyncFile>,
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
        if !crtc.drm_state.borrow().active {
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

        let cd = node.global.color_description.get();
        let blend_cd = self.state.color_manager.srgb_gamma22();

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
        let mut direct_scanout_id = None;
        if let Some(latched) = &latched {
            let fb = self.prepare_present_fb(&cd, blend_cd, buffer, &plane, latched, true)?;
            direct_scanout_id = fb.direct_scanout_data.as_ref().map(|d| d.dma_buf_id);
            present_fb = Some(fb);
        }
        self.perform_screencopies(&present_fb, &node, &cd);
        if let Some(sync_file) = self.cursor_sync_file.take()
            && let Err(e) = self.state.ring.readable(&sync_file).await
        {
            log::error!(
                "Could not wait for cursor sync file to complete: {}",
                ErrorFmt(e)
            );
        }
        self.await_present_fb(present_fb.as_mut()).await;
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
            && let Some(dsd_id) = direct_scanout_id
        {
            let fb = self.prepare_present_fb(
                &cd,
                blend_cd,
                buffer,
                &plane,
                latched.as_ref().unwrap(),
                false,
            )?;
            present_fb = Some(fb);
            self.await_present_fb(present_fb.as_mut()).await;
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
                let mut cache = self.scanout_buffers.borrow_mut();
                if let Some(buffer) = cache.remove(&dsd_id) {
                    cache.insert(
                        dsd_id,
                        DirectScanoutCache {
                            tex: buffer.tex,
                            fb: None,
                        },
                    );
                }
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
                self.presentation_is_zero_copy
                    .set(fb.direct_scanout_data.is_some());
                if fb.direct_scanout_data.is_none() {
                    buffer.damage_queue.clear();
                } else {
                    reset_damage();
                }
                buffer.locked.set(fb.locked);
                self.next_framebuffer.set(Some(fb));
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

    async fn await_present_fb(&self, new_fb: Option<&mut PresentFb>) {
        let Some(fb) = new_fb else {
            return;
        };
        let Some(sync_file) = fb.sync_file.take() else {
            return;
        };
        if let Err(e) = self.state.ring.readable(&sync_file).await {
            log::error!(
                "Could not wait for primary sync file to complete: {}",
                ErrorFmt(e)
            );
        }
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
                match &fb.direct_scanout_data {
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
            changes.change_object(plane.id, |c| {
                c.change(plane.fb_id, fb.fb.id());
                drm_state.fb_id = fb.fb.id();
                connector_state.fb = fb.fb.id();
                connector_state.locked = fb.locked;
                if fb.direct_scanout_data.is_none() {
                    connector_state.fb_idx += 1;
                }
                macro_rules! change {
                    ($prop:ident, $new:expr) => {{
                        if drm_state.$prop != $new {
                            c.change(plane.$prop, $new as u64);
                            try_async_flip = false;
                            drm_state.$prop = $new;
                        }
                        connector_state.$prop = drm_state.$prop;
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
                        c.change(plane.$prop, $new);
                        drm_state.$prop = $new;
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
                        if !self.dev.is_nvidia
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
            if !self.dev.is_nvidia {
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
            cursor_swap_buffer: false,
            cursor_x: self.cursor_x.get(),
            cursor_y: self.cursor_y.get(),
            cursor_buffer: &buffers[buffer_idx],
            sync_file: None,
            cursor_size: (self.dev.cursor_width as _, self.dev.cursor_height as _),
        };
        self.state.present_hardware_cursor(node, &mut c);
        if c.cursor_swap_buffer {
            c.sync_file = c.cursor_buffer.copy_to_dev(cd, c.sync_file)?;
        }
        self.cursor_swap_buffer.set(c.cursor_swap_buffer);
        if c.sync_file.is_some() {
            self.cursor_sync_file.set(c.sync_file);
        }
        let mut cursor_changed = false;
        cursor_changed |= self.cursor_enabled.replace(c.cursor_enabled) != c.cursor_enabled;
        cursor_changed |= c.cursor_swap_buffer;
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
            let (width, height) = buffer.dev_fb.physical_size();
            CursorProgrammingType::Enable {
                fb: buffer.drm.clone(),
                x: self.cursor_x.get(),
                y: self.cursor_y.get(),
                width,
                height,
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
            node.global.add_visualizer_damage();
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
            Some(node.global.pos.get()),
            node.global.persistent.scale.get(),
            true,
            render_hw_cursor,
            node.has_fullscreen(),
            true,
            node.global.persistent.transform.get(),
            Some(&self.state.damage_visualizer),
        );
        Some(Latched {
            pass,
            damage_count,
            damage,
            locked: self.state.lock.locked.get(),
        })
    }

    fn trim_scanout_cache(&self) {
        self.scanout_buffers
            .borrow_mut()
            .retain(|_, buffer| buffer.tex.strong_count() > 0);
    }

    fn prepare_direct_scanout(
        &self,
        pass: &GfxRenderPass,
        plane: &Rc<MetalPlane>,
        cd: &Rc<ColorDescription>,
    ) -> Option<DirectScanoutData> {
        let ct = 'ct: {
            let mut ops = pass.ops.iter().rev();
            let ct = 'ct2: {
                for opt in &mut ops {
                    match opt {
                        GfxApiOpt::Sync => {}
                        GfxApiOpt::FillRect(_) => {
                            // Top-most layer must be a texture.
                            return None;
                        }
                        GfxApiOpt::CopyTexture(ct) => break 'ct2 ct,
                    }
                }
                return None;
            };
            if !ct.cd.embeds_into(cd) {
                // Direct scanout requires embeddable color descriptions.
                return None;
            }
            if ct.alpha.is_some() {
                // Direct scanout with alpha factor is not supported.
                return None;
            }
            if !ct.tex.format().has_alpha && ct.target.is_covering() {
                // Texture covers the entire screen and is opaque.
                break 'ct ct;
            }
            for opt in ops {
                match opt {
                    GfxApiOpt::Sync => {}
                    GfxApiOpt::FillRect(fr) => {
                        if fr.effective_color() == Color::SOLID_BLACK {
                            // Black fills can be ignored because this is the CRTC background color.
                            if fr.rect.is_covering() {
                                // If fill covers the entire screen, we don't have to look further.
                                break 'ct ct;
                            }
                        } else {
                            // Fill could be visible.
                            return None;
                        }
                    }
                    GfxApiOpt::CopyTexture(_) => {
                        // Texture could be visible.
                        return None;
                    }
                }
            }
            if let Some(clear) = pass.clear
                && clear != Color::SOLID_BLACK
            {
                // Background could be visible.
                return None;
            }
            ct
        };
        if let AcquireSync::None | AcquireSync::Implicit = ct.acquire_sync {
            // Cannot perform scanout without explicit sync.
            return None;
        }
        if ct.source.buffer_transform != ct.target.output_transform {
            // Rotations and mirroring are not supported.
            return None;
        }
        if !ct.source.is_covering() {
            // Viewports are not supported.
            return None;
        }
        if ct.target.x1 < -1.0 || ct.target.y1 < -1.0 || ct.target.x2 > 1.0 || ct.target.y2 > 1.0 {
            // Rendering outside the screen is not supported.
            return None;
        }
        let (tex_w, tex_h) = ct.tex.size();
        let (x1, x2, y1, y2) = {
            let plane_w = plane.mode_w.get() as f32;
            let plane_h = plane.mode_h.get() as f32;
            let ((x1, x2), (y1, y2)) = ct
                .target
                .output_transform
                .maybe_swap(((ct.target.x1, ct.target.x2), (ct.target.y1, ct.target.y2)));
            (
                (x1 + 1.0) * plane_w / 2.0,
                (x2 + 1.0) * plane_w / 2.0,
                (y1 + 1.0) * plane_h / 2.0,
                (y2 + 1.0) * plane_h / 2.0,
            )
        };
        let (crtc_w, crtc_h) = (x2 - x1, y2 - y1);
        if crtc_w < 0.0 || crtc_h < 0.0 {
            // Flipping x or y axis is not supported.
            return None;
        }
        if self.cursor_enabled.get() && (tex_w as f32, tex_h as f32) != (crtc_w, crtc_h) {
            // If hardware cursors are used, we cannot scale the texture.
            return None;
        }
        let Some(dmabuf) = ct.tex.dmabuf() else {
            // Shm buffers cannot be scanned out.
            return None;
        };
        let position = DirectScanoutPosition {
            src_width: tex_w,
            src_height: tex_h,
            crtc_x: x1 as _,
            crtc_y: y1 as _,
            crtc_width: crtc_w as _,
            crtc_height: crtc_h as _,
        };
        let mut cache = self.scanout_buffers.borrow_mut();
        if let Some(buffer) = cache.get(&dmabuf.id) {
            return buffer.fb.as_ref().map(|fb| DirectScanoutData {
                tex: buffer.tex.upgrade().unwrap(),
                acquire_sync: ct.acquire_sync.clone(),
                release_sync: ct.release_sync,
                resv: ct.buffer_resv.clone(),
                fb: fb.clone(),
                dma_buf_id: dmabuf.id,
                position,
            });
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
        let data = match self.dev.master.add_fb(dmabuf, Some(format.format)) {
            Ok(fb) => Some(DirectScanoutData {
                tex: ct.tex.clone(),
                acquire_sync: ct.acquire_sync.clone(),
                release_sync: ct.release_sync,
                resv: ct.buffer_resv.clone(),
                fb: Rc::new(fb),
                dma_buf_id: dmabuf.id,
                position,
            }),
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
                tex: Rc::downgrade(&ct.tex),
                fb: data.as_ref().map(|dsd| dsd.fb.clone()),
            },
        );
        data
    }

    fn direct_scanout_enabled(&self) -> bool {
        self.dev
            .direct_scanout_enabled
            .get()
            .unwrap_or(self.state.direct_scanout_enabled.get())
    }

    fn prepare_present_fb(
        &self,
        cd: &Rc<ColorDescription>,
        blend_cd: &Rc<ColorDescription>,
        buffer: &RenderBuffer,
        plane: &Rc<MetalPlane>,
        latched: &Latched,
        try_direct_scanout: bool,
    ) -> Result<PresentFb, MetalError> {
        self.trim_scanout_cache();
        let try_direct_scanout = try_direct_scanout
            && self.direct_scanout_enabled()
            // at least on AMD, using a FB on a different device for rendering will fail
            // and destroy the render context. it's possible to work around this by waiting
            // until the FB is no longer being scanned out, but if a notification pops up
            // then we must be able to disable direct scanout immediately.
            // https://gitlab.freedesktop.org/drm/amd/-/issues/3186
            && self.dev.is_render_device();
        let mut direct_scanout_data = None;
        if try_direct_scanout {
            direct_scanout_data = self.prepare_direct_scanout(&latched.pass, plane, cd);
        }
        let direct_scanout_active = direct_scanout_data.is_some();
        if self.direct_scanout_active.replace(direct_scanout_active) != direct_scanout_active {
            let change = match direct_scanout_active {
                true => "Enabling",
                false => "Disabling",
            };
            log::debug!("{} direct scanout on {}", change, self.kernel_id());
        }
        let sync_file;
        let fb;
        let tex;
        match &direct_scanout_data {
            None => {
                let sf = buffer
                    .render_fb()
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
                sync_file = buffer.copy_to_dev(cd, sf)?;
                fb = buffer.drm.clone();
                tex = buffer.render_tex.clone();
            }
            Some(dsd) => {
                sync_file = match &dsd.acquire_sync {
                    AcquireSync::None => None,
                    AcquireSync::Implicit => None,
                    AcquireSync::SyncFile { sync_file } => Some(sync_file.clone()),
                    AcquireSync::Unnecessary => None,
                };
                fb = dsd.fb.clone();
                tex = dsd.tex.clone();
            }
        };
        Ok(PresentFb {
            fb,
            tex,
            direct_scanout_data,
            sync_file,
            locked: latched.locked,
        })
    }

    fn perform_screencopies(
        &self,
        new_fb: &Option<PresentFb>,
        output: &OutputNode,
        cd: &Rc<ColorDescription>,
    ) {
        let active_fb;
        let fb = match &new_fb {
            Some(f) => f,
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
                    cd,
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
                    cd,
                    dsd.resv.as_ref(),
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
