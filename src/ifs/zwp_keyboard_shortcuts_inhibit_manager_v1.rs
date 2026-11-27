use crate::client::CAP_KEYBOARD_SHORTCUTS_INHIBIT;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::NodeBase;
use crate::wire::ZwpKeyboardShortcutsInhibitManagerV1Id;
use crate::wire::zwp_keyboard_shortcuts_inhibit_manager_v1::*;
use crate::wire::zwp_keyboard_shortcuts_inhibitor_v1::Active;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Global)]
pub struct ZwpKeyboardShortcutsInhibitManagerV1Global {
    name: GlobalName,
}

impl ZwpKeyboardShortcutsInhibitManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpKeyboardShortcutsInhibitManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), !> {
        let obj = Rc::new(ZwpKeyboardShortcutsInhibitManagerV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for ZwpKeyboardShortcutsInhibitManagerV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_KEYBOARD_SHORTCUTS_INHIBIT
    }
}

#[derive(Object)]
pub struct ZwpKeyboardShortcutsInhibitManagerV1 {
    id: ZwpKeyboardShortcutsInhibitManagerV1Id,
    client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl ZwpKeyboardShortcutsInhibitManagerV1RequestHandler for ZwpKeyboardShortcutsInhibitManagerV1 {
    type Error = ZwpKeyboardShortcutsInhibitManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn inhibit_shortcuts(&self, req: InhibitShortcuts, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let seat = self.client.lookup(req.seat)?.global.id();
        let inhibitors = &surface.node_seat_state().shortcuts_inhibitors;
        if inhibitors.insert(seat, ()).is_some() {
            return Err(ZwpKeyboardShortcutsInhibitManagerV1Error::AlreadyInhibited);
        }
        let inhibitor = Rc::new(ZwpKeyboardShortcutsInhibitorV1 {
            id: req.id,
            client: self.client.clone(),
            surface,
            seat,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, inhibitor);
        self.client.add_client_obj(&inhibitor);
        self.client.event(Active { self_id: req.id });
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ZwpKeyboardShortcutsInhibitManagerV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("The surface already inhibits shortcuts for this seat")]
    AlreadyInhibited,
}
