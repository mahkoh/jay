use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_viewport::{WpViewport, WpViewportError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_viewporter::*, WpViewporterId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpViewporterGlobal {
    pub name: GlobalName,
}

impl WpViewporterGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpViewporterId,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WpViewporterError> {
        let obj = Rc::new(WpViewporter {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(WpViewporterGlobal, WpViewporter, WpViewporterError);

impl Global for WpViewporterGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpViewporterGlobal);

pub struct WpViewporter {
    pub id: WpViewporterId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WpViewporter {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpViewporterError> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_viewport(&self, msg: MsgParser<'_, '_>) -> Result<(), WpViewporterError> {
        let req: GetViewport = self.client.parse(self, msg)?;
        let surface = self.client.lookup(req.surface)?;
        let viewport = Rc::new(WpViewport::new(req.id, &surface));
        track!(self.client, viewport);
        viewport.install()?;
        self.client.add_client_obj(&viewport)?;
        Ok(())
    }
}

object_base! {
    self = WpViewporter;

    DESTROY => destroy,
    GET_VIEWPORT => get_viewport,
}

impl Object for WpViewporter {}

simple_add_obj!(WpViewporter);

#[derive(Debug, Error)]
pub enum WpViewporterError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpViewportError(#[from] WpViewportError),
}
efrom!(WpViewporterError, MsgParserError);
efrom!(WpViewporterError, ClientError);
