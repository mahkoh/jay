use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_relative_pointer_v1::*, ZwpRelativePointerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpRelativePointerV1 {
    pub id: ZwpRelativePointerV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeat>,
    pub tracker: Tracker<Self>,
}

impl ZwpRelativePointerV1 {
    pub fn send_relative_motion(
        &self,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        dx_unaccelerated: Fixed,
        dy_unaccelerated: Fixed,
    ) {
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpRelativePointerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.seat.relative_pointers.remove(&self.id);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpRelativePointerV1;

    DESTROY => destroy,
}

impl Object for ZwpRelativePointerV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}

simple_add_obj!(ZwpRelativePointerV1);

#[derive(Debug, Error)]
pub enum ZwpRelativePointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
}
efrom!(ZwpRelativePointerV1Error, ClientError);
efrom!(ZwpRelativePointerV1Error, MsgParserError);
