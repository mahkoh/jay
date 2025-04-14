use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_buffer::WlBuffer,
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpSinglePixelBufferManagerV1Id, wp_single_pixel_buffer_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpSinglePixelBufferManagerV1Global {
    name: GlobalName,
}

impl WpSinglePixelBufferManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpSinglePixelBufferManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpSinglePixelBufferManagerV1Error> {
        let obj = Rc::new(WpSinglePixelBufferManagerV1 {
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

global_base!(
    WpSinglePixelBufferManagerV1Global,
    WpSinglePixelBufferManagerV1,
    WpSinglePixelBufferManagerV1Error
);

impl Global for WpSinglePixelBufferManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpSinglePixelBufferManagerV1Global);

pub struct WpSinglePixelBufferManagerV1 {
    pub id: WpSinglePixelBufferManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpSinglePixelBufferManagerV1RequestHandler for WpSinglePixelBufferManagerV1 {
    type Error = WpSinglePixelBufferManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_u32_rgba_buffer(
        &self,
        req: CreateU32RgbaBuffer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let map = |c: u32| (c as f64 / u32::MAX as f64) as f32;
        let buffer = Rc::new(WlBuffer::new_single_pixel(
            req.id,
            &self.client,
            map(req.r),
            map(req.g),
            map(req.b),
            map(req.a),
        ));
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }

    fn create_f32_rgba_buffer(
        &self,
        req: CreateF32RgbaBuffer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let buffer = Rc::new(WlBuffer::new_single_pixel(
            req.id,
            &self.client,
            f32::from_bits(req.r),
            f32::from_bits(req.g),
            f32::from_bits(req.b),
            f32::from_bits(req.a),
        ));
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }
}

object_base! {
    self = WpSinglePixelBufferManagerV1;
    version = self.version;
}

impl Object for WpSinglePixelBufferManagerV1 {}

simple_add_obj!(WpSinglePixelBufferManagerV1);

#[derive(Debug, Error)]
pub enum WpSinglePixelBufferManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpSinglePixelBufferManagerV1Error, ClientError);
