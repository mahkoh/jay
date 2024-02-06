use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_buffer::WlBuffer,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_single_pixel_buffer_manager_v1::*, WpSinglePixelBufferManagerV1Id},
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
        _version: u32,
    ) -> Result<(), WpSinglePixelBufferManagerV1Error> {
        let obj = Rc::new(WpSinglePixelBufferManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
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
}

impl WpSinglePixelBufferManagerV1 {
    pub fn destroy(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpSinglePixelBufferManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn create_u32_rgba_buffer(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpSinglePixelBufferManagerV1Error> {
        let req: CreateU32RgbaBuffer = self.client.parse(self, parser)?;
        let buffer = Rc::new(WlBuffer::new_single_pixel(
            req.id,
            &self.client,
            req.r,
            req.g,
            req.b,
            req.a,
        ));
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }
}

object_base! {
    self = WpSinglePixelBufferManagerV1;

    DESTROY => destroy,
    CREATE_U32_RGBA_BUFFER => create_u32_rgba_buffer,
}

impl Object for WpSinglePixelBufferManagerV1 {}

simple_add_obj!(WpSinglePixelBufferManagerV1);

#[derive(Debug, Error)]
pub enum WpSinglePixelBufferManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpSinglePixelBufferManagerV1Error, ClientError);
efrom!(WpSinglePixelBufferManagerV1Error, MsgParserError);
