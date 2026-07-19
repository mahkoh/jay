use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::WlDataDeviceId;
use crate::wire::wl_data_device::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_data_offer::UsrWlDataOffer;
use crate::wl_usr::usr_ifs::usr_wl_data_source::UsrWlDataSource;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWlDataDevice {
    pub id: WlDataDeviceId,
    pub con: Rc<UsrCon>,
    pub version: Version,
    pub offer: CloneCell<Option<Rc<UsrWlDataOffer>>>,
    pub selection: CloneCell<Option<Rc<UsrWlDataOffer>>>,
}

impl UsrWlDataDevice {
    pub fn set_selection(&self, serial: u32, source: &UsrWlDataSource) {
        self.con.request(SetSelection {
            self_id: self.id,
            source: source.id,
            serial,
        });
    }
}

impl WlDataDeviceEventHandler for UsrWlDataDevice {
    type Error = Infallible;

    fn data_offer(&self, ev: DataOffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(UsrWlDataOffer {
            id: ev.id,
            con: self.con.clone(),
            version: self.version,
            mime_types: Default::default(),
        });
        self.con.add_object(obj.clone());
        if let Some(offer) = self.offer.set(Some(obj)) {
            self.con.remove_obj(&*offer);
        }
        Ok(())
    }

    fn enter(&self, ev: Enter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn leave(&self, ev: Leave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn motion(&self, ev: Motion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn drop_(&self, ev: Drop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = ev;
        Ok(())
    }

    fn selection(&self, ev: Selection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.selection.take();
        if let Some(offer) = self.offer.get()
            && offer.id == ev.id
        {
            self.selection.set(Some(offer));
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlDataDevice = WlDataDevice;
    version = self.version;
}

impl UsrObject for UsrWlDataDevice {
    fn destroy(&self) {
        if let Some(offer) = self.offer.take() {
            self.con.remove_obj(&*offer);
        }
        self.con.request(Release { self_id: self.id });
    }

    fn break_loops(&self) {
        self.selection.take();
        self.offer.take();
    }
}
