use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::HyprlandGlobalShortcutV1Id;
use crate::wire::hyprland_global_shortcut_v1::Destroy;
use crate::wire::hyprland_global_shortcut_v1::HyprlandGlobalShortcutV1EventHandler;
use crate::wire::hyprland_global_shortcut_v1::Pressed;
use crate::wire::hyprland_global_shortcut_v1::Released;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::rc::Rc;

pub struct UsrHyprlandGlobalShortcutV1 {
    pub id: HyprlandGlobalShortcutV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrHyprlandGlobalShortcutV1Owner>>>,
    pub version: Version,
}

pub trait UsrHyprlandGlobalShortcutV1Owner {
    fn pressed(&self, timestamp_ms: u64) {
        let _ = timestamp_ms;
    }

    fn released(&self, timestamp_ms: u64) {
        let _ = timestamp_ms;
    }
}

fn timestamp_ms(tv_sec_hi: u32, tv_sec_lo: u32, tv_nsec: u32) -> u64 {
    let secs = ((tv_sec_hi as u64) << 32) | (tv_sec_lo as u64);
    secs.wrapping_mul(1000)
        .wrapping_add(tv_nsec as u64 / 1_000_000)
}

impl HyprlandGlobalShortcutV1EventHandler for UsrHyprlandGlobalShortcutV1 {
    type Error = !;

    fn pressed(&self, ev: Pressed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.pressed(timestamp_ms(ev.tv_sec_hi, ev.tv_sec_lo, ev.tv_nsec));
        }
        Ok(())
    }

    fn released(&self, ev: Released, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(owner) = self.owner.get() {
            owner.released(timestamp_ms(ev.tv_sec_hi, ev.tv_sec_lo, ev.tv_nsec));
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrHyprlandGlobalShortcutV1 = HyprlandGlobalShortcutV1;
    version = self.version;
}

impl UsrObject for UsrHyprlandGlobalShortcutV1 {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
