use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wp_fifo_v1::*, WpFifoV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpFifoV1 {
    pub id: WpFifoV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
}

impl WpFifoV1 {
    pub fn new(id: WpFifoV1Id, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpFifoV1Error> {
        if self.surface.fifo.is_some() {
            return Err(WpFifoV1Error::Exists);
        }
        self.surface.fifo.set(Some(self.clone()));
        Ok(())
    }

    fn fifo(&self, msg: MsgParser<'_, '_>) -> Result<(), WpFifoV1Error> {
        let _req: Fifo = self.client.parse(self, msg)?;
        self.surface.pending.borrow_mut().fifo = true;
        Ok(())
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpFifoV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.surface.fifo.take();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpFifoV1;

    FIFO => fifo,
    DESTROY => destroy,
}

impl Object for WpFifoV1 {}

simple_add_obj!(WpFifoV1);

#[derive(Debug, Error)]
pub enum WpFifoV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a fifo extension attached")]
    Exists,
}
efrom!(WpFifoV1Error, MsgParserError);
efrom!(WpFifoV1Error, ClientError);
