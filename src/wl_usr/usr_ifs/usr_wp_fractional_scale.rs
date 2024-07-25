use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{wp_fractional_scale_v1::*, WpFractionalScaleV1Id},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWpFractionalScale {
    pub id: WpFractionalScaleV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWpFractionalScaleOwner>>>,
    pub version: Version,
}

pub trait UsrWpFractionalScaleOwner {
    fn preferred_scale(self: Rc<Self>, ev: &PreferredScale) {
        let _ = ev;
    }
}

impl WpFractionalScaleV1EventHandler for UsrWpFractionalScale {
    type Error = Infallible;

    fn preferred_scale(&self, ev: PreferredScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.preferred_scale(&ev);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWpFractionalScale = WpFractionalScaleV1;
    version = self.version;
}

impl UsrObject for UsrWpFractionalScale {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
