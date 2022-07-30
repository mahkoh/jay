use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{wl_output::*, WlOutputId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlOutput {
    pub id: WlOutputId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlOutputOwner>>>,
}

pub trait UsrWlOutputOwner {
    fn geometry(&self, ev: &Geometry) {
        let _ = ev;
    }

    fn mode(&self, ev: &Mode) {
        let _ = ev;
    }

    fn done(&self) {}

    fn scale(&self, ev: &Scale) {
        let _ = ev;
    }

    fn name(&self, ev: &Name) {
        let _ = ev;
    }

    fn description(&self, ev: &Description) {
        let _ = ev;
    }
}

impl UsrWlOutput {
    fn geometry(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Geometry = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.geometry(&ev);
        }
        Ok(())
    }

    fn mode(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Mode = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.mode(&ev);
        }
        Ok(())
    }

    fn done(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Done = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.done();
        }
        Ok(())
    }

    fn scale(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Scale = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.scale(&ev);
        }
        Ok(())
    }

    fn name(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Name = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.name(&ev);
        }
        Ok(())
    }

    fn description(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Description = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.description(&ev);
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWlOutput, WlOutput;

    GEOMETRY => geometry,
    MODE => mode,
    DONE => done,
    SCALE => scale,
    NAME => name,
    DESCRIPTION => description,
}

impl UsrObject for UsrWlOutput {
    fn destroy(&self) {
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.set(None);
    }
}
