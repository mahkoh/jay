use crate::client::Client;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::it::test_error::TestErrorError;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::WlRegistryId;
use crate::wire::WlSeat;
use crate::wire::wl_registry::*;
use std::rc::Rc;

pub struct TestGlobal {
    interface: String,
    _version: u32,
}

pub struct TestRegistry {
    pub id: WlRegistryId,
    pub client: Rc<Client>,
    pub globals: CopyHashMap<u32, Rc<TestGlobal>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
}

impl TestRegistry {
    pub fn destroy(&self) {
        self.client.send_wl_fixes_destroy_registry(self.id);
    }
}

synthetic_event_handler!(TestRegistry);

impl WlRegistryEventHandler for TestRegistry {
    type Error = TestErrorError;

    fn global(&self, ev: Global<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let global = Rc::new(TestGlobal {
            interface: ev.interface.to_string(),
            _version: ev.version,
        });
        let prev = self.globals.set(ev.name, global.clone());
        let name = GlobalName::from_raw(ev.name);
        if ev.interface == WlSeat.name() {
            let seat = match self.client.state.globals.seats.get(&name) {
                Some(s) => s,
                _ => bail!("Compositor sent seat global but seat does not exist"),
            };
            self.seats.set(name, seat);
        }
        if prev.is_some() {
            bail!("Compositor sent global {} multiple times", ev.name);
        }
        Ok(())
    }

    fn global_remove(&self, ev: GlobalRemove, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let global = match self.globals.remove(&ev.name) {
            Some(g) => g,
            _ => bail!(
                "Compositor sent global_remove for {} which does not exist",
                ev.name
            ),
        };
        let name = GlobalName::from_raw(ev.name);
        if global.interface == WlSeat.name() {
            self.seats.remove(&name);
        }
        Ok(())
    }
}
