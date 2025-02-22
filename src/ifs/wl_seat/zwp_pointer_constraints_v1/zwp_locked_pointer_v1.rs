use {
    crate::{
        client::ClientError,
        ifs::wl_seat::zwp_pointer_constraints_v1::{
            ConstraintOwner, SeatConstraint, SeatConstraintStatus, ZwpPointerConstraintsV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpLockedPointerV1Id, zwp_locked_pointer_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpLockedPointerV1 {
    pub id: ZwpLockedPointerV1Id,
    pub tracker: Tracker<Self>,
    pub constraint: Rc<SeatConstraint>,
    pub version: Version,
}

impl ZwpLockedPointerV1RequestHandler for ZwpLockedPointerV1 {
    type Error = ZwpLockedPointerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.constraint.detach();
        self.constraint.client.remove_obj(self)?;
        Ok(())
    }

    fn set_cursor_position_hint(
        &self,
        req: SetCursorPositionHint,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if self.constraint.status.get() != SeatConstraintStatus::Active {
            return Ok(());
        }
        let mut x = req.surface_x;
        let mut y = req.surface_y;
        client_wire_scale_to_logical!(self.constraint.client, x, y);
        self.constraint.set_cursor_hint(x, y);
        Ok(())
    }

    fn set_region(&self, req: SetRegion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
    version = self.version;
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
    #[error(transparent)]
    ZwpPointerConstraintsV1Error(#[from] ZwpPointerConstraintsV1Error),
}
efrom!(ZwpLockedPointerV1Error, ClientError);
