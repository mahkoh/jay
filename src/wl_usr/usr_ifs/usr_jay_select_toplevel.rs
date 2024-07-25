use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{jay_select_toplevel::*, JaySelectToplevelId},
        wl_usr::{usr_ifs::usr_jay_toplevel::UsrJayToplevel, usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJaySelectToplevel {
    pub id: JaySelectToplevelId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySelectToplevelOwner>>>,
    pub version: Version,
}

pub trait UsrJaySelectToplevelOwner {
    fn done(&self, toplevel: Option<Rc<UsrJayToplevel>>);
}

impl JaySelectToplevelEventHandler for UsrJaySelectToplevel {
    type Error = Infallible;

    fn done(&self, ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = if ev.id.is_none() {
            None
        } else {
            let tl = Rc::new(UsrJayToplevel {
                id: ev.id,
                con: self.con.clone(),
                owner: Default::default(),
                version: self.version,
            });
            self.con.add_object(tl.clone());
            Some(tl)
        };
        match self.owner.get() {
            Some(owner) => owner.done(tl),
            _ => {
                if let Some(tl) = tl {
                    self.con.remove_obj(&*tl);
                }
            }
        }
        self.con.remove_obj(self);
        Ok(())
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
