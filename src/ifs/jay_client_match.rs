use crate::client::Client;
use crate::client::ClientError;
use crate::criteria::CritUpstreamNode;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayClientMatchId;
use crate::wire::jay_client_match::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct JayClientMatch {
    pub id: JayClientMatchId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub m: Rc<dyn CritUpstreamNode<Rc<Client>>>,
}

impl JayClientMatchRequestHandler for JayClientMatch {
    type Error = JayClientMatchError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayClientMatchError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayClientMatchError, ClientError);
