use crate::client::Client;
use crate::criteria::CritUpstreamNode;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayClientMatchId;
use crate::wire::jay_client_match::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct JayClientMatch {
    pub id: JayClientMatchId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub m: Rc<dyn CritUpstreamNode<Rc<Client>>>,
}

impl JayClientMatchRequestHandler for JayClientMatch {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
