use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::it::test_error::TestError;
use crate::it::test_ifs::test_xdg_toplevel::TestXdgToplevel;
use crate::it::test_ifs::test_xdg_toplevel::TestXdgToplevelCore;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::XdgSurfaceId;
use crate::wire::xdg_surface::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestXdgSurface {
    pub id: XdgSurfaceId,
    pub tran: Rc<TestTransport>,
    pub _server: Rc<XdgSurface>,
    pub destroyed: Cell<bool>,
    pub last_serial: Cell<Option<u32>>,
}

impl TestXdgSurface {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub async fn create_toplevel(&self) -> Result<Rc<TestXdgToplevel>, TestError> {
        let id = self.tran.id();
        self.tran.send(GetToplevel {
            self_id: self.id,
            id,
        })?;
        let core = Rc::new(TestXdgToplevelCore {
            id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            configured: Cell::new(false),
            configured_waiter: Cell::new(None),
            width: Cell::new(0),
            height: Cell::new(0),
            states: Default::default(),
            close_requested: Cell::new(false),
        });
        self.tran.add_obj(core.clone())?;
        self.tran.sync().await;
        let client = self.tran.get_client()?;
        let server = client.lookup(id)?;
        let tl = Rc::new(TestXdgToplevel { core, server });
        Ok(tl)
    }

    pub fn ack_configure(&self, serial: u32) -> Result<(), TestError> {
        self.tran.send(AckConfigure {
            self_id: self.id,
            serial,
        })?;
        Ok(())
    }

    fn handle_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Configure::parse_full(parser)?;
        self.last_serial.set(Some(ev.serial));
        Ok(())
    }
}

impl Drop for TestXdgSurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestXdgSurface, XdgSurface;

    CONFIGURE => handle_configure,
}

impl TestObject for TestXdgSurface {}
