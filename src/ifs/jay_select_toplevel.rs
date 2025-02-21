use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            jay_toplevel::{ID_SINCE, JayToplevel},
            wl_seat::ToplevelSelector,
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::ToplevelNode,
        utils::clonecell::CloneCell,
        wire::{JaySelectToplevelId, JayToplevelId, jay_select_toplevel::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JaySelectToplevel {
    pub id: JaySelectToplevelId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub destroyed: Cell<bool>,
    pub version: Version,
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
                let id = match self.client.new_id() {
                    Ok(id) => id,
                    Err(e) => {
                        self.client.error(e);
                        return;
                    }
                };
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
                    jtl.send_done();
                }
            }
        }
        let _ = self.client.remove_obj(self);
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
    type Error = JaySelectToplevelError;
}

object_base! {
    self = JaySelectToplevel;
    version = Version(1);
}

impl Object for JaySelectToplevel {
    fn break_loops(&self) {
        self.destroyed.set(true);
    }
}

simple_add_obj!(JaySelectToplevel);

#[derive(Debug, Error)]
pub enum JaySelectToplevelError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JaySelectToplevelError, ClientError);
