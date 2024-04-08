use {
    crate::{
        client::Client,
        leaks::Tracker,
        object::{Object, Version},
        wire::{jay_screenshot::*, JayScreenshotId},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

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
