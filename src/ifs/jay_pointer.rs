use {
    crate::{
        client::{Client, ClientError},
        cursor::KnownCursor,
        ifs::wl_seat::WlSeatGlobal,
        leaks::Tracker,
        object::{Object, Version},
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

impl JayPointerRequestHandler for JayPointer {
    type Error = JayPointerError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_known_cursor(&self, req: SetKnownCursor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
    self = JayPointer;
    version = Version(1);
}

impl Object for JayPointer {}

simple_add_obj!(JayPointer);

#[derive(Debug, Error)]
pub enum JayPointerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Cursor index is out of bounds")]
    OutOfBounds,
}
efrom!(JayPointerError, ClientError);
