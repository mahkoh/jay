use {
    crate::{
        gfx_api::SyncFile,
        object::Version,
        wire::{JaySyncFileSurfaceId, jay_sync_file_surface::*},
        wl_usr::{
            UsrCon, usr_ifs::usr_jay_sync_file_release::UsrJaySyncFileRelease,
            usr_object::UsrObject,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJaySyncFileSurface {
    pub id: JaySyncFileSurfaceId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrJaySyncFileSurface {
    #[expect(dead_code)]
    pub fn set_acquire(&self, sf: Option<&SyncFile>) {
        match sf {
            None => {
                self.con.request(SetAcquireImmediate { self_id: self.id });
            }
            Some(sf) => {
                self.con.request(SetAcquireAsync {
                    self_id: self.id,
                    sync_file: sf.0.clone(),
                });
            }
        }
    }

    #[expect(dead_code)]
    pub fn get_release(&self) -> Rc<UsrJaySyncFileRelease> {
        let obj = Rc::new(UsrJaySyncFileRelease {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(GetRelease {
            self_id: self.id,
            release: obj.id,
        });
        self.con.add_object(obj.clone());
        obj
    }
}

impl JaySyncFileSurfaceEventHandler for UsrJaySyncFileSurface {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrJaySyncFileSurface = JaySyncFileSurface;
    version = self.version;
}

impl UsrObject for UsrJaySyncFileSurface {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
