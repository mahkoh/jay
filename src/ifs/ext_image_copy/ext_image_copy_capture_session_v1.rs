use {
    crate::{
        client::{Client, ClientError},
        format::{Format, FORMATS},
        gfx_api::{
            AcquireSync, BufferResv, GfxInternalFramebuffer, GfxStagingBuffer, GfxTexture,
            PendingShmTransfer, ReleaseSync,
        },
        ifs::{
            ext_image_capture_source_v1::ImageCaptureSource,
            ext_image_copy::ext_image_copy_capture_frame_v1::{
                ExtImageCopyCaptureFrameV1, FrameFailureReason, FrameStatus,
            },
            wl_buffer::WlBuffer,
        },
        leaks::Tracker,
        object::{Object, Version},
        time::Time,
        tree::{LatchListener, OutputNode, PresentationListener},
        utils::{cell_ext::CellExt, clonecell::CloneCell, event_listener::EventListener},
        video::Modifier,
        wire::{ext_image_copy_capture_session_v1::*, ExtImageCopyCaptureSessionV1Id},
    },
    std::{
        cell::Cell,
        rc::{Rc, Weak},
    },
    thiserror::Error,
    uapi::c,
};

pub struct ExtImageCopyCaptureSessionV1 {
    pub(super) id: ExtImageCopyCaptureSessionV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub(super) frame: CloneCell<Option<Rc<ExtImageCopyCaptureFrameV1>>>,
    pub(super) shm_bridge: CloneCell<Option<Rc<dyn GfxInternalFramebuffer>>>,
    pub(super) shm_staging: CloneCell<Option<Rc<dyn GfxStagingBuffer>>>,
    pub(super) source: ImageCaptureSource,
    pub(super) force_capture: Cell<bool>,
    pub(super) stopped: Cell<bool>,
    pub(super) latch_listener: EventListener<dyn LatchListener>,
    pub(super) presentation_listener: EventListener<dyn PresentationListener>,
    pub(super) size_debounce: Cell<bool>,
    pub(super) status: Cell<FrameStatus>,
    pub(super) buffer: CloneCell<Option<Rc<WlBuffer>>>,
    pub(super) pending_download: Cell<Option<PendingShmTransfer>>,
    pub(super) presented: Cell<Option<(u64, u32)>>,
}

impl ExtImageCopyCaptureSessionV1 {
    pub(super) fn new(
        id: ExtImageCopyCaptureSessionV1Id,
        client: &Rc<Client>,
        version: Version,
        source: &ImageCaptureSource,
        slf: &Weak<Self>,
    ) -> Self {
        ExtImageCopyCaptureSessionV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            frame: Default::default(),
            shm_bridge: Default::default(),
            shm_staging: Default::default(),
            source: source.clone(),
            force_capture: Cell::new(true),
            stopped: Default::default(),
            latch_listener: EventListener::new(slf.clone()),
            presentation_listener: EventListener::new(slf.clone()),
            size_debounce: Default::default(),
            status: Cell::new(FrameStatus::Unused),
            buffer: Default::default(),
            pending_download: Default::default(),
            presented: Default::default(),
        }
    }

    pub fn buffer_size_changed(&self) {
        if self.size_debounce.replace(true) {
            return;
        }
        self.force_capture.set(true);
        self.send_current_buffer_size();
        self.send_done();
    }

    pub(super) fn send_current_buffer_size(&self) {
        let (width, height) = match &self.source {
            ImageCaptureSource::Output(o) => {
                let Some(node) = o.node() else {
                    return;
                };
                node.global.pixel_size()
            }
            ImageCaptureSource::Toplevel(o) => {
                let Some(node) = o.get() else {
                    return;
                };
                node.tl_data().desired_pixel_size()
            }
        };
        self.send_buffer_size(width, height);
    }

    pub(super) fn send_buffer_size(&self, width: i32, height: i32) {
        self.client.event(BufferSize {
            self_id: self.id,
            width: width as _,
            height: height as _,
        });
    }

    pub(super) fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn stop(&self) {
        self.stopped.set(true);
        self.send_stopped();
        self.stop_pending_frame();
    }

    fn stop_pending_frame(&self) {
        if let Some(frame) = self.frame.get() {
            if let FrameStatus::Capturing | FrameStatus::Captured = self.status.get() {
                frame.fail(FrameFailureReason::Stopped);
            }
        }
    }

    pub(super) fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }

    pub(super) fn send_shm_formats(&self) {
        for format in FORMATS {
            if format.shm_info.is_some() {
                self.client.event(ShmFormat {
                    self_id: self.id,
                    format: format.wl_id.unwrap_or(format.drm),
                });
            }
        }
    }

    pub(super) fn send_dmabuf_device(&self, device: c::dev_t) {
        self.client.event(DmabufDevice {
            self_id: self.id,
            device,
        });
    }

    pub(super) fn send_dmabuf_format(&self, format: &Format, modifiers: &[Modifier]) {
        self.client.event(DmabufFormat {
            self_id: self.id,
            format: format.drm,
            modifiers: uapi::as_bytes(modifiers),
        });
    }

    fn detach(&self) {
        let id = (self.client.id, self.id);
        match &self.source {
            ImageCaptureSource::Output(o) => {
                if let Some(n) = o.node() {
                    n.ext_copy_sessions.remove(&id);
                }
            }
            ImageCaptureSource::Toplevel(tl) => {
                if let Some(n) = tl.get() {
                    n.tl_data().ext_copy_sessions.remove(&id);
                }
            }
        }
        self.frame.take();
        self.shm_bridge.take();
        self.shm_staging.take();
        self.latch_listener.detach();
        self.presentation_listener.detach();
        self.buffer.take();
        self.pending_download.take();
        self.presented.take();
    }

    pub fn update_latch_listener(&self) {
        let ImageCaptureSource::Toplevel(tl) = &self.source else {
            return;
        };
        let Some(tl) = tl.get() else {
            return;
        };
        let data = tl.tl_data();
        if data.visible.get() {
            self.latch_listener.attach(&data.output().latch_event);
        } else {
            self.latch_listener.detach();
        }
        if self.status.get() == FrameStatus::Captured && self.presented.is_none() {
            self.presentation_listener.detach();
            let now = Time::now_unchecked();
            self.presented
                .set(Some((now.0.tv_sec as _, now.0.tv_nsec as _)));
            if let Some(frame) = self.frame.get() {
                frame.maybe_ready();
            }
        }
    }

    pub fn copy_texture(
        self: &Rc<Self>,
        on: &OutputNode,
        texture: &Rc<dyn GfxTexture>,
        resv: Option<&Rc<dyn BufferResv>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
        if self.status.get() == FrameStatus::Capturing {
            if let Some(frame) = self.frame.get() {
                frame.copy_texture(
                    on,
                    texture,
                    resv,
                    acquire_sync,
                    release_sync,
                    render_hardware_cursors,
                    x_off,
                    y_off,
                    size,
                );
                return;
            }
        }
        self.force_capture.set(true);
    }
}

