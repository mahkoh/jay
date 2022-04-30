use std::rc::Rc;
use thiserror::Error;
use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_seat::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::wire::ZwpRelativePointerManagerV1Id;
use crate::wire::zwp_relative_pointer_manager_v1::*;

pub struct ZwpRelativePointerManagerV1Global {
    pub name: GlobalName,
}

pub struct ZwpRelativePointerManagerV1 {
    pub id: ZwpRelativePointerManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl ZwpRelativePointerManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpRelativePointerManagerV1Id,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), ZwpRelativePointerManagerV1Error> {
        let obj = Rc::new(ZwpRelativePointerManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(ZwpRelativePointerManagerV1Global, ZwpRelativePointerManagerV1, ZwpRelativePointerManagerV1Error);

impl Global for ZwpRelativePointerManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpRelativePointerManagerV1Global);

impl ZwpRelativePointerManagerV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpRelativePointerManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_relative_pointer(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpRelativePointerManagerV1Error> {
        let req: GetRelativePointer = self.client.parse(self, parser)?;
        let pointer = self.client.lookup(req.pointer)?;
        let rp = Rc::new(ZwpRelativePointerV1 {
            id: req.id,
            client: self.client.clone(),
            seat: pointer.seat.clone(),
            tracker: Default::default(),
        });
        track!(self.client, rp);
        self.client.add_client_obj(&rp)?;
        pointer.seat.relative_pointers.set(req.id, rp);
        Ok(())
    }
}

object_base! {
    ZwpRelativePointerManagerV1;

    DESTROY => destroy,
    GET_RELATIVE_POINTER => get_relative_pointer,
}

impl Object for ZwpRelativePointerManagerV1 {
    fn num_requests(&self) -> u32 {
        GET_RELATIVE_POINTER + 1
    }
}

simple_add_obj!(ZwpRelativePointerManagerV1);

#[derive(Debug, Error)]
pub enum ZwpRelativePointerManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpRelativePointerManagerV1Error, MsgParserError);
efrom!(ZwpRelativePointerManagerV1Error, ClientError);
