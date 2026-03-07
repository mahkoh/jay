use {
    crate::{
        object::Version,
        video::dmabuf::DmaBuf,
        wire::{ZwpLinuxDmabufV1Id, zwp_linux_dmabuf_v1::*},
        wl_usr::{
            UsrCon,
            usr_ifs::{
                usr_wl_buffer::UsrWlBuffer,
                usr_zwp_linux_buffer_params_v1::UsrZwpLinuxBufferParamsV1,
            },
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrZwpLinuxDmabufV1 {
    pub id: ZwpLinuxDmabufV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrZwpLinuxDmabufV1 {
    #[expect(dead_code)]
    pub fn create_buffer(&self, buffer: &DmaBuf) -> Rc<UsrWlBuffer> {
        let params = Rc::new(UsrZwpLinuxBufferParamsV1 {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
        });
        self.con.request(CreateParams {
            self_id: self.id,
            params_id: params.id,
        });
        self.con.add_object(params.clone());
        for (idx, plane) in buffer.planes.iter().enumerate() {
            params.add(&plane.fd, idx, plane.offset, plane.stride, buffer.modifier);
        }
        let obj = params.create_immed(buffer.width, buffer.height, &buffer.format);
        self.con.remove_obj(&*params);
        obj
    }
}

impl ZwpLinuxDmabufV1EventHandler for UsrZwpLinuxDmabufV1 {
    type Error = Infallible;

    fn format(&self, _ev: Format, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn modifier(&self, _ev: Modifier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrZwpLinuxDmabufV1 = ZwpLinuxDmabufV1;
    version = self.version;
}

impl UsrObject for UsrZwpLinuxDmabufV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
