use {
    crate::{
        ifs::jay_toplevel::ID_SINCE,
        object::Version,
        utils::clonecell::CloneCell,
        wire::{JaySelectToplevelId, jay_select_toplevel::*},
        wl_usr::{
            UsrCon,
            usr_ifs::usr_jay_toplevel::{UsrJayToplevel, UsrJayToplevelOwner},
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJaySelectToplevel {
    pub id: JaySelectToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectToplevelOwner>>>,
    pub version: Version,
}

impl UsrJaySelectToplevel {
    fn send(&self, tl: Option<Rc<UsrJayToplevel>>) {
        if let Some(owner) = self.owner.get() {
            owner.done(tl);
        } else {
            if let Some(tl) = tl {
                self.con.remove_obj(&*tl);
            }
        }
        self.con.remove_obj(self);
    }
}

pub trait UsrJaySelectToplevelOwner {
    fn done(&self, toplevel: Option<Rc<UsrJayToplevel>>);
}

impl JaySelectToplevelEventHandler for UsrJaySelectToplevel {
    type Error = Infallible;

    fn done(&self, ev: Done, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = if ev.id.is_none() {
            None
        } else {
            let tl = Rc::new(UsrJayToplevel {
                id: ev.id,
                con: self.con.clone(),
                owner: Default::default(),
                version: self.version,
                toplevel_id: Default::default(),
            });
            self.con.add_object(tl.clone());
            Some(tl)
        };
        'send: {
            if self.version >= ID_SINCE
                && let Some(tl) = tl
            {
                tl.owner.set(Some(slf.clone()));
                break 'send;
            }
            self.send(tl);
        }
        Ok(())
    }
}

impl UsrJayToplevelOwner for UsrJaySelectToplevel {
    fn done(&self, tl: &Rc<UsrJayToplevel>) {
        tl.owner.take();
        self.send(Some(tl.clone()));
    }
}

usr_object_base! {
    self = UsrJaySelectToplevel = JaySelectToplevel;
    version = self.version;
}

impl UsrObject for UsrJaySelectToplevel {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
