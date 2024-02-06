use {
    crate::{
        client::ClientError,
        ifs::wl_seat::zwp_pointer_constraints_v1::{
            ConstraintOwner, SeatConstraint, ZwpPointerConstraintsV1Error,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_locked_pointer_v1::*, ZwpLockedPointerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpLockedPointerV1 {
    pub id: ZwpLockedPointerV1Id,
    pub tracker: Tracker<Self>,
    pub constraint: Rc<SeatConstraint>,
}

impl ZwpLockedPointerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ZwpLockedPointerV1Error> {
        let _req: Destroy = self.constraint.client.parse(self, msg)?;
        self.constraint.detach();
        self.constraint.client.remove_obj(self)?;
        Ok(())
    }

    fn set_cursor_position_hint(
        &self,
        msg: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLockedPointerV1Error> {
        let _req: SetCursorPositionHint = self.constraint.client.parse(self, msg)?;
        Ok(())
    }

    fn set_region(&self, msg: MsgParser<'_, '_>) -> Result<(), ZwpLockedPointerV1Error> {
        let req: SetRegion = self.constraint.client.parse(self, msg)?;
        self.constraint.set_region(req.region)?;
        Ok(())
    }
}

impl ConstraintOwner for ZwpLockedPointerV1 {
    fn send_enabled(&self) {
        self.constraint.client.event(Locked { self_id: self.id });
    }

    fn send_disabled(&self) {
        self.constraint.client.event(Unlocked { self_id: self.id });
    }
}

object_base! {
    self = ZwpLockedPointerV1;

    DESTROY => destroy,
    SET_CURSOR_POSITION_HINT => set_cursor_position_hint,
    SET_REGION => set_region,
}

impl Object for ZwpLockedPointerV1 {
    fn break_loops(&self) {
        self.constraint.detach();
    }
}

simple_add_obj!(ZwpLockedPointerV1);

#[derive(Debug, Error)]
pub enum ZwpLockedPointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ZwpPointerConstraintsV1Error(#[from] ZwpPointerConstraintsV1Error),
}
efrom!(ZwpLockedPointerV1Error, ClientError);
efrom!(ZwpLockedPointerV1Error, MsgParserError);
