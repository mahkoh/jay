use crate::client::Client;
use crate::ifs::wl_output::OutputGlobalOpt;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::ToplevelOpt;
use crate::wire::ExtImageCaptureSourceV1Id;
use crate::wire::ext_image_capture_source_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Clone)]
pub enum ImageCaptureSource {
    Output(Rc<OutputGlobalOpt>),
    Toplevel(ToplevelOpt),
}

#[derive(Object)]
pub struct ExtImageCaptureSourceV1 {
    pub id: ExtImageCaptureSourceV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub ty: ImageCaptureSource,
}

impl ExtImageCaptureSourceV1RequestHandler for ExtImageCaptureSourceV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
