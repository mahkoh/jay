use crate::object::Version;
use crate::wire::ZwlrLayerShellV1Id;
use crate::wire::zwlr_layer_shell_v1::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_output::UsrWlOutput;
use crate::wl_usr::usr_ifs::usr_wl_surface::UsrWlSurface;
use crate::wl_usr::usr_ifs::usr_wlr_layer_surface::UsrWlrLayerSurface;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWlrLayerShell {
    pub id: ZwlrLayerShellV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlrLayerShell {
    pub fn get_layer_surface(
        &self,
        surface: &UsrWlSurface,
        output: &UsrWlOutput,
        layer: u32,
    ) -> Rc<UsrWlrLayerSurface> {
        let sfc = Rc::new(UsrWlrLayerSurface {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.add_object(sfc.clone());
        self.con.request(GetLayerSurface {
            self_id: self.id,
            id: sfc.id,
            surface: surface.id,
            output: output.id,
            layer,
            namespace: "",
        });
        sfc
    }
}

impl ZwlrLayerShellV1EventHandler for UsrWlrLayerShell {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWlrLayerShell = ZwlrLayerShellV1;
    version = self.version;
}

impl UsrObject for UsrWlrLayerShell {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id })
    }
}
