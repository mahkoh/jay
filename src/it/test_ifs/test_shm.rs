use {
    crate::{
        format::ARGB8888,
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{test_shm_buffer::TestShmBuffer, test_shm_pool::TestShmPool},
            test_mem::TestMem,
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, copyhashmap::CopyHashMap},
        wire::{wl_shm::*, WlShmId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestShm {
    pub id: WlShmId,
    pub tran: Rc<TestTransport>,
    pub formats: CopyHashMap<u32, ()>,
    pub formats_awaited: Cell<bool>,
}

impl TestShm {
    pub async fn formats(&self) -> &CopyHashMap<u32, ()> {
        if !self.formats_awaited.replace(true) {
            self.tran.sync().await;
        }
        &self.formats
    }

    #[allow(dead_code)]
    pub fn create_pool(&self, size: usize) -> Result<Rc<TestShmPool>, TestError> {
        let mem = TestMem::new(size)?;
        let pool = Rc::new(TestShmPool {
            id: self.tran.id(),
            tran: self.tran.clone(),
            mem: CloneCell::new(mem.clone()),
            destroyed: Cell::new(false),
        });
        self.tran.send(CreatePool {
            self_id: self.id,
            id: pool.id,
            fd: mem.fd.clone(),
            size: size as _,
        })?;
        self.tran.add_obj(pool.clone())?;
        Ok(pool)
    }

    #[allow(dead_code)]
    pub fn create_buffer(&self, width: i32, height: i32) -> TestResult<Rc<TestShmBuffer>> {
        let pool = self.create_pool((width * height * 4) as _)?;
        pool.create_buffer(0, width, height, width * 4, ARGB8888)
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
