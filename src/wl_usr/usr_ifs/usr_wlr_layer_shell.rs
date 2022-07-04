use std::rc::Rc;
use crate::wire::zwlr_layer_shell_v1::*;
use crate::wire::ZwlrLayerShellV1Id;
use crate::wl_usr::usr_ifs::usr_wl_output::UsrWlOutput;
use crate::wl_usr::usr_ifs::usr_wl_surface::UsrWlSurface;
use crate::wl_usr::usr_ifs::usr_wlr_layer_surface::UsrWlrLayerSurface;
use crate::wl_usr::usr_object::UsrObject;
use crate::wl_usr::UsrCon;

pub struct UsrWlrLayerShell {
    pub id: ZwlrLayerShellV1Id,
    pub con: Rc<UsrCon>,
}

impl UsrWlrLayerShell {
    pub fn get_layer_surface(&self, surface: &UsrWlSurface, output: &UsrWlOutput, layer: u32) -> Rc<UsrWlrLayerSurface> {
        let sfc = Rc::new(UsrWlrLayerSurface {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
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

impl Drop for UsrWlrLayerShell {
    fn drop(&mut self) {
        self.con.request(Destroy {
            self_id: self.id,
        })
    }
}

usr_object_base! {
    UsrWlrLayerShell, ZwlrLayerShellV1;
}

impl UsrObject for UsrWlrLayerShell {

}
