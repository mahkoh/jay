use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use crate::ifs::ext_image_capture_source_v1::ImageCaptureSource;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ExtOutputImageCaptureSourceManagerV1Id;
use crate::wire::ext_output_image_capture_source_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct ExtOutputImageCaptureSourceManagerV1Global {
    name: GlobalName,
}

impl ExtOutputImageCaptureSourceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtOutputImageCaptureSourceManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ExtOutputImageCaptureSourceManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

#[derive(Object)]
pub struct ExtOutputImageCaptureSourceManagerV1 {
    id: ExtOutputImageCaptureSourceManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl ExtOutputImageCaptureSourceManagerV1RequestHandler for ExtOutputImageCaptureSourceManagerV1 {
    type Error = LookupError;

    fn create_source(&self, req: CreateSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?;
        let obj = Rc::new(ExtImageCaptureSourceV1 {
            id: req.source,
            version: self.version,
            client: self.client.clone(),
            tracker: Default::default(),
            ty: ImageCaptureSource::Output(output.global.clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

impl Global for ExtOutputImageCaptureSourceManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}
