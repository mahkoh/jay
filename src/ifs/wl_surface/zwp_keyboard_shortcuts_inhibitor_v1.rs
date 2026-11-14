use crate::client::Client;
use crate::control_center::CCI_INPUT;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::NodeBase;
use crate::wire::ZwpKeyboardShortcutsInhibitorV1Id;
use crate::wire::zwp_keyboard_shortcuts_inhibitor_v1::*;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;

linear_ids!(ShortcutsInhibitorIds, ShortcutsInhibitorId, u64);

#[derive(Object)]
#[break_loops]
pub struct ZwpKeyboardShortcutsInhibitorV1 {
    pub id: ZwpKeyboardShortcutsInhibitorV1Id,
    pub inhibit_id: ShortcutsInhibitorId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub seat: Rc<WlSeatGlobal>,
    pub active: Cell<bool>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpKeyboardShortcutsInhibitorV1 {
    pub fn new(
        id: ZwpKeyboardShortcutsInhibitorV1Id,
        inhibit_id: ShortcutsInhibitorId,
        client: &Rc<Client>,
        surface: &Rc<WlSurface>,
        seat: &Rc<WlSeatGlobal>,
        version: Version,
    ) -> Self {
        Self {
            id,
            inhibit_id,
            client: client.clone(),
            surface: surface.clone(),
            seat: seat.clone(),
            active: Cell::new(false),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) {
        self.client
            .state
            .shortcuts_inhibitors
            .set((self.client.id, self.id), self.clone());
        self.surface
            .node_seat_state()
            .shortcuts_inhibitors
            .insert(self.seat.id(), self.clone());
        self.client.state.trigger_cci(CCI_INPUT);
        if self.seat.get_keyboard_node().node_id() == self.surface.node_id() {
            self.activate();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    pub fn activate(&self) {
        self.activate_(true);
    }

    pub(crate) fn activate_(&self, update_seat: bool) {
        if !self.client.state.shortcuts_inhibit_enabled.get() {
            return;
        }
        if !self.active.replace(true) {
            self.client.event(Active { self_id: self.id });
            self.client.state.trigger_cci(CCI_INPUT);
            if update_seat {
                self.seat.update_window_management_enabled();
            }
        }
    }

    pub fn deactivate(&self) {
        self.deactivate_(true);
    }

    pub(crate) fn deactivate_(&self, update_seat: bool) {
        if self.active.replace(false) {
            self.client.event(Inactive { self_id: self.id });
            self.client.state.trigger_cci(CCI_INPUT);
            if update_seat {
                self.seat.update_window_management_enabled();
            }
        }
    }

    fn detach(&self) {
        self.surface
            .node_seat_state()
            .shortcuts_inhibitors
            .remove(&self.seat.id());
        self.client
            .state
            .shortcuts_inhibitors
            .remove(&(self.client.id, self.id));
        self.deactivate();
        self.client.state.trigger_cci(CCI_INPUT);
    }
}

impl ZwpKeyboardShortcutsInhibitorV1RequestHandler for ZwpKeyboardShortcutsInhibitorV1 {
    type Error = !;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for ZwpKeyboardShortcutsInhibitorV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
