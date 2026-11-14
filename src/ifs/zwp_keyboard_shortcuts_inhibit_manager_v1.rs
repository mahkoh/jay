use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::NodeBase;
use crate::wire::ZwpKeyboardShortcutsInhibitManagerV1Id;
use crate::wire::zwp_keyboard_shortcuts_inhibit_manager_v1::*;
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
    ) -> Result<(), ZwpKeyboardShortcutsInhibitManagerV1Error> {
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
        let seat: Rc<crate::ifs::wl_seat::WlSeat> = self.client.lookup(req.seat)?;
        let seat_global = seat.global.clone();
        let seat_id = seat_global.id();
        if surface
            .node_seat_state()
            .shortcuts_inhibitors
            .contains(&seat_id)
        {
            return Err(ZwpKeyboardShortcutsInhibitManagerV1Error::AlreadyInhibited);
        }
        let inhibitor = Rc::new(ZwpKeyboardShortcutsInhibitorV1::new(
            req.id,
            self.client.state.shortcuts_inhibitor_ids.next(),
            &self.client,
            &surface,
            &seat_global,
            self.version,
        ));
        track!(self.client, inhibitor);
        self.client.add_client_obj(&inhibitor);
        inhibitor.install();
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ZwpKeyboardShortcutsInhibitManagerV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("The surface already has a shortcuts inhibitor for this seat")]
    AlreadyInhibited,
}
