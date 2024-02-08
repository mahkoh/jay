use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_content_type_v1::WpContentTypeV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_content_type_manager_v1::*, WpContentTypeManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpContentTypeManagerV1Global {
    pub name: GlobalName,
}

impl WpContentTypeManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpContentTypeManagerV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WpContentTypeManagerV1Error> {
        let mgr = Rc::new(WpContentTypeManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        Ok(())
    }
}

global_base!(
    WpContentTypeManagerV1Global,
    WpContentTypeManagerV1,
    WpContentTypeManagerV1Error
);

simple_add_global!(WpContentTypeManagerV1Global);

impl Global for WpContentTypeManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct WpContentTypeManagerV1 {
    pub id: WpContentTypeManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: u32,
}

impl WpContentTypeManagerV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpContentTypeManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_surface_content_type(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpContentTypeManagerV1Error> {
        let req: GetSurfaceContentType = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        if surface.has_content_type_manager.replace(true) {
            return Err(WpContentTypeManagerV1Error::DuplicateContentType);
        }
        let device = Rc::new(WpContentTypeV1 {
            id: req.id,
            client: self.client.clone(),
            surface,
            tracker: Default::default(),
        });
        track!(self.client, device);
        self.client.add_client_obj(&device)?;
        Ok(())
    }
}

object_base! {
    self = WpContentTypeManagerV1;

    DESTROY => destroy,
    GET_SURFACE_CONTENT_TYPE => get_surface_content_type,
}

impl Object for WpContentTypeManagerV1 {}

simple_add_obj!(WpContentTypeManagerV1);

#[derive(Debug, Error)]
pub enum WpContentTypeManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Surface already has a content type object")]
    DuplicateContentType,
}
efrom!(WpContentTypeManagerV1Error, ClientError);
efrom!(WpContentTypeManagerV1Error, MsgParserError);
