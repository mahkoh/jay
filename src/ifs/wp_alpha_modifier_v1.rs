use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_alpha_modifier_surface_v1::{
            WpAlphaModifierSurfaceV1, WpAlphaModifierSurfaceV1Error,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_alpha_modifier_v1::*, WpAlphaModifierV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpAlphaModifierV1Global {
    name: GlobalName,
}

pub struct WpAlphaModifierV1 {
    id: WpAlphaModifierV1Id,
    client: Rc<Client>,
    _version: u32,
    pub tracker: Tracker<Self>,
}

impl WpAlphaModifierV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpAlphaModifierV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WpAlphaModifierV1Error> {
        let obj = Rc::new(WpAlphaModifierV1 {
            id,
            client: client.clone(),
            _version: version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl WpAlphaModifierV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpAlphaModifierV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_surface(&self, parser: MsgParser<'_, '_>) -> Result<(), WpAlphaModifierV1Error> {
        let req: GetSurface = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let modifier = Rc::new(WpAlphaModifierSurfaceV1::new(req.id, &surface));
        track!(self.client, surface);
        self.client.add_client_obj(&modifier)?;
        modifier.install()?;
        Ok(())
    }
}

global_base!(
    WpAlphaModifierV1Global,
    WpAlphaModifierV1,
    WpAlphaModifierV1Error
);

impl Global for WpAlphaModifierV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpAlphaModifierV1Global);

object_base! {
    self = WpAlphaModifierV1;

    DESTROY => destroy,
    GET_SURFACE => get_surface,
}

impl Object for WpAlphaModifierV1 {}

simple_add_obj!(WpAlphaModifierV1);

#[derive(Debug, Error)]
pub enum WpAlphaModifierV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    WpAlphaModifierSurfaceV1Error(#[from] WpAlphaModifierSurfaceV1Error),
}

efrom!(WpAlphaModifierV1Error, ClientError);
efrom!(WpAlphaModifierV1Error, MsgParserError);
