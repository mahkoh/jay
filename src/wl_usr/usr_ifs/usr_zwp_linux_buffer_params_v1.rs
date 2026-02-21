use {
    crate::{
        format::Format,
        object::Version,
        video::Modifier,
        wire::{ZwpLinuxBufferParamsV1Id, zwp_linux_buffer_params_v1::*},
        wl_usr::{UsrCon, usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrZwpLinuxBufferParamsV1 {
    pub id: ZwpLinuxBufferParamsV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrZwpLinuxBufferParamsV1 {
    pub fn add(
        &self,
        fd: &Rc<OwnedFd>,
        plane_idx: usize,
        offset: u32,
        stride: u32,
        modifier: Modifier,
    ) {
        self.con.request(Add {
            self_id: self.id,
            fd: fd.clone(),
            plane_idx: plane_idx as u32,
            offset,
            stride,
            modifier,
        });
    }

    pub fn create_immed(&self, width: i32, height: i32, format: &Format) -> Rc<UsrWlBuffer> {
        let obj = Rc::new(UsrWlBuffer {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(CreateImmed {
            self_id: self.id,
            buffer_id: obj.id,
            width,
            height,
            format: format.drm,
            flags: 0,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl ZwpLinuxBufferParamsV1EventHandler for UsrZwpLinuxBufferParamsV1 {
    type Error = Infallible;

    fn created(&self, _ev: Created, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn failed(&self, _ev: Failed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrZwpLinuxBufferParamsV1 = ZwpLinuxDmabufV1;
    version = self.version;
}

impl UsrObject for UsrZwpLinuxBufferParamsV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
