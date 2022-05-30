use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_fractional_scale_v1::{WpFractionalScaleError, WpFractionalScaleV1},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_fractional_scale_manager_v1::*, WpFractionalScaleManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFractionalScaleManagerV1Global {
    pub name: GlobalName,
}

pub struct WpFractionalScaleManagerV1 {
    pub id: WpFractionalScaleManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WpFractionalScaleManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpFractionalScaleManagerV1Id,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WpFractionalScaleManagerError> {
        let obj = Rc::new(WpFractionalScaleManagerV1 {
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
    WpFractionalScaleManagerV1Global,
    WpFractionalScaleManagerV1,
    WpFractionalScaleManagerError
);

impl Global for WpFractionalScaleManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpFractionalScaleManagerV1Global);

impl WpFractionalScaleManagerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpFractionalScaleManagerError> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_fractional_scale(
        &self,
        msg: MsgParser<'_, '_>,
    ) -> Result<(), WpFractionalScaleManagerError> {
        let req: GetFractionalScale = self.client.parse(self, msg)?;
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(WpFractionalScaleV1::new(req.id, &surface));
        track!(self.client, fs);
        fs.install()?;
        self.client.add_client_obj(&fs)?;
        fs.send_preferred_scale();
        Ok(())
    }
}

object_base! {
    WpFractionalScaleManagerV1;

    DESTROY => destroy,
    GET_FRACTIONAL_SCALE => get_fractional_scale,
}

impl Object for WpFractionalScaleManagerV1 {
    fn num_requests(&self) -> u32 {
        GET_FRACTIONAL_SCALE + 1
    }
}

simple_add_obj!(WpFractionalScaleManagerV1);

#[derive(Debug, Error)]
pub enum WpFractionalScaleManagerError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpFractionalScaleError(#[from] WpFractionalScaleError),
}
efrom!(WpFractionalScaleManagerError, MsgParserError);
efrom!(WpFractionalScaleManagerError, ClientError);
