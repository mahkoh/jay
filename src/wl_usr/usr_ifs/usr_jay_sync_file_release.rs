use crate::gfx_api::SyncFile;
use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::JaySyncFileReleaseId;
use crate::wire::jay_sync_file_release::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct UsrJaySyncFileRelease {
    pub id: JaySyncFileReleaseId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJaySyncFileReleaseOwner>>>,
    pub version: Version,
}

pub trait UsrJaySyncFileReleaseOwner {
    fn release(&self, sync_file: Option<SyncFile>);
}

impl UsrJaySyncFileRelease {
    fn release(&self, sf: Option<Rc<OwnedFd>>) {
        if let Some(owner) = self.owner.get() {
            owner.release(sf.map(SyncFile));
        }
        self.con.remove_obj(self);
    }
}

impl JaySyncFileReleaseEventHandler for UsrJaySyncFileRelease {
    type Error = Infallible;

    fn release_immediate(&self, _ev: ReleaseImmediate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.release(None);
        Ok(())
    }

    fn release_async(&self, ev: ReleaseAsync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.release(Some(ev.sync_file));
        Ok(())
    }
}

usr_object_base! {
    self = UsrJaySyncFileRelease = JaySyncFileRelease;
    version = self.version;
}

impl UsrObject for UsrJaySyncFileRelease {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
