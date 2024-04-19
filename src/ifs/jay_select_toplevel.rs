use {
    crate::{
        client::{Client, ClientError},
        ifs::{jay_toplevel::JayToplevel, wl_seat::ToplevelSelector},
        leaks::Tracker,
        object::{Object, Version},
        tree::ToplevelNode,
        utils::clonecell::CloneCell,
        wire::{jay_select_toplevel::*, JaySelectToplevelId, JayToplevelId},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JaySelectToplevel {
    pub id: JaySelectToplevelId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub destroyed: Cell<bool>,
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
        let id = match self.tl.take() {
            None => JayToplevelId::NONE,
            Some(toplevel) => {
                let id = match self.jst.client.new_id() {
                    Ok(id) => id,
                    Err(e) => {
                        self.jst.client.error(e);
                        return;
                    }
                };
                let jtl = Rc::new(JayToplevel {
                    id,
                    client: self.jst.client.clone(),
                    tracker: Default::default(),
                    toplevel,
                    destroyed: Cell::new(false),
                });
                track!(self.jst.client, jtl);
                self.jst.client.add_server_obj(&jtl);
                jtl.toplevel
                    .tl_data()
                    .jay_toplevels
                    .set((jtl.client.id, jtl.id), jtl.clone());
                jtl.id
            }
        };
        self.jst.send_done(id);
        let _ = self.jst.client.remove_obj(&*self.jst);
    }
}

impl JaySelectToplevel {
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
