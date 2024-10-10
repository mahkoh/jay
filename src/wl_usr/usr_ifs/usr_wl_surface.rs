use {
    crate::{
        object::Version,
        wire::{wl_surface::*, WlSurfaceId},
        wl_usr::{
            usr_ifs::{usr_wl_buffer::UsrWlBuffer, usr_wl_callback::UsrWlCallback},
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlSurface {
    pub id: WlSurfaceId,
    pub con: Rc<UsrCon>,
    pub version: Version,
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

    pub fn damage(&self) {
        self.con.request(DamageBuffer {
            self_id: self.id,
            x: 0,
            y: 0,
            width: i32::MAX,
            height: i32::MAX,
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
}

impl WlSurfaceEventHandler for UsrWlSurface {
    type Error = Infallible;

    fn enter(&self, _ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn leave(&self, _ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn preferred_buffer_scale(
        &self,
        _ev: PreferredBufferScale,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn preferred_buffer_transform(
        &self,
        _ev: PreferredBufferTransform,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlSurface = WlSurface;
    version = self.version;
}

impl UsrObject for UsrWlSurface {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
