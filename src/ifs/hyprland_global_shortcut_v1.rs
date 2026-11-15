use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::wire::HyprlandGlobalShortcutV1Id;
use crate::wire::hyprland_global_shortcut_v1::*;
use jay_config::keyboard::mods::Modifiers;
use jay_config::keyboard::syms::KeySym;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct HyprlandGlobalShortcutV1 {
    pub id: HyprlandGlobalShortcutV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub app_id: String,
    pub shortcut_id: String,
    pub description: String,
    pub preferred_trigger_description: String,
    pub preferred_trigger: Option<(Modifiers, KeySym)>,
    pub pressed_keycode: Cell<Option<u32>>,
}

impl HyprlandGlobalShortcutV1 {
    fn detach(&self) {
        self.client.state.global_shortcuts.remove_shortcut(self);
    }

    pub fn send_pressed(&self, time_usec: u64) {
        let (tv_sec_hi, tv_sec_lo, tv_nsec) = split_time_usec(time_usec);
        self.client.event(Pressed {
            self_id: self.id,
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
        });
    }

    pub fn send_released(&self, time_usec: u64) {
        let (tv_sec_hi, tv_sec_lo, tv_nsec) = split_time_usec(time_usec);
        self.client.event(Released {
            self_id: self.id,
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
        });
    }
}

fn split_time_usec(time_usec: u64) -> (u32, u32, u32) {
    let sec = time_usec / 1_000_000;
    let tv_sec_hi = (sec >> 32) as u32;
    let tv_sec_lo = sec as u32;
    let tv_nsec = ((time_usec % 1_000_000) * 1000) as u32;
    (tv_sec_hi, tv_sec_lo, tv_nsec)
}

impl HyprlandGlobalShortcutV1RequestHandler for HyprlandGlobalShortcutV1 {
    type Error = !;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for HyprlandGlobalShortcutV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
