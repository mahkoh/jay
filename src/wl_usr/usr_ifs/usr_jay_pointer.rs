use {
    crate::{
        cursor::KnownCursor,
        object::Version,
        wire::{jay_pointer::*, JayPointerId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJayPointer {
    pub id: JayPointerId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrJayPointer {
    pub fn set_known_cursor(&self, cursor: KnownCursor) {
        self.con.request(SetKnownCursor {
            self_id: self.id,
            idx: cursor as usize as _,
        });
    }
}

impl JayPointerEventHandler for UsrJayPointer {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrJayPointer = JayPointer;
    version = self.version;
}

impl UsrObject for UsrJayPointer {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
