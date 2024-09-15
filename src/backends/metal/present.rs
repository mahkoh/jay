use {
    crate::{
        backend::Connector,
        backends::metal::{
            video::{
                MetalConnector, MetalCrtc, MetalHardwareCursorChange, MetalPlane, RenderBuffer,
            },
            MetalError,
        },
        gfx_api::{
            create_render_pass, AcquireSync, BufferResv, GfxApiOpt, GfxRenderPass, GfxTexture,
            SyncFile,
        },
        theme::Color,
        time::Time,
        tracy::FrameName,
        tree::OutputNode,
        utils::{errorfmt::ErrorFmt, oserror::OsError, transform_ext::TransformExt},
        video::{
            dmabuf::DmaBufId,
            drm::{
                DrmError, DrmFramebuffer, DRM_MODE_ATOMIC_NONBLOCK, DRM_MODE_PAGE_FLIP_ASYNC,
                DRM_MODE_PAGE_FLIP_EVENT,
            },
        },
    },
    std::rc::{Rc, Weak},
    uapi::c,
};

struct Latched {
    pass: GfxRenderPass,
    damage: u64,
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
    _resv: Option<Rc<dyn BufferResv>>,
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
}

enum CursorProgramming {
    Enable {
        plane: Rc<MetalPlane>,
        fb: Rc<DrmFramebuffer>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        swap: bool,
    },
    Disable {
        plane: Rc<MetalPlane>,
    },
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
            if !self.can_present.get() {
                continue;
            }
            let mut expected_sequence = self.sequence.get() + 1;
            let mut start = Time::now_unchecked();
            let use_frame_scheduling = !self.try_async_flip();
            if use_frame_scheduling {
                let next_present = self
                    .next_flip_nsec
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
            if let Err(e) = self.present_once().await {
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

    async fn present_once(&self) -> Result<(), MetalError> {
        let version = self.version.get();
        if !self.can_present.get() {
            return Ok(());
        }
        if !self.backend.check_render_context(&self.dev) {
            return Ok(());
        }
        let Some(node) = self.state.root.outputs.get(&self.connector_id) else {
            return Ok(());
        };
        let crtc = match self.crtc.get() {
            Some(crtc) => crtc,
            _ => return Ok(()),
        };
        if !crtc.active.value.get() {
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

        self.latch_cursor(&node)?;
        let cursor_programming = self.compute_cursor_programming();
        let latched = self.latch(&node);
        node.latched();

        if cursor_programming.is_none() && latched.is_none() {
            return Ok(());
        }

        let buffer = &buffers[self.next_buffer.get() % buffers.len()];
        let mut present_fb = None;
        let mut direct_scanout_id = None;
        if let Some(latched) = &latched {
            let fb = self.prepare_present_fb(buffer, &plane, &latched.pass, true)?;
            direct_scanout_id = fb.direct_scanout_data.as_ref().map(|d| d.dma_buf_id);
            present_fb = Some(fb);
        }
        self.perform_screencopies(&present_fb, &node);
        if let Some(sync_file) = self.cursor_sync_file.take() {
            if let Err(e) = self.state.ring.readable(&sync_file).await {
                log::error!(
                    "Could not wait for cursor sync file to complete: {}",
                    ErrorFmt(e)
                );
            }
        }
        self.await_present_fb(present_fb.as_mut()).await;
        let mut res = self.program_connector(
            version,
            &crtc,
            &plane,
            cursor_programming.as_ref(),
            present_fb.as_ref(),
        );
        if res.is_err() {
            if let Some(dsd_id) = direct_scanout_id {
                let fb = self.prepare_present_fb(
                    buffer,
                    &plane,
                    &latched.as_ref().unwrap().pass,
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
        }
        if let Err(e) = res {
            if let MetalError::Commit(DrmError::Atomic(OsError(c::EACCES))) = e {
                log::debug!("Could not perform atomic commit, likely because we're no longer the DRM master");
                return Ok(());
            }
            Err(e)
        } else {
            macro_rules! apply_change {
                ($prop:expr) => {
                    if let Some(v) = $prop.pending_value.take() {
                        $prop.value.set(v);
                    }
                };
            }
            apply_change!(plane.src_w);
            apply_change!(plane.src_h);
            apply_change!(plane.crtc_x);
            apply_change!(plane.crtc_y);
            apply_change!(plane.crtc_w);
            apply_change!(plane.crtc_h);
            if let Some(fb) = present_fb {
                self.presentation_is_zero_copy
                    .set(fb.direct_scanout_data.is_some());
                if fb.direct_scanout_data.is_none() {
                    self.next_buffer.fetch_add(1);
                }
                self.next_framebuffer.set(Some(fb));
            }
            if let Some(CursorProgramming::Enable { swap: true, .. }) = cursor_programming {
                self.cursor_swap_buffer.set(false);
                self.cursor_front_buffer.fetch_add(1);
            }
            self.can_present.set(false);
            if let Some(latched) = latched {
                self.has_damage.fetch_sub(latched.damage);
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
        self.tearing_requested.get() && self.dev.supports_async_commit
    }

    fn program_connector(
        &self,
        version: u64,
        crtc: &Rc<MetalCrtc>,
        plane: &Rc<MetalPlane>,
        cursor: Option<&CursorProgramming>,
        new_fb: Option<&PresentFb>,
    ) -> Result<(), MetalError> {
        zone!("program_connector");
        let mut changes = self.master.change();
        let mut try_async_flip = self.try_async_flip();
        macro_rules! change {
            ($c:expr, $prop:expr, $new:expr) => {{
                if $prop.value.get() != $new {
                    $c.change($prop.id, $new as u64);
                    try_async_flip = false;
                    $prop.pending_value.set(Some($new));
                }
            }};
        }
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
                c.change(plane.fb_id, fb.fb.id().0 as _);
                change!(c, plane.src_w, (src_width as u32) << 16);
                change!(c, plane.src_h, (src_height as u32) << 16);
                change!(c, plane.crtc_x, crtc_x);
                change!(c, plane.crtc_y, crtc_y);
                change!(c, plane.crtc_w, crtc_w);
                change!(c, plane.crtc_h, crtc_h);
                if !try_async_flip && !self.dev.is_nvidia {
                    if let Some(sf) = self.backend.signaled_sync_file.get() {
                        c.change(plane.in_fence_fd, sf.0.raw() as u64);
                    }
                }
            });
        } else {
            if self.dev.is_amd && crtc.vrr_enabled.value.get() {
                // Work around https://gitlab.freedesktop.org/drm/amd/-/issues/2186
                if let Some(fb) = &*self.active_framebuffer.borrow() {
                    changes.change_object(plane.id, |c| {
                        c.change(plane.fb_id, fb.fb.id().0 as _);
                    });
                }
            }
        }
        if let Some(cursor) = cursor {
            try_async_flip = false;
            match cursor {
                CursorProgramming::Enable {
                    plane,
                    fb,
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    changes.change_object(plane.id, |c| {
                        c.change(plane.fb_id, fb.id().0 as _);
                        c.change(plane.crtc_id.id, crtc.id.0 as _);
                        c.change(plane.crtc_x.id, *x as _);
                        c.change(plane.crtc_y.id, *y as _);
                        c.change(plane.crtc_w.id, *width as _);
                        c.change(plane.crtc_h.id, *height as _);
                        c.change(plane.src_x.id, 0);
                        c.change(plane.src_y.id, 0);
                        c.change(plane.src_w.id, (*width as u64) << 16);
                        c.change(plane.src_h.id, (*height as u64) << 16);
                        if !self.dev.is_nvidia {
                            if let Some(sf) = self.backend.signaled_sync_file.get() {
                                c.change(plane.in_fence_fd, sf.0.raw() as u64);
                            }
                        }
                    });
                }
                CursorProgramming::Disable { plane } => {
                    changes.change_object(plane.id, |c| {
                        c.change(plane.fb_id, 0);
                        c.change(plane.crtc_id.id, 0);
                    });
                }
            }
        }
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
            res = changes.commit(FLAGS, 0);
        }
        res.map_err(MetalError::Commit)
    }

    fn latch_cursor(&self, node: &Rc<OutputNode>) -> Result<(), MetalError> {
        if !self.cursor_damage.take() {
            return Ok(());
        }
        if self.cursor_plane.is_none() {
            return Ok(());
        }
        let buffers = self.cursor_buffers.get().unwrap();
        let mut c = MetalHardwareCursorChange {
            cursor_enabled: self.cursor_enabled.get(),
            cursor_swap_buffer: false,
            cursor_x: self.cursor_x.get(),
            cursor_y: self.cursor_y.get(),
            cursor_buffer: &buffers[(self.cursor_front_buffer.get() + 1) % buffers.len()],
            sync_file: None,
            cursor_size: (self.dev.cursor_width as _, self.dev.cursor_height as _),
        };
        self.state.present_hardware_cursor(node, &mut c);
        if c.cursor_swap_buffer {
            c.sync_file = c.cursor_buffer.copy_to_dev(c.sync_file)?;
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

    fn compute_cursor_programming(&self) -> Option<CursorProgramming> {
        if !self.cursor_changed.get() {
            return None;
        }
        let plane = self.cursor_plane.get()?;
        let programming = if self.cursor_enabled.get() {
            let swap = self.cursor_swap_buffer.get();
            let mut front_buffer = self.cursor_front_buffer.get();
            if swap {
                front_buffer = front_buffer.wrapping_add(1);
            }
            let buffers = self.cursor_buffers.get().unwrap();
            let buffer = &buffers[front_buffer % buffers.len()];
            let (width, height) = buffer.dev_fb.physical_size();
            CursorProgramming::Enable {
                plane,
                fb: buffer.drm.clone(),
                x: self.cursor_x.get(),
                y: self.cursor_y.get(),
                width,
                height,
                swap,
            }
        } else {
            CursorProgramming::Disable { plane }
        };
        Some(programming)
    }

    fn latch(&self, node: &Rc<OutputNode>) -> Option<Latched> {
        let damage = self.has_damage.get();
        if damage == 0 {
            return None;
        }
        node.global.connector.damaged.set(false);
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
            node.global.persistent.transform.get(),
            Some(&self.state.damage_visualizer),
        );
        Some(Latched { pass, damage })
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
                        if fr.color == Color::SOLID_BLACK {
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
            if let Some(clear) = pass.clear {
                if clear != Color::SOLID_BLACK {
                    // Background could be visible.
                    return None;
                }
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
                _resv: ct.buffer_resv.clone(),
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
            if let Some(opaque) = dmabuf.format.opaque {
                if let Some(f) = plane.formats.get(&opaque.drm) {
                    break 'format f;
                }
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
                _resv: ct.buffer_resv.clone(),
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
        buffer: &RenderBuffer,
        plane: &Rc<MetalPlane>,
        pass: &GfxRenderPass,
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
            direct_scanout_data = self.prepare_direct_scanout(&pass, plane);
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
                    .perform_render_pass(pass)
                    .map_err(MetalError::RenderFrame)?;
                sync_file = buffer.copy_to_dev(sf)?;
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
        })
    }

    fn perform_screencopies(&self, new_fb: &Option<PresentFb>, output: &OutputNode) {
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
                output.perform_screencopies(&fb.tex, render_hardware_cursor, 0, 0, None);
            }
            Some(dsd) => {
                output.perform_screencopies(
                    &dsd.tex,
                    render_hardware_cursor,
                    dsd.position.crtc_x,
                    dsd.position.crtc_y,
                    Some((dsd.position.crtc_width, dsd.position.crtc_height)),
                );
            }
        }
    }
}
