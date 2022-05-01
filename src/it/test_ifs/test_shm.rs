use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_shm_pool::TestShmPool, test_mem::TestMem,
            test_object::TestObject, test_transport::TestTransport, testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, copyhashmap::CopyHashMap},
        wire::{wl_shm::*, WlShmId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestShm {
    pub id: WlShmId,
    pub transport: Rc<TestTransport>,
    pub formats: CopyHashMap<u32, ()>,
    pub formats_awaited: Cell<bool>,
}

impl TestShm {
    pub async fn formats(&self) -> &CopyHashMap<u32, ()> {
        if !self.formats_awaited.replace(true) {
            self.transport.sync().await;
        }
        &self.formats
    }

    pub fn create_pool(&self, size: usize) -> Result<Rc<TestShmPool>, TestError> {
        let mem = TestMem::new(size)?;
        let pool = Rc::new(TestShmPool {
            id: self.transport.id(),
            transport: self.transport.clone(),
            mem: CloneCell::new(mem.clone()),
            destroyed: Cell::new(false),
        });
        self.transport.send(CreatePool {
            self_id: self.id,
            id: pool.id,
            fd: mem.fd.clone(),
            size: size as _,
        });
        self.transport.add_obj(pool.clone())?;
        Ok(pool)
    }

    fn handle_format(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Format::parse_full(parser)?;
        self.formats.set(ev.format, ());
        Ok(())
    }
}

test_object! {
    TestShm, WlShm;

    FORMAT => handle_format,
}

impl TestObject for TestShm {}
