use {
    crate::{
        client::{Client, ClientError},
        cursor::KnownCursor,
        ifs::wl_seat::WlSeatGlobal,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{jay_pointer::*, JayPointerId},
    },
    num_traits::FromPrimitive,
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayPointer {
    pub id: JayPointerId,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
}

impl JayPointer {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayPointerError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_known_cursor(&self, parser: MsgParser<'_, '_>) -> Result<(), JayPointerError> {
        let req: SetKnownCursor = self.client.parse(self, parser)?;
        let cursor = match KnownCursor::from_u32(req.idx) {
            Some(c) => c,
            _ => return Err(JayPointerError::OutOfBounds),
        };
        let pointer_node = match self.seat.pointer_node() {
            Some(n) => n,
            _ => {
                // cannot happen
                return Ok(());
            }
        };
        if pointer_node.node_client_id() != Some(self.client.id) {
            return Ok(());
        }
        self.seat.set_known_cursor(cursor);
        Ok(())
    }
}

object_base! {
    JayPointer;

    DESTROY => destroy,
    SET_KNOWN_CURSOR => set_known_cursor,
}

impl Object for JayPointer {
    fn num_requests(&self) -> u32 {
        SET_KNOWN_CURSOR + 1
    }
}

simple_add_obj!(JayPointer);

#[derive(Debug, Error)]
pub enum JayPointerError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Cursor index is out of bounds")]
    OutOfBounds,
}
efrom!(JayPointerError, MsgParserError);
efrom!(JayPointerError, ClientError);
