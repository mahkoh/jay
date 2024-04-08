use {
    crate::{
        client::ClientError,
        ifs::wl_seat::zwp_pointer_constraints_v1::{
            ConstraintOwner, SeatConstraint, ZwpPointerConstraintsV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_confined_pointer_v1::*, ZwpConfinedPointerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpConfinedPointerV1 {
    pub id: ZwpConfinedPointerV1Id,
    pub tracker: Tracker<Self>,
    pub constraint: Rc<SeatConstraint>,
    pub version: Version,
}

impl ZwpConfinedPointerV1RequestHandler for ZwpConfinedPointerV1 {
    type Error = ZwpConfinedPointerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.constraint.detach();
        self.constraint.client.remove_obj(self)?;
        Ok(())
    }

    fn set_region(&self, req: SetRegion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
    self = ZwpConfinedPointerV1;
    version = self.version;
}

impl Object for ZwpConfinedPointerV1 {
    fn break_loops(&self) {
        self.constraint.detach();
    }
}

simple_add_obj!(ZwpConfinedPointerV1);

#[derive(Debug, Error)]
pub enum ZwpConfinedPointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwpPointerConstraintsV1Error(#[from] ZwpPointerConstraintsV1Error),
}
efrom!(ZwpConfinedPointerV1Error, ClientError);
