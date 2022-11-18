use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_tearing_control_v1::{WpTearingControlV1, WpTearingControlV1Error},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{
            wp_tearing_control_manager_v1::{GET_TEARING_CONTROL, *},
            WpTearingControlManagerV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpTearingControlManagerV1Global {
    name: GlobalName,
}

impl WpTearingControlManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpTearingControlManagerV1Id,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WpTearingControlManagerV1Error> {
        let obj = Rc::new(WpTearingControlManagerV1 {
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
    WpTearingControlManagerV1Global,
    WpTearingControlManagerV1,
    WpTearingControlManagerV1Error
);

impl Global for WpTearingControlManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpTearingControlManagerV1Global);

pub struct WpTearingControlManagerV1 {
    pub id: WpTearingControlManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WpTearingControlManagerV1 {
    pub fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpTearingControlManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn get_tearing_control(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpTearingControlManagerV1Error> {
        let req: GetTearingControl = self.client.parse(self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let control = Rc::new(WpTearingControlV1 {
            id: req.id,
            surface,
            tracker: Default::default(),
        });
        track!(self.client, control);
        self.client.add_client_obj(&control)?;
        control.install()?;
        Ok(())
    }
}

object_base! {
    WpTearingControlManagerV1;

    DESTROY => destroy,
    GET_TEARING_CONTROL => get_tearing_control,
}

impl Object for WpTearingControlManagerV1 {
    fn num_requests(&self) -> u32 {
        GET_TEARING_CONTROL + 1
    }
}

simple_add_obj!(WpTearingControlManagerV1);

#[derive(Debug, Error)]
pub enum WpTearingControlManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpTearingControlV1Error(#[from] WpTearingControlV1Error),
}
efrom!(WpTearingControlManagerV1Error, ClientError);
efrom!(WpTearingControlManagerV1Error, MsgParserError);
