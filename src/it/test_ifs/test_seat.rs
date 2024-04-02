use {
    crate::{
        ifs::wl_seat::WlSeat,
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{test_keyboard::TestKeyboard, test_pointer::TestPointer},
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, once::Once},
        wire::{wl_seat::*, WlSeatId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSeat {
    pub id: WlSeatId,
    pub tran: Rc<TestTransport>,
    pub server: CloneCell<Option<Rc<WlSeat>>>,
    pub destroyed: Once,
    pub caps: Cell<u32>,
    pub name: CloneCell<Option<Rc<String>>>,
}

impl TestSeat {
    pub fn destroy(&self) -> Result<(), TestError> {
        if self.destroyed.set() {
            self.tran.send(Release { self_id: self.id })?;
        }
        Ok(())
    }

    pub async fn get_keyboard(&self) -> TestResult<Rc<TestKeyboard>> {
        let id = self.tran.id();
        self.tran.send(GetKeyboard {
            self_id: self.id,
            id,
        })?;
        let kb = Rc::new(TestKeyboard {
            id,
            tran: self.tran.clone(),
            server: Default::default(),
            destroyed: Default::default(),
            enter: Default::default(),
            leave: Default::default(),
        });
        self.tran.add_obj(kb.clone())?;
        self.tran.sync().await;
        let server = self.tran.get_server_obj(id)?;
        kb.server.set(Some(server));
        Ok(kb)
    }

    pub async fn get_pointer(&self) -> TestResult<Rc<TestPointer>> {
        let id = self.tran.id();
        self.tran.send(GetPointer {
            self_id: self.id,
            id,
        })?;
        let pointer = Rc::new(TestPointer {
            id,
            tran: self.tran.clone(),
            server: Default::default(),
            destroyed: Default::default(),
            leave: Rc::new(Default::default()),
            enter: Rc::new(Default::default()),
            motion: Rc::new(Default::default()),
            button: Rc::new(Default::default()),
            axis_relative_direction: Rc::new(Default::default()),
        });
        self.tran.add_obj(pointer.clone())?;
        self.tran.sync().await;
        let server = self.tran.get_server_obj(id)?;
        pointer.server.set(Some(server));
        Ok(pointer)
    }

    fn handle_capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Capabilities::parse_full(parser)?;
        self.caps.set(ev.capabilities);
        Ok(())
    }

    fn handle_name(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Name::parse_full(parser)?;
        self.name.set(Some(Rc::new(ev.name.to_string())));
        Ok(())
    }
}

impl Drop for TestSeat {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSeat, WlSeat;

    CAPABILITIES => handle_capabilities,
    NAME => handle_name,
}

impl TestObject for TestSeat {}
