use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_seat::*, WlSeatId},
        wl_usr::{usr_ifs::usr_wl_pointer::UsrWlPointer, usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct UsrWlSeat {
    pub id: WlSeatId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlSeatOwner>>>,
}

pub trait UsrWlSeatOwner {
    fn capabilities(self: Rc<Self>, value: u32) {
        let _ = value;
    }

    fn name(&self, name: &str) {
        let _ = name;
    }
}

impl UsrWlSeat {
    pub fn get_pointer(&self) -> Rc<UsrWlPointer> {
        let ptr = Rc::new(UsrWlPointer {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            any_scroll_events: Cell::new(false),
            pending_scroll: Default::default(),
        });
        self.con.add_object(ptr.clone());
        self.con.request(GetPointer {
            self_id: self.id,
            id: ptr.id,
        });
        ptr
    }

    fn capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Capabilities = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.capabilities(ev.capabilities);
        }
        Ok(())
    }

    fn name(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Name = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.name(ev.name);
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWlSeat, WlSeat;

    CAPABILITIES => capabilities,
    NAME => name,
}

impl UsrObject for UsrWlSeat {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
