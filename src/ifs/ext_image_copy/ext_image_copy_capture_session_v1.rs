use crate::client::Client;
use crate::client::ClientError;
use crate::cmm::cmm_description::ColorDescription;
use crate::format::FORMATS;
use crate::format::Format;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::BufferResv;
use crate::gfx_api::GfxInternalFramebuffer;
use crate::gfx_api::GfxStagingBuffer;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::LazyTexture;
use crate::gfx_api::PendingShmTransfer;
use crate::gfx_api::ReleaseSync;
use crate::ifs::ext_image_capture_source_v1::ImageCaptureSource;
use crate::ifs::ext_image_copy::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;
use crate::ifs::ext_image_copy::ext_image_copy_capture_frame_v1::FrameFailureReason;
use crate::ifs::ext_image_copy::ext_image_copy_capture_frame_v1::FrameStatus;
use crate::ifs::wl_buffer::WlBuffer;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::time::Time;
use crate::tree::LatchListener;
use crate::tree::OutputNode;
use crate::tree::PresentationListener;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::cell_ext::CellExt;
use crate::utils::clonecell::CloneCell;
use crate::utils::event_listener::EventListener;
use crate::video::Modifier;
use crate::wire::ExtImageCopyCaptureSessionV1Id;
use crate::wire::ext_image_copy_capture_session_v1::*;
use std::cell::Cell;
use std::rc::Rc;
use std::rc::Weak;
use thiserror::Error;
use uapi::c;

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
                node.pixel_size()
            }
            ImageCaptureSource::Toplevel(o) => {
                let Some(node) = o.get() else {
                    return;
                };
                node.tl_data().desired_pixel_size(LiveTL)
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
        if let Some(frame) = self.frame.get()
            && let FrameStatus::Capturing | FrameStatus::Captured = self.status.get()
        {
            frame.fail(FrameFailureReason::Stopped);
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
        if data.visible[LiveTL].get() {
            self.latch_listener.attach(&data.output(LiveTL).latch_event);
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
        cd: &Rc<ColorDescription>,
        resv: Option<&Rc<dyn BufferResv>>,
        lazy: Option<&Rc<dyn LazyTexture>>,
        acquire_sync: &AcquireSync,
        release_sync: ReleaseSync,
        render_hardware_cursors: bool,
        x_off: i32,
        y_off: i32,
        size: Option<(i32, i32)>,
    ) {
        if self.status.get() == FrameStatus::Capturing
            && let Some(frame) = self.frame.get()
        {
            frame.copy_texture(
                on,
                texture,
                cd,
                resv,
                lazy,
                acquire_sync,
                release_sync,
                render_hardware_cursors,
                x_off,
                y_off,
                size,
            );
            return;
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
        if !data.visible[RenderTL].get() {
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
        frame.copy_node(on, &*tl, data.desired_pixel_size(RenderTL));
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
    fn break_loops(self: Rc<Self>) {
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
