use {
    crate::{
        it::{
            test_error::TestError,
            test_ifs::{
                test_compositor::TestCompositor, test_jay_compositor::TestJayCompositor,
                test_shm::TestShm,
            },
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, copyhashmap::CopyHashMap},
        wire::{wl_registry::*, WlRegistryId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestGlobal {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

pub struct TestRegistrySingletons {
    pub jay_compositor: u32,
    pub wl_compositor: u32,
    pub wl_shm: u32,
}

pub struct TestRegistry {
    pub id: WlRegistryId,
    pub transport: Rc<TestTransport>,
    pub globals: CopyHashMap<u32, Rc<TestGlobal>>,
    pub singletons: CloneCell<Option<Rc<TestRegistrySingletons>>>,
    pub jay_compositor: CloneCell<Option<Rc<TestJayCompositor>>>,
    pub compositor: CloneCell<Option<Rc<TestCompositor>>>,
    pub shm: CloneCell<Option<Rc<TestShm>>>,
}

macro_rules! singleton {
    ($field:expr) => {
        if let Some(s) = $field.get() {
            return Ok(s);
        }
    };
}

impl TestRegistry {
    pub async fn get_singletons(&self) -> Result<Rc<TestRegistrySingletons>, TestError> {
        singleton!(self.singletons);
        self.transport.sync().await;
        singleton!(self.singletons);
        let mut jay_compositor = 0;
        let mut wl_compositor = 0;
        let mut wl_shm = 0;
        for global in self.globals.lock().values() {
            match global.interface.as_str() {
                "jay_compositor" => jay_compositor = global.name,
                "wl_compositor" => wl_compositor = global.name,
                "wl_shm" => wl_shm = global.name,
                _ => {}
            }
        }
        macro_rules! singleton {
            ($($name:ident,)*) => {
                TestRegistrySingletons {
                    $(
                        $name: {
                            if $name == 0 {
                                bail!("Compositor did not send {} singleton", stringify!($name));
                            }
                            $name
                        },
                    )*
                }
            }
        }
        let singletons = Rc::new(singleton! {
            jay_compositor,
            wl_compositor,
            wl_shm,
        });
        self.singletons.set(Some(singletons.clone()));
        Ok(singletons)
    }

    pub async fn get_jay_compositor(&self) -> Result<Rc<TestJayCompositor>, TestError> {
        singleton!(self.jay_compositor);
        let singletons = self.get_singletons().await?;
        singleton!(self.jay_compositor);
        let jc = Rc::new(TestJayCompositor {
            id: self.transport.id(),
            transport: self.transport.clone(),
            client_id: Default::default(),
        });
        self.bind(&jc, singletons.jay_compositor, 1)?;
        self.jay_compositor.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_compositor(&self) -> Result<Rc<TestCompositor>, TestError> {
        singleton!(self.compositor);
        let singletons = self.get_singletons().await?;
        singleton!(self.compositor);
        let jc = Rc::new(TestCompositor {
            id: self.transport.id(),
            transport: self.transport.clone(),
        });
        self.bind(&jc, singletons.wl_compositor, 4)?;
        self.compositor.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_shm(&self) -> Result<Rc<TestShm>, TestError> {
        singleton!(self.shm);
        let singletons = self.get_singletons().await?;
        singleton!(self.shm);
        let jc = Rc::new(TestShm {
            id: self.transport.id(),
            transport: self.transport.clone(),
            formats: Default::default(),
            formats_awaited: Cell::new(false),
        });
        self.bind(&jc, singletons.wl_shm, 1)?;
        self.shm.set(Some(jc.clone()));
        Ok(jc)
    }

    pub fn bind<O: TestObject>(
        &self,
        obj: &Rc<O>,
        name: u32,
        version: u32,
    ) -> Result<(), TestError> {
        self.transport.send(Bind {
            self_id: self.id,
            name,
            interface: obj.interface().name(),
            version,
            id: obj.id().into(),
        });
        self.transport.add_obj(obj.clone())?;
        Ok(())
    }

    fn handle_global(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Global::parse_full(parser)?;
        let prev = self.globals.set(
            ev.name,
            Rc::new(TestGlobal {
                name: ev.name,
                interface: ev.interface.to_string(),
                version: ev.version,
            }),
        );
        if prev.is_some() {
            self.transport.error(&format!(
                "Compositor sent global {} multiple times",
                ev.name
            ));
        }
        Ok(())
    }

    fn handle_global_remove(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = GlobalRemove::parse_full(parser)?;
        if self.globals.remove(&ev.name).is_none() {
            self.transport.error(&format!(
                "Compositor sent global_remove for {} which does not exist",
                ev.name
            ));
        }
        Ok(())
    }
}

test_object! {
    TestRegistry, WlRegistry;

    GLOBAL => handle_global,
    GLOBAL_REMOVE => handle_global_remove,
}

impl TestObject for TestRegistry {}
