use crate::client::Client;
use crate::ifs::wl_seat::SeatId;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::wire::ZwpKeyboardShortcutsInhibitorV1Id;
use crate::wire::zwp_keyboard_shortcuts_inhibitor_v1::*;
use jay_proc::Object;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct ZwpKeyboardShortcutsInhibitorV1 {
    pub id: ZwpKeyboardShortcutsInhibitorV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub seat_id: SeatId,
    pub active: Cell<bool>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpKeyboardShortcutsInhibitorV1 {
    pub fn new(
        id: ZwpKeyboardShortcutsInhibitorV1Id,
        client: &Rc<Client>,
        surface: &Rc<WlSurface>,
        seat_id: SeatId,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            surface: surface.clone(),
            seat_id,
            active: Cell::new(false),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>, seat: &Rc<WlSeatGlobal>) {
        self.surface
            .shortcuts_inhibitors
            .insert(self.seat_id, self.clone());
        // If the keyboard-focused surface of this seat is our surface, activate immediately.
        if let Some(focused) = seat.get_keyboard_node().node_into_surface() {
            if focused.id == self.surface.id {
                self.activate();
            }
        }
    }

    pub fn activate(self: &Rc<Self>) {
        if !self.active.replace(true) {
            self.client.event(Active { self_id: self.id });
        }
    }

    pub fn deactivate(&self) {
        if self.active.replace(false) {
            self.client.event(Inactive { self_id: self.id });
        }
    }
}

impl ZwpKeyboardShortcutsInhibitorV1RequestHandler for ZwpKeyboardShortcutsInhibitorV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.shortcuts_inhibitors.remove(&self.seat_id);
        self.deactivate();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for ZwpKeyboardShortcutsInhibitorV1 {
    fn break_loops(self: Rc<Self>) {
        self.surface.shortcuts_inhibitors.remove(&self.seat_id);
        self.deactivate();
    }
}
