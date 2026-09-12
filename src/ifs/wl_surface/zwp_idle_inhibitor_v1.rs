use crate::client::Client;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::ZwpIdleInhibitorV1Id;
use crate::wire::zwp_idle_inhibitor_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

linear_ids!(IdleInhibitorIds, IdleInhibitorId, u64);

#[derive(Object)]
#[break_loops]
pub struct ZwpIdleInhibitorV1 {
    pub id: ZwpIdleInhibitorV1Id,
    pub inhibit_id: IdleInhibitorId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpIdleInhibitorV1RequestHandler for ZwpIdleInhibitorV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        if self.surface.idle_inhibitors.remove(&self.id).is_some() {
            self.deactivate();
        }
        Ok(())
    }
}

impl ZwpIdleInhibitorV1 {
    pub fn install(self: &Rc<Self>) {
        self.surface.idle_inhibitors.insert(self.id, self.clone());
        if self.surface.visible[LiveTL].get() {
            self.activate();
        }
    }

    pub fn activate(self: &Rc<Self>) {
        let state = &self.client.state;
        state.idle.add_inhibitor(state, self);
    }

    pub fn deactivate(&self) {
        let state = &self.client.state;
        state.idle.remove_inhibitor(state, self);
    }
}

impl BreakLoops for ZwpIdleInhibitorV1 {
    fn break_loops(self: Rc<Self>) {
        self.deactivate();
    }
}
