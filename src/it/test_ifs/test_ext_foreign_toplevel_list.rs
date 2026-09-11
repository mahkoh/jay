use crate::client::Client;
use crate::globals::Singleton;
use crate::it::test_client::TestClient;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestErrorError;
use crate::it::test_ifs::test_ext_foreign_toplevel_handle::TestExtForeignToplevelHandle;
use crate::wire::ExtForeignToplevelListV1Id;
use crate::wire::ext_foreign_toplevel_list_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtForeignToplevelList {
    pub id: ExtForeignToplevelListV1Id,
    pub client: Rc<Client>,
    pub toplevels: RefCell<Vec<Rc<TestExtForeignToplevelHandle>>>,
}

impl TestClient {
    pub fn new_foreign_toplevel_list(&self) -> Rc<TestExtForeignToplevelList> {
        let id = self.client.bind(Singleton::ExtForeignToplevelListV1);
        let slf = Rc::new(TestExtForeignToplevelList {
            id,
            client: self.client.clone(),
            toplevels: Default::default(),
        });
        self.client.set_synthetic_event_handler(id, &slf);
        slf
    }
}

impl TestExtForeignToplevelList {
    fn destroy(&self) {
        self.client.request(Destroy { self_id: self.id });
    }
}

synthetic_event_handler!(TestExtForeignToplevelList);

impl ExtForeignToplevelListV1EventHandler for TestExtForeignToplevelList {
    type Error = TestErrorError;

    fn toplevel(&self, ev: Toplevel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = Rc::new(TestExtForeignToplevelHandle {
            id: ev.toplevel,
            client: self.client.clone(),
            closed: Cell::new(false),
            title: Cell::new(None),
            app_id: Cell::new(None),
            identifier: Cell::new(None),
        });
        self.client.set_synthetic_event_handler(ev.toplevel, &tl);
        self.toplevels.borrow_mut().push(tl);
        Ok(())
    }

    fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy();
        Ok(())
    }
}
