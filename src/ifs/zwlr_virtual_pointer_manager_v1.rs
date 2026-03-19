use {
    crate::{
        client::{CAP_VIRTUAL_POINTER_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WlOutputId, WlSeatId, ZwlrVirtualPointerManagerV1Id, ZwlrVirtualPointerV1Id,
            zwlr_virtual_pointer_manager_v1::*,
        },
    },
    std::{ops::Deref, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrVirtualPointerManagerV1Global {
    name: GlobalName,
}

impl ZwlrVirtualPointerManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrVirtualPointerManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwlrVirtualPointerManagerV1Error> {
        let obj = Rc::new(ZwlrVirtualPointerManagerV1 {
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
    ZwlrVirtualPointerManagerV1Global,
    ZwlrVirtualPointerManagerV1,
    ZwlrVirtualPointerManagerV1Error
);

simple_add_global!(ZwlrVirtualPointerManagerV1Global);

impl Global for ZwlrVirtualPointerManagerV1Global {
    fn version(&self) -> u32 {
        2
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_VIRTUAL_POINTER_MANAGER
    }
}

pub struct ZwlrVirtualPointerManagerV1 {
    pub id: ZwlrVirtualPointerManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwlrVirtualPointerManagerV1 {
    fn create_virtual_pointer(
        &self,
        id: ZwlrVirtualPointerV1Id,
        seat: WlSeatId,
        output: WlOutputId,
    ) -> Result<(), ZwlrVirtualPointerManagerV1Error> {
        let seat = if seat.is_some() {
            self.client.lookup(seat)?.global.clone()
        } else {
            match self.client.state.seat_queue.last() {
                None => return Err(ZwlrVirtualPointerManagerV1Error::NoSeat),
                Some(s) => s.deref().clone(),
            }
        };
        let output = if output.is_none() {
            None
        } else {
            Some(self.client.lookup(output)?.global.clone())
        };
        let obj = Rc::new(ZwlrVirtualPointerV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            events: Default::default(),
            seat,
            output,
            buttons: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl ZwlrVirtualPointerManagerV1RequestHandler for ZwlrVirtualPointerManagerV1 {
    type Error = ZwlrVirtualPointerManagerV1Error;

    fn create_virtual_pointer(
        &self,
        req: CreateVirtualPointer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.create_virtual_pointer(req.id, req.seat, WlOutputId::NONE)
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_virtual_pointer_with_output(
        &self,
        req: CreateVirtualPointerWithOutput,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.create_virtual_pointer(req.id, req.seat, req.output)
    }
}

object_base! {
    self = ZwlrVirtualPointerManagerV1;
    version = self.version;
}

impl Object for ZwlrVirtualPointerManagerV1 {}

simple_add_obj!(ZwlrVirtualPointerManagerV1);

#[derive(Debug, Error)]
pub enum ZwlrVirtualPointerManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("There are no seats")]
    NoSeat,
}
efrom!(ZwlrVirtualPointerManagerV1Error, ClientError);
