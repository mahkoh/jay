use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpRelativePointerV1Id, zwp_relative_pointer_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpRelativePointerV1 {
    pub id: ZwpRelativePointerV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeat>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpRelativePointerV1 {
    pub fn send_relative_motion(
        &self,
        time_usec: u64,
        mut dx: Fixed,
        mut dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
        logical_to_client_wire_scale!(self.client, dx, dy);
        self.client.event(RelativeMotion {
            self_id: self.id,
            utime_hi: (time_usec >> 32) as u32,
            utime_lo: time_usec as u32,
            dx,
            dy,
            dx_unaccelerated,
            dy_unaccelerated,
        });
    }
}

impl ZwpRelativePointerV1RequestHandler for ZwpRelativePointerV1 {
    type Error = ZwpRelativePointerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.seat.relative_pointers.remove(&self.id);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpRelativePointerV1;
    version = self.version;
}

impl Object for ZwpRelativePointerV1 {}

simple_add_obj!(ZwpRelativePointerV1);

#[derive(Debug, Error)]
pub enum ZwpRelativePointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpRelativePointerV1Error, ClientError);
