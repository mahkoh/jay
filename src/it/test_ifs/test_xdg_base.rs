use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_xdg_surface::TestXdgSurface,
            test_object::TestObject, test_transport::TestTransport, testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{xdg_wm_base::*, WlSurfaceId, XdgWmBaseId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestXdgWmBase {
    pub id: XdgWmBaseId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestXdgWmBase {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub async fn create_xdg_surface(
        &self,
        surface: WlSurfaceId,
    ) -> Result<Rc<TestXdgSurface>, TestError> {
        let id = self.tran.id();
        self.tran.send(GetXdgSurface {
            self_id: self.id,
            id,
            surface,
        })?;
        self.tran.sync().await;
        let client = self.tran.get_client()?;
        let server = client.lookup(id)?;
        let xdg = Rc::new(TestXdgSurface {
            id,
            tran: self.tran.clone(),
            server,
            destroyed: Cell::new(false),
            last_serial: Cell::new(0),
        });
        self.tran.add_obj(xdg.clone())?;
        Ok(xdg)
    }

    fn handle_ping(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Ping::parse_full(parser)?;
        Ok(())
    }
}

test_object! {
    TestXdgWmBase, XdgWmBase;

    PING => handle_ping,
}

impl TestObject for TestXdgWmBase {}

impl Drop for TestXdgWmBase {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}
