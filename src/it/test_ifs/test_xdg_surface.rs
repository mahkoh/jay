use crate::client::Client;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_xdg_toplevel::TestXdgToplevel;
use crate::it::test_ifs::test_xdg_toplevel::TestXdgToplevelCore;
use crate::wire::XdgSurfaceId;
use crate::wire::xdg_surface::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestXdgSurface {
    pub id: XdgSurfaceId,
    pub client: Rc<Client>,
    pub last_serial: Cell<Option<u32>>,
}

impl TestXdgSurface {
    pub async fn create_toplevel(&self) -> Result<Rc<TestXdgToplevel>, TestError> {
        let client = &self.client;
        let id = client.send_xdg_surface_get_toplevel(self.id);
        let core = Rc::new(TestXdgToplevelCore {
            id,
            client: client.clone(),
            configured: Cell::new(false),
            configured_waiter: Cell::new(None),
            width: Cell::new(0),
            height: Cell::new(0),
            states: Default::default(),
            close_requested: Cell::new(false),
        });
        client.set_synthetic_event_handler(id, &core);
        self.client.sync().await;
        let server = client.lookup(id)?;
        let tl = Rc::new(TestXdgToplevel { core, server });
        Ok(tl)
    }

    pub fn ack_configure(&self, serial: u32) {
        self.client.send_xdg_surface_ack_configure(self.id, serial);
    }
}

synthetic_event_handler!(TestXdgSurface);

impl XdgSurfaceEventHandler for TestXdgSurface {
    type Error = TestErrorError;

    fn configure(&self, ev: Configure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.last_serial.set(Some(ev.serial));
        Ok(())
    }
}

impl TestClient {}