impl ExtImageCopyCaptureSessionV1RequestHandler for ExtImageCopyCaptureSessionV1 {
    type Error = ExtImageCopyCaptureSessionV1Error;

    fn create_frame(&self, req: CreateFrame, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.frame.is_some() {
            return Err(ExtImageCopyCaptureSessionV1Error::HaveFrame);
        }
        let obj = Rc::new(ExtImageCopyCaptureFrameV1 {
            id: req.frame,
            client: self.client.clone(),
            tracker: Default::default(),
            session: slf.clone(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        self.frame.set(Some(obj));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.stop_pending_frame();
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl LatchListener for ExtImageCopyCaptureSessionV1 {
    fn after_latch(self: Rc<Self>, on: &OutputNode, _tearing: bool) {
        let ImageCaptureSource::Toplevel(tl) = &self.source else {
            return;
        };
        let Some(tl) = tl.get() else {
            return;
        };
        let data = tl.tl_data();
        if !data.visible.get() {
            return;
        }
        let Some(frame) = self.frame.get() else {
            self.force_capture.set(true);
            return;
        };
        if self.status.get() != FrameStatus::Capturing {
            self.force_capture.set(true);
            return;
        }
        frame.copy_node(on, tl.tl_as_node(), data.desired_pixel_size());
    }
}

impl PresentationListener for ExtImageCopyCaptureSessionV1 {
    fn presented(
        self: Rc<Self>,
        _output: &OutputNode,
        tv_sec: u64,
        tv_nsec: u32,
        _refresh: u32,
        _seq: u64,
        _flags: u32,
        _vrr: bool,
    ) {
        self.presentation_listener.detach();
        let Some(frame) = self.frame.get() else {
            return;
        };
        if self.status.get() != FrameStatus::Captured {
            return;
        };
        self.presented.set(Some((tv_sec, tv_nsec)));
        frame.maybe_ready();
    }
}

object_base! {
    self = ExtImageCopyCaptureSessionV1;
    version = self.version;
}

impl Object for ExtImageCopyCaptureSessionV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(
    ExtImageCopyCaptureSessionV1,
    ExtImageCopyCaptureSessionV1Id,
    ext_copy_sessions
);

#[derive(Debug, Error)]
pub enum ExtImageCopyCaptureSessionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("There already is a pending frame")]
    HaveFrame,
}
efrom!(ExtImageCopyCaptureSessionV1Error, ClientError);
