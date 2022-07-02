use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        wire::{zwlr_screencopy_frame_v1::*, ZwlrScreencopyFrameV1Id},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrZwlrScreencopyFrame {
    pub id: ZwlrScreencopyFrameV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrZwlrScreencopyFrameOwner>>>,
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
    pub fn request_copy(&self, buffer: &UsrWlBuffer) {
        self.con.request(Copy {
            self_id: self.id,
            buffer: buffer.id,
        });
    }

    #[allow(dead_code)]
    pub fn request_copy_with_damage(&self, buffer: &UsrWlBuffer) {
        self.con.request(CopyWithDamage {
            self_id: self.id,
            buffer: buffer.id,
        });
    }

    fn buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Buffer = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.buffer(&ev);
        }
        Ok(())
    }

    fn flags(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Flags = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.flags(&ev);
        }
        Ok(())
    }

    fn ready(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Ready = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.ready(&ev);
        }
        Ok(())
    }

    fn failed(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Failed = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.failed();
        }
        Ok(())
    }

    fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Damage = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.damage(&ev);
        }
        Ok(())
    }

    fn linux_dmabuf(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: LinuxDmabuf = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.linux_dmabuf(&ev);
        }
        Ok(())
    }

    fn buffer_done(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: BufferDone = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.buffer_done();
        }
        Ok(())
    }
}

impl Drop for UsrZwlrScreencopyFrame {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrZwlrScreencopyFrame, ZwlrScreencopyFrameV1;

    BUFFER => buffer,
    FLAGS => flags,
    READY => ready,
    FAILED => failed,
    DAMAGE => damage,
    LINUX_DMABUF => linux_dmabuf,
    BUFFER_DONE => buffer_done,
}

impl UsrObject for UsrZwlrScreencopyFrame {
    fn break_loops(&self) {
        self.owner.take();
    }
}
