use {
    super::HeadCommon,
    crate::{
        client::{Client, ClientError},
        ifs::head_management::HeadCommonError,
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayHeadV1Id, jay_head_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayHeadV1 {
    pub id: JayHeadV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub common: Rc<HeadCommon>,
}

impl JayHeadV1 {
    pub fn send_removed(&self) {
        self.common.removed.set(true);
        self.client.event(Removed { self_id: self.id });
    }
}

impl JayHeadV1RequestHandler for JayHeadV1 {
    type Error = JayHeadV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_removed()?;
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadV1;
    version = self.version;
}

impl Object for JayHeadV1 {}

simple_add_obj!(JayHeadV1);

#[derive(Debug, Error)]
pub enum JayHeadV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Common(#[from] HeadCommonError),
}
efrom!(JayHeadV1Error, ClientError);
