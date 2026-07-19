use crate::object::Version;
use crate::utils::bhash::BHashSet;
use crate::wire::WlDataOfferId;
use crate::wire::wl_data_offer::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct UsrWlDataOffer {
    pub id: WlDataOfferId,
    pub con: Rc<UsrCon>,
    pub version: Version,
    pub mime_types: RefCell<BHashSet<String>>,
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
