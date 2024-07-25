use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{zwlr_screencopy_frame_v1::*, ZwlrScreencopyFrameV1Id},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrZwlrScreencopyFrame {
    pub id: ZwlrScreencopyFrameV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrZwlrScreencopyFrameOwner>>>,
    pub version: Version,
}

pub trait UsrZwlrScreencopyFrameOwner {
    fn buffer(&self, buffer: &Buffer) {
        let _ = buffer;
    }

    fn flags(&self, flags: &Flags) {
        let _ = flags;
    }

    fn ready(&self, ready: &Ready) {
        let _ = ready;
    }

    fn failed(&self) {}

    fn damage(&self, damage: &Damage) {
        let _ = damage;
    }

    fn linux_dmabuf(&self, dmabuf: &LinuxDmabuf) {
        let _ = dmabuf;
    }

    fn buffer_done(&self) {}
}

impl UsrZwlrScreencopyFrame {
    #[allow(dead_code)]
    pub fn copy(&self, buffer: &UsrWlBuffer) {
        self.con.request(Copy {
            self_id: self.id,
            buffer: buffer.id,
        });
    }

    #[allow(dead_code)]
    pub fn copy_with_damage(&self, buffer: &UsrWlBuffer) {
        self.con.request(CopyWithDamage {
            self_id: self.id,
            buffer: buffer.id,
        });
    }
}

impl ZwlrScreencopyFrameV1EventHandler for UsrZwlrScreencopyFrame {
    type Error = Infallible;

    fn buffer(&self, ev: Buffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.buffer(&ev);
        }
        Ok(())
    }

    fn flags(&self, ev: Flags, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.flags(&ev);
        }
        Ok(())
    }

    fn ready(&self, ev: Ready, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.ready(&ev);
        }
        Ok(())
    }

    fn failed(&self, _ev: Failed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.failed();
        }
        Ok(())
    }

    fn damage(&self, ev: Damage, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.damage(&ev);
        }
        Ok(())
    }

    fn linux_dmabuf(&self, ev: LinuxDmabuf, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.linux_dmabuf(&ev);
        }
        Ok(())
    }

    fn buffer_done(&self, _ev: BufferDone, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.buffer_done();
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrZwlrScreencopyFrame = ZwlrScreencopyFrameV1;
    version = self.version;
}

impl UsrObject for UsrZwlrScreencopyFrame {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
