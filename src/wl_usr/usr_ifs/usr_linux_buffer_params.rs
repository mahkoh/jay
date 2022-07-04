use {
    crate::{
        format::Format,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
        video::dmabuf::DmaBuf,
        wire::{zwp_linux_buffer_params_v1::*, ZwpLinuxBufferParamsV1Id},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct UsrLinuxBufferParams {
    pub id: ZwpLinuxBufferParamsV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrLinuxBufferParamsOwner>>>,
    pub format: Cell<Option<&'static Format>>,
    pub width: Cell<Option<i32>>,
    pub height: Cell<Option<i32>>,
}

pub trait UsrLinuxBufferParamsOwner {
    fn created(&self, buffer: Rc<UsrWlBuffer>) {
        let _ = buffer;
    }

    fn failed(&self) {}
}

impl UsrLinuxBufferParams {
    pub fn request_create(&self, buf: &DmaBuf) {
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
        self.width.set(Some(buf.width));
        self.height.set(Some(buf.height));
        self.format.set(Some(buf.format));
    }

    fn created(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Created = self.con.parse(self, parser)?;
        let buffer = UsrWlBuffer {
            id: ev.buffer,
            con: self.con.clone(),
            width: self.width.get().unwrap(),
            height: self.height.get().unwrap(),
            stride: None,
            format: self.format.get().unwrap(),
            owner: Default::default(),
        };
        if let Some(owner) = self.owner.get() {
            owner.created(Rc::new(buffer));
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

impl Drop for UsrLinuxBufferParams {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrLinuxBufferParams, ZwpLinuxBufferParamsV1;

    CREATED => created,
    FAILED => failed,
}

impl UsrObject for UsrLinuxBufferParams {
    fn break_loops(&self) {
        self.owner.take();
    }
}
