use {
    crate::{
        cursor::KnownCursor,
        wire::{jay_pointer::*, JayPointerId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrJayPointer {
    pub id: JayPointerId,
    pub con: Rc<UsrCon>,
}

impl UsrJayPointer {
    pub fn set_known_cursor(&self, cursor: KnownCursor) {
        self.con.request(SetKnownCursor {
            self_id: self.id,
            idx: cursor as usize as _,
        });
    }
}

usr_object_base! {
    UsrJayPointer, JayPointer;
}

impl UsrObject for UsrJayPointer {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
