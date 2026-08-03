use crate::client::Client;
use crate::client::ClientError;
use crate::criteria::CritUpstreamNode;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::tree::ToplevelData;
use crate::wire::JayWindowMatchId;
use crate::wire::jay_window_match::*;
use std::rc::Rc;
use thiserror::Error;

pub struct JayWindowMatch {
    pub id: JayWindowMatchId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub m: Rc<dyn CritUpstreamNode<ToplevelData>>,
}

impl JayWindowMatchRequestHandler for JayWindowMatch {
    type Error = JayWindowMatchError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayWindowMatch;
    version = self.version;
}

impl Object for JayWindowMatch {}

dedicated_add_obj!(JayWindowMatch, JayWindowMatchId, jay_window_match);

#[derive(Debug, Error)]
pub enum JayWindowMatchError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayWindowMatchError, ClientError);
