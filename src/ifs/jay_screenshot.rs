use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufPlane;
use crate::wire::JayScreenshotId;
use crate::wire::jay_screenshot::*;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct JayScreenshot {
    pub id: JayScreenshotId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayScreenshot {
    pub fn send_dmabuf(
        &self,
        drm_dev: &Rc<OwnedFd>,
        fd: &Rc<OwnedFd>,
        width: i32,
        height: i32,
        offset: u32,
        stride: u32,
        modifier: u64,
    ) {
        self.client.event(Dmabuf {
            self_id: self.id,
            drm_dev: drm_dev.clone(),
            fd: fd.clone(),
            width: width as _,
            height: height as _,
            offset,
            stride,
            modifier,
        });
    }

    pub fn send_error(&self, msg: &str) {
        self.client.event(Error {
            self_id: self.id,
            msg,
        });
    }

    pub fn send_drm_dev(&self, drm: &Rc<OwnedFd>) {
        self.client.event(DrmDev {
            self_id: self.id,
            drm_dev: drm.clone(),
        })
    }

    pub fn send_plane(&self, plane: &DmaBufPlane) {
        self.client.event(Plane {
            self_id: self.id,
            fd: plane.fd.clone(),
            offset: plane.offset,
            stride: plane.stride,
        })
    }

    pub fn send_dmabuf2(&self, buf: &DmaBuf) {
        self.client.event(Dmabuf2 {
            self_id: self.id,
            width: buf.width,
            height: buf.height,
            modifier: buf.modifier,
        })
    }
}

impl JayScreenshotRequestHandler for JayScreenshot {
    type Error = Infallible;
}

object_base! {
    self = JayScreenshot;
    version = Version(1);
}

impl Object for JayScreenshot {}

simple_add_obj!(JayScreenshot);
