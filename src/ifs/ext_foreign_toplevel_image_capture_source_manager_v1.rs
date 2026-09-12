use crate::client::Client;
use crate::client::ClientError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use crate::ifs::ext_image_capture_source_v1::ImageCaptureSource;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::ExtForeignToplevelImageCaptureSourceManagerV1Id;
use crate::wire::ext_foreign_toplevel_image_capture_source_manager_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct ExtForeignToplevelImageCaptureSourceManagerV1Global {
    name: GlobalName,
}

impl ExtForeignToplevelImageCaptureSourceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtForeignToplevelImageCaptureSourceManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtForeignToplevelImageCaptureSourceManagerV1Error> {
        let obj = Rc::new(ExtForeignToplevelImageCaptureSourceManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

pub struct ExtForeignToplevelImageCaptureSourceManagerV1 {
    id: ExtForeignToplevelImageCaptureSourceManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl ExtForeignToplevelImageCaptureSourceManagerV1RequestHandler
    for ExtForeignToplevelImageCaptureSourceManagerV1
{
    type Error = ExtForeignToplevelImageCaptureSourceManagerV1Error;

    fn create_source(&self, req: CreateSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let handle = self.client.lookup(req.toplevel_handle)?;
        let obj = Rc::new(ExtImageCaptureSourceV1 {
            id: req.source,
            version: self.version,
            client: self.client.clone(),
            tracker: Default::default(),
            ty: ImageCaptureSource::Toplevel(handle.toplevel.clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

global_base!(
    ExtForeignToplevelImageCaptureSourceManagerV1Global,
    ExtForeignToplevelImageCaptureSourceManagerV1,
);

impl Global for ExtForeignToplevelImageCaptureSourceManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ExtForeignToplevelImageCaptureSourceManagerV1Global);

object_base!(ExtForeignToplevelImageCaptureSourceManagerV1);

impl Object for ExtForeignToplevelImageCaptureSourceManagerV1 {}

simple_add_obj!(ExtForeignToplevelImageCaptureSourceManagerV1);

#[derive(Debug, Error)]
pub enum ExtForeignToplevelImageCaptureSourceManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(
    ExtForeignToplevelImageCaptureSourceManagerV1Error,
    ClientError
);
