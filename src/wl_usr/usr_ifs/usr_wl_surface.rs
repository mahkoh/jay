use {
    crate::{
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_surface::*, WlSurfaceId},
        wl_usr::{
            usr_ifs::{usr_wl_buffer::UsrWlBuffer, usr_wl_callback::UsrWlCallback},
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::rc::Rc,
};

pub struct UsrWlSurface {
    pub id: WlSurfaceId,
    pub con: Rc<UsrCon>,
}

impl UsrWlSurface {
    pub fn attach(&self, buffer: &UsrWlBuffer) {
        self.con.request(Attach {
            self_id: self.id,
            buffer: buffer.id,
            x: 0,
            y: 0,
        });
    }

    pub fn frame<F>(&self, f: F)
    where
        F: FnOnce() + 'static,
    {
        let cb = Rc::new(UsrWlCallback::new(&self.con, f));
        self.con.request(Frame {
            self_id: self.id,
            callback: cb.id,
        });
        self.con.add_object(cb);
    }

    pub fn commit(&self) {
        self.con.request(Commit { self_id: self.id });
    }

    fn enter(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Enter = self.con.parse(self, parser)?;
        Ok(())
    }

    fn leave(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Leave = self.con.parse(self, parser)?;
        Ok(())
    }

    fn preferred_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: PreferredBufferScale = self.con.parse(self, parser)?;
        Ok(())
    }

    fn preferred_buffer_transform(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: PreferredBufferTransform = self.con.parse(self, parser)?;
        Ok(())
    }
}

usr_object_base! {
    UsrWlSurface, WlSurface;

    ENTER => enter,
    LEAVE => leave,
    PREFERRED_BUFFER_SCALE => preferred_buffer_scale,
    PREFERRED_BUFFER_TRANSFORM => preferred_buffer_transform,
}

impl UsrObject for UsrWlSurface {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
