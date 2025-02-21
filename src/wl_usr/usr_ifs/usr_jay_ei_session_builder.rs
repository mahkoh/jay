use {
    crate::{
        object::Version,
        wire::{
            JayEiSessionBuilderId,
            jay_ei_session_builder::{Commit, JayEiSessionBuilderEventHandler, SetAppId},
        },
        wl_usr::{UsrCon, usr_ifs::usr_jay_ei_session::UsrJayEiSession, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrJayEiSessionBuilder {
    pub id: JayEiSessionBuilderId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrJayEiSessionBuilder {
    pub fn set_app_id(&self, app_id: &str) {
        self.con.request(SetAppId {
            self_id: self.id,
            app_id,
        });
    }

    pub fn commit(&self) -> Rc<UsrJayEiSession> {
        let obj = Rc::new(UsrJayEiSession {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.add_object(obj.clone());
        self.con.request(Commit {
            self_id: self.id,
            id: obj.id,
        });
        self.con.remove_obj(self);
        obj
    }
}

impl JayEiSessionBuilderEventHandler for UsrJayEiSessionBuilder {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrJayEiSessionBuilder = JayEiSessionBuilder;
    version = self.version;
}

impl UsrObject for UsrJayEiSessionBuilder {
    fn destroy(&self) {
        // nothing
    }
}
