use {
    crate::{
        object::Version,
        wire::{wp_fractional_scale_manager_v1::*, WpFractionalScaleManagerV1Id},
        wl_usr::{
            usr_ifs::{
                usr_wl_surface::UsrWlSurface, usr_wp_fractional_scale::UsrWpFractionalScale,
            },
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWpFractionalScaleManager {
    pub id: WpFractionalScaleManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWpFractionalScaleManager {
    pub fn get_fractional_scale(&self, surface: &UsrWlSurface) -> Rc<UsrWpFractionalScale> {
        let fs = Rc::new(UsrWpFractionalScale {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.add_object(fs.clone());
        self.con.request(GetFractionalScale {
            self_id: self.id,
            id: fs.id,
            surface: surface.id,
        });
        fs
    }
}

impl WpFractionalScaleManagerV1EventHandler for UsrWpFractionalScaleManager {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWpFractionalScaleManager = WpFractionalScaleManagerV1;
    version = self.version;
}

impl UsrObject for UsrWpFractionalScaleManager {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
