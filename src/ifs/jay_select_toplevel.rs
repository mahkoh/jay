use crate::client::Client;
use crate::ifs::jay_toplevel::CLIENT_ID_SINCE;
use crate::ifs::jay_toplevel::ID_SINCE;
use crate::ifs::jay_toplevel::JayToplevel;
use crate::ifs::wl_seat::ToplevelSelector;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::ToplevelNode;
use crate::utils::clonecell::CloneCell;
use crate::wire::JaySelectToplevelId;
use crate::wire::JayToplevelId;
use crate::wire::jay_select_toplevel::*;
use jay_proc::Object;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JaySelectToplevel {
    id: JaySelectToplevelId,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
    destroyed: Cell<bool>,
    version: Version,
}

pub struct JayToplevelSelector {
    pub tl: CloneCell<Option<Rc<dyn ToplevelNode>>>,
    pub jst: Rc<JaySelectToplevel>,
}

impl ToplevelSelector for JayToplevelSelector {
    fn set(&self, toplevel: Rc<dyn ToplevelNode>) {
        self.tl.set(Some(toplevel));
    }
}

impl Drop for JayToplevelSelector {
    fn drop(&mut self) {
        if self.jst.destroyed.get() {
            return;
        }
        self.jst.done(self.tl.take());
    }
}

impl JaySelectToplevel {
    pub fn done(&self, tl: Option<Rc<dyn ToplevelNode>>) {
        let jtl = match tl {
            None => None,
            Some(toplevel) => {
                let id = self.client.new_id(self);
                let jtl = Rc::new(JayToplevel {
                    id,
                    client: self.client.clone(),
                    tracker: Default::default(),
                    toplevel,
                    destroyed: Cell::new(false),
                    version: self.version,
                });
                track!(self.client, jtl);
                self.client.add_server_obj(&jtl);
                jtl.toplevel
                    .tl_data()
                    .jay_toplevels
                    .set((jtl.client.id, jtl.id), jtl.clone());
                Some(jtl)
            }
        };
        match jtl {
            None => self.send_done(JayToplevelId::NONE),
            Some(jtl) => {
                self.send_done(jtl.id);
                if jtl.version >= ID_SINCE {
                    jtl.send_id();
                    if jtl.version >= CLIENT_ID_SINCE {
                        jtl.send_client_id();
                    }
                    jtl.send_done();
                }
            }
        }
        self.client.remove_obj(self);
    }

    pub fn new(client: &Rc<Client>, id: JaySelectToplevelId, version: Version) -> Rc<Self> {
        Rc::new(JaySelectToplevel {
            id,
            client: client.clone(),
            tracker: Default::default(),
            destroyed: Cell::new(false),
            version,
        })
    }

    fn send_done(&self, id: JayToplevelId) {
        self.client.event(Done {
            self_id: self.id,
            id,
        });
    }
}

impl JaySelectToplevelRequestHandler for JaySelectToplevel {
    type Error = Infallible;
}

impl BreakLoops for JaySelectToplevel {
    fn break_loops(self: Rc<Self>) {
        self.destroyed.set(true);
    }
}
