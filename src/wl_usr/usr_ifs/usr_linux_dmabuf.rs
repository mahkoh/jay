use {
    crate::{
        format::{Format, formats},
        object::Version,
        utils::clonecell::CloneCell,
        wire::{
            ZwpLinuxDmabufV1Id,
            zwp_linux_dmabuf_v1::{self, *},
        },
        wl_usr::{
            UsrCon, usr_ifs::usr_linux_buffer_params::UsrLinuxBufferParams, usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrLinuxDmabuf {
    pub id: ZwpLinuxDmabufV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrLinuxDmabufOwner>>>,
    pub version: Version,
}

pub trait UsrLinuxDmabufOwner {
    fn modifier(&self, format: &'static Format, modifier: u64) {
        let _ = format;
        let _ = modifier;
    }
}

impl UsrLinuxDmabuf {
    pub fn create_params(&self) -> Rc<UsrLinuxBufferParams> {
        let params = Rc::new(UsrLinuxBufferParams {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(CreateParams {
            self_id: self.id,
            params_id: params.id,
        });
        self.con.add_object(params.clone());
        params
    }
}

impl ZwpLinuxDmabufV1EventHandler for UsrLinuxDmabuf {
    type Error = Infallible;

    fn format(&self, _ev: zwp_linux_dmabuf_v1::Format, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn modifier(&self, ev: Modifier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            if let Some(format) = formats().get(&ev.format) {
                owner.modifier(format, ev.modifier);
            }
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrLinuxDmabuf = ZwpLinuxDmabufV1;
    version = self.version;
}

impl UsrObject for UsrLinuxDmabuf {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
