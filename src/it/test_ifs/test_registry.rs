use {
    crate::{
        globals::GlobalName,
        ifs::wl_seat::WlSeatGlobal,
        it::{
            test_error::TestError,
            test_ifs::{
                test_alpha_modifier::TestAlphaModifier, test_compositor::TestCompositor,
                test_content_type_manager::TestContentTypeManager,
                test_cursor_shape_manager::TestCursorShapeManager,
                test_data_control_manager::TestDataControlManager,
                test_data_device_manager::TestDataDeviceManager, test_dmabuf::TestDmabuf,
                test_ext_foreign_toplevel_list::TestExtForeignToplevelList,
                test_input_method_manager::TestInputMethodManager,
                test_jay_compositor::TestJayCompositor, test_shm::TestShm,
                test_single_pixel_buffer_manager::TestSinglePixelBufferManager,
                test_subcompositor::TestSubcompositor, test_syncobj_manager::TestSyncobjManager,
                test_text_input_manager::TestTextInputManager,
                test_toplevel_drag_manager::TestToplevelDragManager,
                test_viewporter::TestViewporter,
                test_virtual_keyboard_manager::TestVirtualKeyboardManager,
                test_wl_fixes::TestWlFixes, test_xdg_activation::TestXdgActivation,
                test_xdg_base::TestXdgWmBase,
            },
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, clonecell::CloneCell, copyhashmap::CopyHashMap},
        wire::{wl_registry::*, WlRegistryId, WlSeat},
    },
    std::rc::Rc,
};

pub struct TestGlobal {
    pub name: u32,
    pub interface: String,
    pub _version: u32,
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
    pub wp_linux_drm_syncobj_manager_v1: u32,
    pub wp_content_type_manager_v1: u32,
    pub zwlr_data_control_manager_v1: u32,
    pub zwp_linux_dmabuf_v1: u32,
    pub xdg_toplevel_drag_manager_v1: u32,
    pub wp_alpha_modifier_v1: u32,
    pub zwp_virtual_keyboard_manager_v1: u32,
    pub zwp_input_method_manager_v2: u32,
    pub zwp_text_input_manager_v3: u32,
    pub wl_fixes: u32,
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
    pub syncobj_manager: CloneCell<Option<Rc<TestSyncobjManager>>>,
    pub content_type_manager: CloneCell<Option<Rc<TestContentTypeManager>>>,
    pub data_control_manager: CloneCell<Option<Rc<TestDataControlManager>>>,
    pub dmabuf: CloneCell<Option<Rc<TestDmabuf>>>,
    pub drag_manager: CloneCell<Option<Rc<TestToplevelDragManager>>>,
    pub alpha_modifier: CloneCell<Option<Rc<TestAlphaModifier>>>,
    pub virtual_keyboard_manager: CloneCell<Option<Rc<TestVirtualKeyboardManager>>>,
    pub input_method_manager: CloneCell<Option<Rc<TestInputMethodManager>>>,
    pub text_input_manager: CloneCell<Option<Rc<TestTextInputManager>>>,
    pub wl_fixes: CloneCell<Option<Rc<TestWlFixes>>>,
    pub seats: CopyHashMap<GlobalName, Rc<WlSeatGlobal>>,
}

macro_rules! singleton {
    ($field:expr) => {
        if let Some(s) = $field.get() {
            return Ok(s);
        }
    };
}

