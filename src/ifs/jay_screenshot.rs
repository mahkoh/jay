use {
    crate::{
        client::Client,
        leaks::Tracker,
        object::Object,
        wire::{jay_screenshot::*, JayScreenshotId},
    },
    std::rc::Rc,
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
    ) {
        self.client.event(Dmabuf {
            self_id: self.id,
            drm_dev: drm_dev.clone(),
            fd: fd.clone(),
            width: width as _,
            height: height as _,
            offset,
            stride,
        });
    }

    pub fn send_error(&self, msg: &str) {
        self.client.event(Error {
            self_id: self.id,
            msg,
        });
    }
}

object_base! {
    self = JayScreenshot;
}

impl Object for JayScreenshot {}

simple_add_obj!(JayScreenshot);
