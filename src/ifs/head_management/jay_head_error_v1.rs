use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayHeadErrorV1Id, jay_head_error_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayHeadErrorV1 {
    pub id: JayHeadErrorV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayHeadErrorV1RequestHandler for JayHeadErrorV1 {
    type Error = JayHeadErrorV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadErrorV1;
    version = self.version;
}

impl Object for JayHeadErrorV1 {}

dedicated_add_obj!(JayHeadErrorV1, JayHeadErrorV1Id, jay_head_errors);

#[derive(Debug, Error)]
pub enum JayHeadErrorV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayHeadErrorV1Error, ClientError);
