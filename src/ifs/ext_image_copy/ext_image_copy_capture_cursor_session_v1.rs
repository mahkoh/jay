use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ext_image_capture_source_v1::ImageCaptureSource,
            ext_image_copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ExtImageCopyCaptureCursorSessionV1Id, ext_image_copy_capture_cursor_session_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ExtImageCopyCaptureCursorSessionV1 {
    pub(super) id: ExtImageCopyCaptureCursorSessionV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub(super) have_session: Cell<bool>,
    pub(super) source: ImageCaptureSource,
}

impl ExtImageCopyCaptureCursorSessionV1RequestHandler for ExtImageCopyCaptureCursorSessionV1 {
    type Error = ExtImageCopyCaptureCursorSessionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_capture_session(
        &self,
        req: GetCaptureSession,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.have_session.replace(true) {
            return Err(ExtImageCopyCaptureCursorSessionV1Error::HaveSession);
        }
        let obj = Rc::new_cyclic(|slf| {
            ExtImageCopyCaptureSessionV1::new(
                req.session,
                &self.client,
                self.version,
                &self.source,
                slf,
            )
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_shm_formats();
        obj.send_buffer_size(1, 1);
        obj.send_done();
        Ok(())
    }
}

object_base! {
    self = ExtImageCopyCaptureCursorSessionV1;
    version = self.version;
}

impl Object for ExtImageCopyCaptureCursorSessionV1 {}

simple_add_obj!(ExtImageCopyCaptureCursorSessionV1);

#[derive(Debug, Error)]
pub enum ExtImageCopyCaptureCursorSessionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The session has already been created")]
    HaveSession,
}
efrom!(ExtImageCopyCaptureCursorSessionV1Error, ClientError);
