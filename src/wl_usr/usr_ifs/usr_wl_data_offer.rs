use {
    crate::{
        object::Version,
        wire::{WlDataOfferId, wl_data_offer::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    ahash::AHashSet,
    std::{cell::RefCell, convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrWlDataOffer {
    pub id: WlDataOfferId,
    pub con: Rc<UsrCon>,
    pub version: Version,
    pub mime_types: RefCell<AHashSet<String>>,
}

impl UsrWlDataOffer {
    pub fn receive(&self, mime_type: &str, fd: &Rc<OwnedFd>) {
        self.con.request(Receive {
            self_id: self.id,
            mime_type,
            fd: fd.clone(),
        });
    }
}

impl WlDataOfferEventHandler for UsrWlDataOffer {
    type Error = Infallible;

    fn offer(&self, ev: Offer<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.mime_types
            .borrow_mut()
            .insert(ev.mime_type.to_string());
        Ok(())
    }

    fn source_actions(&self, _ev: SourceActions, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn action(&self, _ev: Action, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlDataOffer = WlDataOffer;
    version = self.version;
}

impl UsrObject for UsrWlDataOffer {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
