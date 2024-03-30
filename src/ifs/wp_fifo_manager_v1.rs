use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_fifo_v1::{WpFifoV1, WpFifoV1Error},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_fifo_manager_v1::*, WpFifoManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFifoManagerV1Global {
    pub name: GlobalName,
}

pub struct WpFifoManagerV1 {
    pub id: WpFifoManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WpFifoManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpFifoManagerV1Id,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), WpFifoManagerV1Error> {
        let obj = Rc::new(WpFifoManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(WpFifoManagerV1Global, WpFifoManagerV1, WpFifoManagerV1Error);

impl Global for WpFifoManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpFifoManagerV1Global);

impl WpFifoManagerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpFifoManagerV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_fifo(&self, msg: MsgParser<'_, '_>) -> Result<(), WpFifoManagerV1Error> {
        let req: GetFifo = self.client.parse(self, msg)?;
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(WpFifoV1::new(req.id, &surface));
        track!(self.client, fs);
        fs.install()?;
        self.client.add_client_obj(&fs)?;
        Ok(())
    }
}

object_base! {
    self = WpFifoManagerV1;

    DESTROY => destroy,
    GET_FIFO => get_fifo,
}

impl Object for WpFifoManagerV1 {}

simple_add_obj!(WpFifoManagerV1);

#[derive(Debug, Error)]
pub enum WpFifoManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpFifoV1Error(#[from] WpFifoV1Error),
}
efrom!(WpFifoManagerV1Error, MsgParserError);
efrom!(WpFifoManagerV1Error, ClientError);
