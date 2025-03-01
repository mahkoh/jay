use {
    crate::{
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        gfx_api::{
            AcquireSync, AsyncShmGfxTextureCallback, BufferResv, GfxError, GfxFramebuffer,
            GfxTexture, ReleaseSync, STAGING_DOWNLOAD, SyncFile,
        },
        ifs::{
            ext_image_capture_source_v1::ImageCaptureSource,
            ext_image_copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
            wl_buffer::WlBufferStorage,
        },
        leaks::Tracker,
        object::Object,
        rect::Region,
        tree::{Node, OutputNode},
        utils::{cell_ext::CellExt, errorfmt::ErrorFmt, transform_ext::TransformExt},
        wire::{ExtImageCopyCaptureFrameV1Id, ext_image_copy_capture_frame_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum FrameStatus {
    Unused,
    Capturing,
    Captured,
    Ready,
    Failed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum FrameFailureReason {
    Unknown,
    BufferConstraints,
    Stopped,
}

pub struct ExtImageCopyCaptureFrameV1 {
    pub(super) id: ExtImageCopyCaptureFrameV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) session: Rc<ExtImageCopyCaptureSessionV1>,
}

impl ExtImageCopyCaptureFrameV1 {
    fn ensure_unused(&self) -> Result<(), ExtImageCopyCaptureFrameV1Error> {
        if self.session.status.get() != FrameStatus::Unused {
            return Err(ExtImageCopyCaptureFrameV1Error::AlreadyCaptured);
        }
        Ok(())
    }

    pub(super) fn fail(&self, reason: FrameFailureReason) {
        let reason = match reason {
            FrameFailureReason::Unknown => 0,
            FrameFailureReason::BufferConstraints => 1,
            FrameFailureReason::Stopped => 2,
        };
        self.client.event(Failed {
            self_id: self.id,
            reason,
        });
        self.session.status.set(FrameStatus::Failed);
        self.session.presentation_listener.detach();
        self.session.buffer.take();
        self.session.pending_download.take();
        self.session.force_capture.set(true);
    }

    fn try_copy(
        self: &Rc<Self>,
        on: &OutputNode,
        size: (i32, i32),
        f: impl FnOnce(
            Rc<dyn GfxFramebuffer>,
            AcquireSync,
            ReleaseSync,
        ) -> Result<Option<SyncFile>, GfxError>,
    ) -> Result<(), FrameFailureReason> {
        let Some(ctx) = self.client.state.render_ctx.get() else {
            return Err(FrameFailureReason::BufferConstraints);
        };
        let buffer = self.session.buffer.get().unwrap();
        if size != buffer.rect.size() {
            self.session.buffer_size_changed();
            // https://gitlab.freedesktop.org/wayland/wayland-protocols/-/issues/222
            // self.fail(FrameFailureReason::BufferConstraints);
            // return;
        }
        if let Err(e) = buffer.update_framebuffer() {
            log::error!("Could not import buffer: {}", ErrorFmt(e));
            return Err(FrameFailureReason::BufferConstraints);
        }
        let storage = &*buffer.storage.borrow();
        let Some(storage) = storage else {
            return Err(FrameFailureReason::BufferConstraints);
        };
        let mut shm_bridge = self.session.shm_bridge.take();
        let mut shm_staging = self.session.shm_staging.take();
        match storage {
            WlBufferStorage::Shm { mem, stride } => {
                if let Some(b) = &shm_bridge {
                    if b.physical_size() != buffer.rect.size()
                        || b.format() != buffer.format
                        || b.stride() != *stride
                    {
                        shm_bridge = None;
                    }
                }
                let bridge = match shm_bridge {
                    Some(b) => b,
                    _ => {
                        let res = ctx.clone().create_internal_fb(
                            &self.client.state.cpu_worker,
                            buffer.rect.width(),
                            buffer.rect.height(),
                            *stride,
                            buffer.format,
                        );
                        match res {
                            Ok(b) => b,
                            Err(e) => {
                                log::error!("Could not allocate staging fb: {}", ErrorFmt(e));
                                return Err(FrameFailureReason::Unknown);
                            }
                        }
                    }
                };
                if let Some(s) = &shm_staging {
                    if s.size() != bridge.staging_size() {
                        shm_staging = None;
                    }
                }
                let staging = match shm_staging {
                    Some(s) => s,
                    _ => ctx.create_staging_buffer(bridge.staging_size(), STAGING_DOWNLOAD),
                };
                let res = f(
                    bridge.clone().into_fb(),
                    AcquireSync::Unnecessary,
                    ReleaseSync::None,
                );
                if let Err(e) = res {
                    log::error!("Could not copy frame to staging texture: {}", ErrorFmt(e));
                    return Err(FrameFailureReason::Unknown);
                }
                let res = bridge.clone().download(
                    &staging,
                    self.clone(),
                    mem.clone(),
                    Region::new2(buffer.rect),
                );
                match res {
                    Ok(d) => self.session.pending_download.set(d),
                    Err(e) => {
                        log::error!("Could not initiate bridge download: {}", ErrorFmt(e));
                        return Err(FrameFailureReason::Unknown);
                    }
                }
                self.session.shm_bridge.set(Some(bridge));
                self.session.shm_staging.set(Some(staging));
            }
            WlBufferStorage::Dmabuf { fb, .. } => {
                let Some(fb) = fb else {
                    return Err(FrameFailureReason::BufferConstraints);
                };
                let res = f(fb.clone(), AcquireSync::Implicit, ReleaseSync::Implicit);
                if let Err(e) = res {
                    log::error!("Could not copy frame to client fb: {}", ErrorFmt(e));
                    return Err(FrameFailureReason::Unknown);
                }
            }
        }
        self.session
            .presentation_listener
            .attach(&on.presentation_event);
        Ok(())
    }

    fn copy(
        self: &Rc<Self>,
        on: &OutputNode,
        size: (i32, i32),
        f: impl FnOnce(
            Rc<dyn GfxFramebuffer>,
            AcquireSync,
            ReleaseSync,
        ) -> Result<Option<SyncFile>, GfxError>,
    ) {
        match self.try_copy(on, size, f) {
            Ok(()) => self.session.status.set(FrameStatus::Captured),
            Err(e) => self.fail(e),
        }
    }

    pub(super) fn copy_texture(
        self: &Rc<Self>,
        on: &OutputNode,
        texture: &Rc<dyn GfxTexture>,
        cd: &Rc<ColorDescription>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
        let transform = on.global.persistent.transform.get();
        let req_size = size.unwrap_or(transform.maybe_swap(texture.size()));
        self.copy(on, req_size, |fb, aq, re| {
            self.client.state.perform_screencopy(
                texture,
                resv,
                acquire_sync,
                release_sync,
                cd,
                &fb,
                aq,
                re,
                jay_config::video::Transform::None,
                self.client.state.color_manager.srgb_srgb(),
                on.global.pos.get(),
                render_hardware_cursors,
                x_off,
                y_off,
                size,
                transform,
                on.global.persistent.scale.get(),
            )
        });
    }

    pub(super) fn copy_node(self: &Rc<Self>, on: &OutputNode, node: &dyn Node, size: (i32, i32)) {
        let scale = on.global.persistent.scale.get();
        self.copy(on, size, |fb, aq, re| {
            fb.render_node(
                aq,
                re,
                self.client.state.color_manager.srgb_srgb(),
                node,
                &self.client.state,
                Some(node.node_absolute_position()),
                scale,
                true,
                true,
                true,
                false,
                jay_config::video::Transform::None,
                None,
                self.client.state.color_manager.srgb_linear(),
            )
        });
    }

    pub(super) fn maybe_ready(&self) {
        if self.session.pending_download.is_some() {
            return;
        }
        let Some((tv_sec, tv_nsec)) = self.session.presented.get() else {
            return;
        };
        if let Some(buffer) = self.session.buffer.get() {
            self.client.event(Damage {
                self_id: self.id,
                x: 0,
                y: 0,
                width: buffer.rect.width(),
                height: buffer.rect.height(),
            });
        }
        self.client.event(PresentationTime {
            self_id: self.id,
            tv_sec_hi: (tv_sec >> 32) as u32,
            tv_sec_lo: tv_sec as u32,
            tv_nsec,
        });
        self.client.event(Ready { self_id: self.id });
        self.session.status.set(FrameStatus::Ready);
    }
}

impl ExtImageCopyCaptureFrameV1RequestHandler for ExtImageCopyCaptureFrameV1 {
    type Error = ExtImageCopyCaptureFrameV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.session.status.get() == FrameStatus::Captured {
            self.session.shm_staging.take();
            self.session.shm_bridge.take();
        }
        self.session.frame.take();
        self.session.presentation_listener.detach();
        self.session.presented.take();
        self.session.pending_download.take();
        self.session.status.set(FrameStatus::Unused);
        self.session.buffer.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn attach_buffer(&self, req: AttachBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ensure_unused()?;
        let buffer = self.client.lookup(req.buffer)?;
        self.session.buffer.set(Some(buffer));
        self.session.size_debounce.set(false);
        Ok(())
    }

    fn damage_buffer(&self, _req: DamageBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ensure_unused()?;
        Ok(())
    }

    fn capture(&self, _req: Capture, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ensure_unused()?;
        if self.session.buffer.is_none() {
            return Err(ExtImageCopyCaptureFrameV1Error::NoBuffer);
        }
        if self.session.stopped.get() {
            self.fail(FrameFailureReason::Stopped);
            return Ok(());
        }
        self.session.status.set(FrameStatus::Capturing);
        if self.session.force_capture.get() {
            self.session.force_capture.set(false);
            match &self.session.source {
                ImageCaptureSource::Output(o) => {
                    if let Some(node) = o.node.get() {
                        node.global.connector.damage();
                    }
                }
                ImageCaptureSource::Toplevel(tl) => {
                    if let Some(tl) = tl.get() {
                        tl.tl_data().output().global.connector.damage();
                    }
                }
            }
        }
        Ok(())
    }
}

impl AsyncShmGfxTextureCallback for ExtImageCopyCaptureFrameV1 {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>) {
        self.session.pending_download.take();
        if self.session.status.get() != FrameStatus::Captured {
            return;
        }
        if let Err(e) = res {
            log::error!("Bridge download failed: {}", ErrorFmt(e));
            self.fail(FrameFailureReason::Unknown);
            return;
        }
        self.maybe_ready();
    }
}

object_base! {
    self = ExtImageCopyCaptureFrameV1;
    version = self.session.version;
}

impl Object for ExtImageCopyCaptureFrameV1 {}

simple_add_obj!(ExtImageCopyCaptureFrameV1);

#[derive(Debug, Error)]
pub enum ExtImageCopyCaptureFrameV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The frame has already been captured")]
    AlreadyCaptured,
    #[error("The frame does not have a buffer attached")]
    NoBuffer,
}
efrom!(ExtImageCopyCaptureFrameV1Error, ClientError);
