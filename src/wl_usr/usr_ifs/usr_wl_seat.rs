use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{WlSeatId, wl_seat::*},
        wl_usr::{UsrCon, usr_ifs::usr_wl_pointer::UsrWlPointer, usr_object::UsrObject},
    },
    std::{cell::Cell, convert::Infallible, rc::Rc},
};

pub struct UsrWlSeat {
    pub id: WlSeatId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlSeatOwner>>>,
    pub version: Version,
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
            version: self.version,
        });
        self.con.add_object(ptr.clone());
        self.con.request(GetPointer {
            self_id: self.id,
            id: ptr.id,
        });
        ptr
    }
}

impl WlSeatEventHandler for UsrWlSeat {
    type Error = Infallible;

    fn capabilities(&self, ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.capabilities(ev.capabilities);
        }
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.name(ev.name);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlSeat = WlSeat;
    version = self.version;
}

impl UsrObject for UsrWlSeat {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
