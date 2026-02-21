use {
    crate::{
        object::Version,
        utils::clonecell::CloneCell,
        wire::{WlDataSourceId, wl_data_source::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrWlDataSource {
    pub id: WlDataSourceId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlDataSourceOwner>>>,
    pub version: Version,
}

pub trait UsrWlDataSourceOwner {
    fn send(&self, mime_type: &str, fd: Rc<OwnedFd>);
}

impl UsrWlDataSource {
    pub fn offer(&self, mime_type: &str) {
        self.con.request(Offer {
            self_id: self.id,
            mime_type,
        });
    }
}

impl WlDataSourceEventHandler for UsrWlDataSource {
    type Error = Infallible;

    fn target(&self, ev: Target<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn send(&self, ev: Send<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.send(ev.mime_type, ev.fd);
        }
        Ok(())
    }

    fn cancelled(&self, ev: Cancelled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn dnd_drop_performed(&self, ev: DndDropPerformed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn dnd_finished(&self, ev: DndFinished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn action(&self, ev: Action, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlDataSource = WlDataSource;
    version = self.version;
}

impl UsrObject for UsrWlDataSource {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
