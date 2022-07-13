use {
    crate::{
        fixed::Fixed,
        wire::{wp_viewport::*, WpViewportId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWpViewport {
    pub id: WpViewportId,
    pub con: Rc<UsrCon>,
}

impl UsrWpViewport {
    pub fn set_source(&self, x: Fixed, y: Fixed, width: Fixed, height: Fixed) {
        self.con.request(SetSource {
            self_id: self.id,
            x,
            y,
            width,
            height,
        });
    }

    pub fn set_destination(&self, width: i32, height: i32) {
        self.con.request(SetDestination {
            self_id: self.id,
            width,
            height,
        });
    }
}

usr_object_base! {
    UsrWpViewport, WpViewport;
}

impl UsrObject for UsrWpViewport {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
