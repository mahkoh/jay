use crate::client::Client;
use crate::criteria::CritUpstreamNode;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::ToplevelData;
use crate::wire::JayWindowMatchId;
use crate::wire::jay_window_match::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct JayWindowMatch {
    pub id: JayWindowMatchId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub m: Rc<dyn CritUpstreamNode<ToplevelData>>,
}

impl JayWindowMatchRequestHandler for JayWindowMatch {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
