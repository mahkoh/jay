use crate::client::Client;
use crate::ifs::wl_seat::SeatId;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::wire::ZwpKeyboardShortcutsInhibitorV1Id;
use crate::wire::zwp_keyboard_shortcuts_inhibitor_v1::*;
use jay_proc::Object;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct ZwpKeyboardShortcutsInhibitorV1 {
    pub id: ZwpKeyboardShortcutsInhibitorV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub seat: SeatId,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpKeyboardShortcutsInhibitorV1 {
    fn uninstall(&self) {
        self.surface
            .seat_state
            .shortcuts_inhibitors
            .remove(&self.seat);
    }
}

impl ZwpKeyboardShortcutsInhibitorV1RequestHandler for ZwpKeyboardShortcutsInhibitorV1 {
    type Error = !;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        self.uninstall();
        Ok(())
    }
}

impl BreakLoops for ZwpKeyboardShortcutsInhibitorV1 {
    fn break_loops(self: Rc<Self>) {
        self.uninstall();
    }
}
