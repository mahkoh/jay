use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::ToplevelOpt;
use crate::wire::ExtForeignToplevelHandleV1Id;
use crate::wire::ext_foreign_toplevel_handle_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct ExtForeignToplevelHandleV1 {
    pub id: ExtForeignToplevelHandleV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub toplevel: ToplevelOpt,
    pub version: Version,
}

impl ExtForeignToplevelHandleV1 {
    fn detach(&self) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_data().handles.remove(&(self.client.id, self.id));
        }
    }
}

impl ExtForeignToplevelHandleV1RequestHandler for ExtForeignToplevelHandleV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl ExtForeignToplevelHandleV1 {
    pub fn send_closed(&self) {
        self.client.event(Closed { self_id: self.id });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_title(&self, title: &str) {
        self.client.event(Title {
            self_id: self.id,
            title,
        });
    }

    pub fn send_app_id(&self, app_id: &str) {
        self.client.event(AppId {
            self_id: self.id,
            app_id,
        });
    }

    pub fn send_identifier(&self, identifier: &str) {
        self.client.event(Identifier {
            self_id: self.id,
            identifier,
        });
    }
}

impl BreakLoops for ExtForeignToplevelHandleV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
