use {
    crate::{
        client::ClientError,
        ifs::wl_seat::zwp_pointer_constraints_v1::{
            ConstraintOwner, SeatConstraint, ZwpPointerConstraintsV1Error,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_confined_pointer_v1::*, ZwpConfinedPointerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpConfinedPointerV1 {
    pub id: ZwpConfinedPointerV1Id,
    pub tracker: Tracker<Self>,
    pub constraint: Rc<SeatConstraint>,
}

impl ZwpConfinedPointerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ZwpConfinedPointerV1Error> {
        let _req: Destroy = self.constraint.client.parse(self, msg)?;
        self.constraint.detach();
        self.constraint.client.remove_obj(self)?;
        Ok(())
    }

    fn set_region(&self, msg: MsgParser<'_, '_>) -> Result<(), ZwpConfinedPointerV1Error> {
        let req: SetRegion = self.constraint.client.parse(self, msg)?;
        self.constraint.set_region(req.region)?;
        Ok(())
    }
}

impl ConstraintOwner for ZwpConfinedPointerV1 {
    fn send_enabled(&self) {
        self.constraint.client.event(Confined { self_id: self.id });
    }

    fn send_disabled(&self) {
        self.constraint
            .client
            .event(Unconfined { self_id: self.id });
    }
}

object_base! {
    ZwpConfinedPointerV1;

    DESTROY => destroy,
    SET_REGION => set_region,
}

impl Object for ZwpConfinedPointerV1 {
    fn num_requests(&self) -> u32 {
        SET_REGION + 1
    }

    fn break_loops(&self) {
        self.constraint.detach();
    }
}

simple_add_obj!(ZwpConfinedPointerV1);

#[derive(Debug, Error)]
pub enum ZwpConfinedPointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ZwpPointerConstraintsV1Error(#[from] ZwpPointerConstraintsV1Error),
}
efrom!(ZwpConfinedPointerV1Error, ClientError);
efrom!(ZwpConfinedPointerV1Error, MsgParserError);
