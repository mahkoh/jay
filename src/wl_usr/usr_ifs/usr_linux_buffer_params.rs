use {
    crate::{
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        video::dmabuf::DmaBuf,
        wire::{zwp_linux_buffer_params_v1::*, ZwpLinuxBufferParamsV1Id},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::{ops::Deref, rc::Rc},
};

pub struct UsrLinuxBufferParams {
    pub id: ZwpLinuxBufferParamsV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrLinuxBufferParamsOwner>>>,
}

pub trait UsrLinuxBufferParamsOwner {
    fn created(&self, buffer: Rc<UsrWlBuffer>) {
        buffer.con.remove_obj(buffer.deref());
    }

    fn failed(&self) {}
}

impl UsrLinuxBufferParams {
    pub fn create(&self, buf: &DmaBuf) {
        for (idx, plane) in buf.planes.iter().enumerate() {
            self.con.request(Add {
                self_id: self.id,
                fd: plane.fd.clone(),
                plane_idx: idx as _,
                offset: plane.offset,
                stride: plane.stride,
                modifier_hi: (buf.modifier >> 32) as _,
                modifier_lo: buf.modifier as _,
            });
        }
        self.con.request(Create {
            self_id: self.id,
            width: buf.width,
            height: buf.height,
            format: buf.format.drm,
            flags: 0,
        });
    }

    fn created(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Created = self.con.parse(self, parser)?;
        let buffer = Rc::new(UsrWlBuffer {
            id: ev.buffer,
            con: self.con.clone(),
            owner: Default::default(),
        });
        self.con.add_object(buffer.clone());
        if let Some(owner) = self.owner.get() {
            owner.created(buffer);
        } else {
            self.con.remove_obj(buffer.deref());
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
}

usr_object_base! {
    UsrLinuxBufferParams, ZwpLinuxBufferParamsV1;

    CREATED => created,
    FAILED => failed,
}

impl UsrObject for UsrLinuxBufferParams {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
