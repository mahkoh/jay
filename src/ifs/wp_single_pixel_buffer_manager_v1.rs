use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_buffer::WlBuffer;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::WpSinglePixelBufferManagerV1Id;
use crate::wire::wp_single_pixel_buffer_manager_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

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
    ) -> Result<(), Infallible> {
        let obj = Rc::new(WpSinglePixelBufferManagerV1 {
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

global_base!(
    WpSinglePixelBufferManagerV1Global,
    WpSinglePixelBufferManagerV1,
);

impl Global for WpSinglePixelBufferManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpSinglePixelBufferManagerV1Global);

#[derive(Object)]
pub struct WpSinglePixelBufferManagerV1 {
    id: WpSinglePixelBufferManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WpSinglePixelBufferManagerV1RequestHandler for WpSinglePixelBufferManagerV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_u32_rgba_buffer(
        &self,
        req: CreateU32RgbaBuffer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let buffer = WlBuffer::new_single_pixel(req.id, &self.client, req.r, req.g, req.b, req.a);
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer);
        Ok(())
    }
}
