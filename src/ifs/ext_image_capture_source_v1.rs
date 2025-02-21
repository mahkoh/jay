use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_output::OutputGlobalOpt,
        leaks::Tracker,
        object::{Object, Version},
        tree::ToplevelOpt,
        wire::{ExtImageCaptureSourceV1Id, ext_image_capture_source_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[derive(Clone)]
pub enum ImageCaptureSource {
    Output(Rc<OutputGlobalOpt>),
    Toplevel(ToplevelOpt),
}

pub struct ExtImageCaptureSourceV1 {
    pub id: ExtImageCaptureSourceV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub ty: ImageCaptureSource,
}

impl ExtImageCaptureSourceV1RequestHandler for ExtImageCaptureSourceV1 {
    type Error = ExtImageCaptureSourceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ExtImageCaptureSourceV1;
    version = Version(1);
}

impl Object for ExtImageCaptureSourceV1 {}

dedicated_add_obj!(
    ExtImageCaptureSourceV1,
    ExtImageCaptureSourceV1Id,
    image_capture_sources
);

#[derive(Debug, Error)]
pub enum ExtImageCaptureSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtImageCaptureSourceError, ClientError);
