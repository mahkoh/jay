use crate::client::Client;
use crate::gfx_api::PendingShmTransfer;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::screenshoter::ScreenshooterError;
use crate::screenshoter::ScreenshotCallback;
use crate::utils::errorfmt::ErrorFmt;
use crate::video::dmabuf::DmaBuf;
use crate::wire::JayScreenshotId;
use crate::wire::jay_screenshot::*;
use jay_proc::Object;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JayScreenshot {
    pub id: JayScreenshotId,
    pub version: Version,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub pending: Cell<Option<PendingShmTransfer>>,
}

impl JayScreenshot {
    pub fn send_error(&self, msg: &str) {
        self.client.event(Error {
            self_id: self.id,
            msg,
        });
    }

    pub fn send_udmabuf(&self, buf: &DmaBuf) {
        let plane = &buf.planes[0];
        self.client.event(Udmabuf {
            self_id: self.id,
            width: buf.width,
            height: buf.height,
            format: buf.format.drm,
            fd: plane.fd.clone(),
            stride: plane.stride,
        });
    }
}

impl ScreenshotCallback for JayScreenshot {
    fn completed(self: Rc<Self>, res: Result<Rc<DmaBuf>, ScreenshooterError>) {
        self.pending.take();
        match res {
            Ok(dmabuf) => {
                self.send_udmabuf(&dmabuf);
            }
            Err(e) => {
                let msg = ErrorFmt(e).to_string();
                self.send_error(&msg);
            }
        }
        self.client.remove_obj(self.deref());
    }
}

impl BreakLoops for JayScreenshot {
    fn break_loops(self: Rc<Self>) {
        self.pending.take();
    }
}

impl JayScreenshotRequestHandler for JayScreenshot {
    type Error = !;
}
