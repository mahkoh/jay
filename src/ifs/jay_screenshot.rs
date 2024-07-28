use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        video::{
            dmabuf::{DmaBuf, DmaBufPlane},
            gbm::GbmBo,
        },
        wire::{jay_screenshot::*, JayScreenshotId},
    },
    std::{cell::Cell, rc::Rc},
    uapi::OwnedFd,
};

pub const PLANES_SINCE: Version = Version(6);

pub struct JayScreenshot {
    pub id: JayScreenshotId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub bo: Cell<Option<GbmBo>>,
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
            modifier_lo: modifier as u32,
            modifier_hi: (modifier >> 32) as u32,
        });
    }

    pub fn send_error(&self, msg: &str) {
        self.client.event(Error {
            self_id: self.id,
            msg,
        });
    }

    pub fn send_plane(&self, plane: &DmaBufPlane) {
        self.client.event(Plane {
            self_id: self.id,
            fd: plane.fd.clone(),
            offset: plane.offset,
            stride: plane.stride,
        });
    }

    pub fn send_format(&self, drm_dev: &Rc<OwnedFd>, dmabuf: &DmaBuf) {
        self.client.event(Format {
            self_id: self.id,
            drm_dev: drm_dev.clone(),
            format: dmabuf.format.drm,
            width: dmabuf.width,
            height: dmabuf.height,
            modifier_lo: dmabuf.modifier as u32,
            modifier_hi: (dmabuf.modifier >> 32) as u32,
        });
    }
}

impl JayScreenshotRequestHandler for JayScreenshot {
    type Error = ClientError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayScreenshot;
    version = Version(1);
}

impl Object for JayScreenshot {}

simple_add_obj!(JayScreenshot);
