use {
    crate::{
        globals::GlobalName,
        ifs::wl_seat::WlSeatGlobal,
        it::{
            test_error::TestError,
            test_ifs::{
                test_compositor::TestCompositor, test_cursor_shape_manager::TestCursorShapeManager,
                test_data_device_manager::TestDataDeviceManager,
                test_ext_foreign_toplevel_list::TestExtForeignToplevelList,
                test_jay_compositor::TestJayCompositor, test_shm::TestShm,
                test_single_pixel_buffer_manager::TestSinglePixelBufferManager,
                test_subcompositor::TestSubcompositor, test_viewporter::TestViewporter,
                test_xdg_activation::TestXdgActivation, test_xdg_base::TestXdgWmBase,
            },
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, copyhashmap::CopyHashMap},
        wire::{wl_registry::*, WlRegistryId, WlSeat},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestGlobal {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

pub struct TestRegistrySingletons {
    pub jay_compositor: u32,
    pub wl_compositor: u32,
    pub wl_subcompositor: u32,
    pub wl_shm: u32,
    pub xdg_wm_base: u32,
    pub wp_single_pixel_buffer_manager_v1: u32,
    pub wp_viewporter: u32,
    pub xdg_activation_v1: u32,
    pub ext_foreign_toplevel_list_v1: u32,
    pub wl_data_device_manager: u32,
    pub wp_cursor_shape_manager_v1: u32,
}

pub struct TestRegistry {
    pub id: WlRegistryId,
    pub tran: Rc<TestTransport>,
    pub globals: CopyHashMap<u32, Rc<TestGlobal>>,
    pub singletons: CloneCell<Option<Rc<TestRegistrySingletons>>>,
    pub jay_compositor: CloneCell<Option<Rc<TestJayCompositor>>>,
    pub compositor: CloneCell<Option<Rc<TestCompositor>>>,
    pub subcompositor: CloneCell<Option<Rc<TestSubcompositor>>>,
    pub shm: CloneCell<Option<Rc<TestShm>>>,
    pub spbm: CloneCell<Option<Rc<TestSinglePixelBufferManager>>>,
    pub viewporter: CloneCell<Option<Rc<TestViewporter>>>,
    pub xdg: CloneCell<Option<Rc<TestXdgWmBase>>>,
    pub activation: CloneCell<Option<Rc<TestXdgActivation>>>,
    pub foreign_toplevel_list: CloneCell<Option<Rc<TestExtForeignToplevelList>>>,
    pub data_device_manager: CloneCell<Option<Rc<TestDataDeviceManager>>>,
    pub cursor_shape_manager: CloneCell<Option<Rc<TestCursorShapeManager>>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
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
        self.tran.sync().await;
        singleton!(self.singletons);
        macro_rules! singleton {
            ($($name:ident,)*) => {{
                $(
                    let mut $name = 0;
                )*
                for global in self.globals.lock().values() {
                    match global.interface.as_str() {
                        $(
                            stringify!($name) => $name = global.name,
                        )*
                        _ => {}
                    }
                }
                Rc::new(TestRegistrySingletons {
                    $(
                        $name: {
                            if $name == 0 {
                                bail!("Compositor did not send {} singleton", stringify!($name));
                            }
                            $name
                        },
                    )*
                })
            }}
        }
        let singletons = singleton! {
            jay_compositor,
            wl_compositor,
            wl_subcompositor,
            wl_shm,
            xdg_wm_base,
            wp_single_pixel_buffer_manager_v1,
            wp_viewporter,
            xdg_activation_v1,
            ext_foreign_toplevel_list_v1,
            wl_data_device_manager,
            wp_cursor_shape_manager_v1,
        };
        self.singletons.set(Some(singletons.clone()));
        Ok(singletons)
    }

    pub async fn get_jay_compositor(&self) -> Result<Rc<TestJayCompositor>, TestError> {
        singleton!(self.jay_compositor);
        let singletons = self.get_singletons().await?;
        singleton!(self.jay_compositor);
        let jc = Rc::new(TestJayCompositor {
            id: self.tran.id(),
            tran: self.tran.clone(),
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
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.bind(&jc, singletons.wl_compositor, 6)?;
        self.compositor.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_subcompositor(&self) -> Result<Rc<TestSubcompositor>, TestError> {
        singleton!(self.subcompositor);
        let singletons = self.get_singletons().await?;
        singleton!(self.subcompositor);
        let jc = Rc::new(TestSubcompositor {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.bind(&jc, singletons.wl_subcompositor, 1)?;
        self.subcompositor.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_shm(&self) -> Result<Rc<TestShm>, TestError> {
        singleton!(self.shm);
        let singletons = self.get_singletons().await?;
        singleton!(self.shm);
        let jc = Rc::new(TestShm {
            id: self.tran.id(),
            tran: self.tran.clone(),
            formats: Default::default(),
            formats_awaited: Cell::new(false),
        });
        self.bind(&jc, singletons.wl_shm, 1)?;
        self.shm.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_spbm(&self) -> Result<Rc<TestSinglePixelBufferManager>, TestError> {
        singleton!(self.spbm);
        let singletons = self.get_singletons().await?;
        singleton!(self.spbm);
        let jc = Rc::new(TestSinglePixelBufferManager {
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.bind(&jc, singletons.wp_single_pixel_buffer_manager_v1, 1)?;
        self.spbm.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_viewporter(&self) -> Result<Rc<TestViewporter>, TestError> {
        singleton!(self.viewporter);
        let singletons = self.get_singletons().await?;
        singleton!(self.viewporter);
        let jc = Rc::new(TestViewporter {
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.bind(&jc, singletons.wp_viewporter, 1)?;
        self.viewporter.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_activation(&self) -> Result<Rc<TestXdgActivation>, TestError> {
        singleton!(self.activation);
        let singletons = self.get_singletons().await?;
        singleton!(self.activation);
        let jc = Rc::new(TestXdgActivation {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.bind(&jc, singletons.xdg_activation_v1, 1)?;
        self.activation.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_xdg(&self) -> Result<Rc<TestXdgWmBase>, TestError> {
        singleton!(self.xdg);
        let singletons = self.get_singletons().await?;
        singleton!(self.xdg);
        let jc = Rc::new(TestXdgWmBase {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.bind(&jc, singletons.xdg_wm_base, 6)?;
        self.xdg.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_foreign_toplevel_list(
        &self,
    ) -> Result<Rc<TestExtForeignToplevelList>, TestError> {
        singleton!(self.foreign_toplevel_list);
        let singletons = self.get_singletons().await?;
        singleton!(self.foreign_toplevel_list);
        let jc = Rc::new(TestExtForeignToplevelList {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            toplevels: RefCell::new(vec![]),
        });
        self.bind(&jc, singletons.ext_foreign_toplevel_list_v1, 1)?;
        self.foreign_toplevel_list.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_data_device_manager(&self) -> Result<Rc<TestDataDeviceManager>, TestError> {
        singleton!(self.data_device_manager);
        let singletons = self.get_singletons().await?;
        singleton!(self.data_device_manager);
        let jc = Rc::new(TestDataDeviceManager {
            id: self.tran.id(),
            tran: self.tran.clone(),
        });
        self.bind(&jc, singletons.wl_data_device_manager, 3)?;
        self.data_device_manager.set(Some(jc.clone()));
        Ok(jc)
    }

    pub async fn get_cursor_shape_manager(&self) -> Result<Rc<TestCursorShapeManager>, TestError> {
        singleton!(self.cursor_shape_manager);
        let singletons = self.get_singletons().await?;
        singleton!(self.cursor_shape_manager);
        let jc = Rc::new(TestCursorShapeManager {
            id: self.tran.id(),
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
        });
        self.bind(&jc, singletons.wp_cursor_shape_manager_v1, 1)?;
        self.cursor_shape_manager.set(Some(jc.clone()));
        Ok(jc)
    }

    pub fn bind<O: TestObject>(
        &self,
        obj: &Rc<O>,
        name: u32,
        version: u32,
    ) -> Result<(), TestError> {
        self.tran.send(Bind {
            self_id: self.id,
            name,
            interface: obj.interface().name(),
            version,
            id: obj.id().into(),
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(())
    }

    fn handle_global(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Global::parse_full(parser)?;
        let global = Rc::new(TestGlobal {
            name: ev.name,
            interface: ev.interface.to_string(),
            version: ev.version,
        });
        let prev = self.globals.set(ev.name, global.clone());
        let name = GlobalName::from_raw(ev.name);
        if ev.interface == WlSeat.name() {
            let seat = match self.tran.run.state.globals.seats.get(&name) {
                Some(s) => s,
                _ => bail!("Compositor sent seat global but seat does not exist"),
            };
            self.seats.set(GlobalName::from_raw(ev.name), seat);
        }
        if prev.is_some() {
            bail!("Compositor sent global {} multiple times", ev.name);
        }
        Ok(())
    }

    fn handle_global_remove(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = GlobalRemove::parse_full(parser)?;
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

test_object! {
    TestRegistry, WlRegistry;

    GLOBAL => handle_global,
    GLOBAL_REMOVE => handle_global_remove,
}

impl TestObject for TestRegistry {}
