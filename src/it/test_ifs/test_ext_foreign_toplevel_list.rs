use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::test_ext_foreign_toplevel_handle::TestExtForeignToplevelHandle,
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{ExtForeignToplevelListV1Id, ext_foreign_toplevel_list_v1::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestExtForeignToplevelList {
    pub id: ExtForeignToplevelListV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub toplevels: RefCell<Vec<Rc<TestExtForeignToplevelHandle>>>,
}

impl TestExtForeignToplevelList {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
            toplevels: RefCell::new(vec![]),
        }
    }

    #[expect(dead_code)]
    pub fn stop(&self) -> TestResult {
        self.tran.send(Stop { self_id: self.id })?;
        Ok(())
    }

    pub fn destroy(&self) -> TestResult {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    fn handle_toplevel(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Toplevel::parse_full(parser)?;
        let tl = Rc::new(TestExtForeignToplevelHandle {
            id: ev.toplevel,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            closed: Cell::new(false),
            title: Cell::new(None),
            app_id: Cell::new(None),
            identifier: Cell::new(None),
        });
        self.tran.add_obj(tl.clone())?;
        self.toplevels.borrow_mut().push(tl);
        Ok(())
    }

    fn handle_finished(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Finished::parse_full(parser)?;
        self.destroy()?;
        Ok(())
    }
}

test_object! {
    TestExtForeignToplevelList, ExtForeignToplevelListV1;

    TOPLEVEL => handle_toplevel,
    FINISHED => handle_finished,
}

impl TestObject for TestExtForeignToplevelList {}