macro_rules! create_singleton {
    ($fn:ident, $field:ident, $name:ident, $ver:expr, $ty:ident) => {
        pub async fn $fn(&self) -> Result<Rc<$ty>, TestError> {
            singleton!(self.$field);
            let singletons = self.get_singletons().await?;
            singleton!(self.$field);
            let jc = Rc::new($ty::new(&self.tran));
            self.bind(&jc, singletons.$name, $ver)?;
            self.$field.set(Some(jc.clone()));
            Ok(jc)
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
                    let mut $name = u32::MAX;
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
                        $name,
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
            wp_linux_drm_syncobj_manager_v1,
            wp_content_type_manager_v1,
            zwlr_data_control_manager_v1,
            zwp_linux_dmabuf_v1,
            xdg_toplevel_drag_manager_v1,
            wp_alpha_modifier_v1,
            zwp_virtual_keyboard_manager_v1,
            zwp_input_method_manager_v2,
            zwp_text_input_manager_v3,
            wl_fixes,
        };
        self.singletons.set(Some(singletons.clone()));
        Ok(singletons)
    }

    create_singleton!(
        get_jay_compositor,
        jay_compositor,
        jay_compositor,
        6,
        TestJayCompositor
    );
    create_singleton!(get_compositor, compositor, wl_compositor, 6, TestCompositor);
    create_singleton!(
        get_subcompositor,
        subcompositor,
        wl_subcompositor,
        1,
        TestSubcompositor
    );
    create_singleton!(get_shm, shm, wl_shm, 1, TestShm);
    create_singleton!(
        get_spbm,
        spbm,
        wp_single_pixel_buffer_manager_v1,
        1,
        TestSinglePixelBufferManager
    );
    create_singleton!(get_viewporter, viewporter, wp_viewporter, 1, TestViewporter);
    create_singleton!(
        get_activation,
        activation,
        xdg_activation_v1,
        1,
        TestXdgActivation
    );
    create_singleton!(get_xdg, xdg, xdg_wm_base, 6, TestXdgWmBase);
    create_singleton!(
        get_foreign_toplevel_list,
        foreign_toplevel_list,
        ext_foreign_toplevel_list_v1,
        1,
        TestExtForeignToplevelList
    );
    create_singleton!(
        get_data_device_manager,
        data_device_manager,
        wl_data_device_manager,
        3,
        TestDataDeviceManager
    );
    create_singleton!(
        get_cursor_shape_manager,
        cursor_shape_manager,
        wp_cursor_shape_manager_v1,
        1,
        TestCursorShapeManager
    );
    create_singleton!(
        get_syncobj_manager,
        syncobj_manager,
        wp_linux_drm_syncobj_manager_v1,
        1,
        TestSyncobjManager
    );
    create_singleton!(
        get_content_type_manager,
        content_type_manager,
        wp_content_type_manager_v1,
        1,
        TestContentTypeManager
    );
    create_singleton!(
        get_data_control_manager,
        data_control_manager,
        zwlr_data_control_manager_v1,
        2,
        TestDataControlManager
    );
    create_singleton!(get_dmabuf, dmabuf, zwp_linux_dmabuf_v1, 5, TestDmabuf);
    create_singleton!(
        get_drag_manager,
        drag_manager,
        xdg_toplevel_drag_manager_v1,
        1,
        TestToplevelDragManager
    );
    create_singleton!(
        get_alpha_modifier,
        alpha_modifier,
        wp_alpha_modifier_v1,
        1,
        TestAlphaModifier
    );
    create_singleton!(
        get_virtual_keyboard_manager,
        virtual_keyboard_manager,
        zwp_virtual_keyboard_manager_v1,
        1,
        TestVirtualKeyboardManager
    );
    create_singleton!(
        get_input_method_manager,
        input_method_manager,
        zwp_input_method_manager_v2,
        1,
        TestInputMethodManager
    );
    create_singleton!(
        get_text_input_manager,
        text_input_manager,
        zwp_text_input_manager_v3,
        1,
        TestTextInputManager
    );
    create_singleton!(get_wl_fixes, wl_fixes, wl_fixes, 1, TestWlFixes);

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
            id: obj.id(),
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(())
    }

    fn handle_global(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Global::parse_full(parser)?;
        let global = Rc::new(TestGlobal {
            name: ev.name,
            interface: ev.interface.to_string(),
            _version: ev.version,
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
