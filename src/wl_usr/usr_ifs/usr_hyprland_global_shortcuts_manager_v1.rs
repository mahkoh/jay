use crate::object::Version;
use crate::wire::HyprlandGlobalShortcutsManagerV1Id;
use crate::wire::hyprland_global_shortcuts_manager_v1::Destroy;
use crate::wire::hyprland_global_shortcuts_manager_v1::HyprlandGlobalShortcutsManagerV1EventHandler;
use crate::wire::hyprland_global_shortcuts_manager_v1::RegisterShortcut;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_hyprland_global_shortcut_v1::UsrHyprlandGlobalShortcutV1;
use crate::wl_usr::usr_object::UsrObject;
use std::rc::Rc;

pub struct UsrHyprlandGlobalShortcutsManagerV1 {
    pub id: HyprlandGlobalShortcutsManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrHyprlandGlobalShortcutsManagerV1 {
    pub fn register_shortcut(
        &self,
        id: &str,
        app_id: &str,
        description: &str,
        trigger_description: &str,
    ) -> Rc<UsrHyprlandGlobalShortcutV1> {
        let obj = Rc::new(UsrHyprlandGlobalShortcutV1 {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.add_object(obj.clone());
        self.con.request(RegisterShortcut {
            self_id: self.id,
            shortcut: obj.id,
            id,
            app_id,
            description,
            trigger_description,
        });
        obj
    }
}

impl HyprlandGlobalShortcutsManagerV1EventHandler for UsrHyprlandGlobalShortcutsManagerV1 {
    type Error = !;
}

usr_object_base! {
    self = UsrHyprlandGlobalShortcutsManagerV1 = HyprlandGlobalShortcutsManagerV1;
    version = self.version;
}

impl UsrObject for UsrHyprlandGlobalShortcutsManagerV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
