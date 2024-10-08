use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ext_image_capture_source_v1::{ExtImageCaptureSourceV1, ImageCaptureSource},
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            ext_output_image_capture_source_manager_v1::*, ExtOutputImageCaptureSourceManagerV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtOutputImageCaptureSourceManagerV1Global {
    pub name: GlobalName,
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
    ) -> Result<(), ExtOutputImageCaptureSourceManagerV1Error> {
        let obj = Rc::new(ExtOutputImageCaptureSourceManagerV1 {
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

pub struct ExtOutputImageCaptureSourceManagerV1 {
    pub id: ExtOutputImageCaptureSourceManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ExtOutputImageCaptureSourceManagerV1RequestHandler for ExtOutputImageCaptureSourceManagerV1 {
    type Error = ExtOutputImageCaptureSourceManagerV1Error;

    fn create_source(&self, req: CreateSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?;
        let obj = Rc::new(ExtImageCaptureSourceV1 {
            id: req.source,
            client: self.client.clone(),
            tracker: Default::default(),
            ty: ImageCaptureSource::Output(output.global.clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ExtOutputImageCaptureSourceManagerV1Global,
    ExtOutputImageCaptureSourceManagerV1,
    ExtOutputImageCaptureSourceManagerV1Error
);

impl Global for ExtOutputImageCaptureSourceManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ExtOutputImageCaptureSourceManagerV1Global);

object_base! {
    self = ExtOutputImageCaptureSourceManagerV1;
    version = self.version;
}

impl Object for ExtOutputImageCaptureSourceManagerV1 {}

simple_add_obj!(ExtOutputImageCaptureSourceManagerV1);

#[derive(Debug, Error)]
pub enum ExtOutputImageCaptureSourceManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtOutputImageCaptureSourceManagerV1Error, ClientError);
