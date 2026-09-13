use crate::client::Client;
use crate::ifs::wl_output::OutputGlobalOpt;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::wire::JayOutputId;
use crate::wire::jay_output::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JayOutput {
    pub id: JayOutputId,
    pub version: Version,
    pub client: Rc<Client>,
    pub output: Rc<OutputGlobalOpt>,
    pub tracker: Tracker<Self>,
}

impl JayOutput {
    pub fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn send_linear_id(&self) {
        if let Some(output) = self.output.node() {
            self.client.event(LinearId {
                self_id: self.id,
                linear_id: output.id.raw(),
            });
        }
    }

    fn remove_from_node(&self) {
        if let Some(output) = self.output.node() {
            output.jay_outputs.remove(&(self.client.id, self.id));
        }
    }
}

impl JayOutputRequestHandler for JayOutput {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_node();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for JayOutput {
    fn break_loops(self: Rc<Self>) {
        self.remove_from_node();
    }
}
