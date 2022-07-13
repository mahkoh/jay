use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wp_fractional_scale_v1::*, WpFractionalScaleV1Id},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWpFractionalScale {
    pub id: WpFractionalScaleV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWpFractionalScaleOwner>>>,
}

pub trait UsrWpFractionalScaleOwner {
    fn preferred_scale(self: Rc<Self>, ev: &PreferredScale) {
        let _ = ev;
    }
}

impl UsrWpFractionalScale {
    fn preferred_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: PreferredScale = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.preferred_scale(&ev);
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWpFractionalScale, WpFractionalScaleV1;

    PREFERRED_SCALE => preferred_scale,
}

impl UsrObject for UsrWpFractionalScale {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
