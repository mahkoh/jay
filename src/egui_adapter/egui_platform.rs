use {
    crate::{
        allocator::{Allocator, AllocatorError, BO_USE_RENDERING, BufferObject},
        async_engine::SpawnedFuture,
        client::{Client, ClientCaps, ClientError},
        cursor::KnownCursor,
        egui_adapter::egui_vulkan::{
            EGV_FORMAT, EgvContext, EgvError, EgvFramebuffer, EgvRenderer,
        },
        gfx_api::SyncFile,
        globals::{GlobalName, Singleton},
        ifs::wl_seat::{
            BTN_LEFT, BTN_RIGHT,
            wl_pointer::{self, HORIZONTAL_SCROLL, PendingScroll, VERTICAL_SCROLL},
        },
        object::Version,
        scale::Scale,
        security_context_acceptor::AcceptorMetadata,
        state::State,
        utils::{
            asyncevent::AsyncEvent, clonecell::CloneCell, copyhashmap::CopyHashMap,
            double_buffered::DoubleBuffered, errorfmt::ErrorFmt, numcell::NumCell,
            oserror::OsError, rc_eq::rc_eq,
        },
        wire::{
            WlSurfaceId,
            wl_pointer::{Button, Enter, Leave, Motion},
            wp_fractional_scale_v1::PreferredScale,
        },
        wl_usr::{
            UsrCon,
            usr_ifs::{
                usr_jay_compositor::UsrJayCompositor,
                usr_jay_sync_file_release::UsrJaySyncFileReleaseOwner,
                usr_jay_sync_file_surface::UsrJaySyncFileSurface,
                usr_wl_buffer::UsrWlBuffer,
                usr_wl_callback::UsrWlCallbackOwner,
                usr_wl_compositor::UsrWlCompositor,
                usr_wl_data_device_manager::UsrWlDataDeviceManager,
                usr_wl_keyboard::{UsrWlKeyboard, UsrWlKeyboardOwner},
                usr_wl_pointer::{UsrWlPointer, UsrWlPointerOwner},
                usr_wl_registry::UsrWlRegistry,
                usr_wl_seat::UsrWlSeat,
                usr_wl_surface::UsrWlSurface,
                usr_wp_cursor_shape_device_v1::UsrWpCursorShapeDeviceV1,
                usr_wp_cursor_shape_manager_v1::UsrWpCursorShapeManagerV1,
                usr_wp_fractional_scale::{UsrWpFractionalScale, UsrWpFractionalScaleOwner},
                usr_wp_fractional_scale_manager::UsrWpFractionalScaleManager,
                usr_wp_viewport::UsrWpViewport,
                usr_wp_viewporter::UsrWpViewporter,
                usr_xdg_surface::{UsrXdgSurface, UsrXdgSurfaceOwner},
                usr_xdg_toplevel::{UsrXdgToplevel, UsrXdgToplevelOwner},
                usr_xdg_wm_base::UsrXdgWmBase,
                usr_zwp_linux_dmabuf_v1::UsrZwpLinuxDmabufV1,
                usr_zwp_primary_selection_device_manager::UsrZwpPrimarySelectionDeviceManagerV1,
            },
        },
    },
    egui::{
        CursorIcon, Event, FontData, FontDefinitions, FontFamily, FullOutput, Key, Modifiers,
        MouseWheelUnit, OutputCommand, PlatformOutput, PointerButton, Pos2, RawInput, Vec2,
        ViewportCommand, ViewportEvent, ViewportId, ViewportInfo, pos2, vec2,
    },
    fontconfig::Fontconfig,
    futures_util::{FutureExt, select},
    isnt::std_1::primitive::{IsntCharExt, IsntSliceExt, IsntStrExt},
    kbvm::{Keysym, ModifierMask, lookup::Lookup},
    std::{
        cell::{Cell, RefCell},
        collections::btree_map::Entry,
        fs, mem,
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
pub enum EggError {
    #[error("Could not initialize fontconfig")]
    InitializeFontconfig,
    #[error("Could not create a socket pair")]
    CreateSocketPair(#[source] OsError),
    #[error("Could not spawn a client")]
    SpawnClient(#[source] ClientError),
    #[error("Could not create a renderer")]
    CreateRenderer(#[source] EgvError),
    #[error("There is no render context")]
    NoRenderContext,
    #[error("Could not allocate a buffer")]
    AllocateBuffer(#[source] AllocatorError),
    #[error("Could not import a framebuffer")]
    ImportFramebuffer(#[source] EgvError),
    #[error("Could not render")]
    Render(#[source] EgvError),
    #[error("No viewport output")]
    NoViewportOutput,
}

pub struct EggState {
    fc: Fontconfig,
    fonts: RefCell<EggFonts>,
    ctx: CloneCell<Option<Rc<EggContext>>>,
    next_context_id: NumCell<u64>,
    cxts: CopyHashMap<u64, Rc<EggContextInner>>,
}

#[derive(Default)]
struct EggFonts {
    definitions: Option<FontDefinitions>,
    proportional: Vec<String>,
    monospace: Vec<String>,
}

pub struct EggContext {
    inner: Rc<EggContextInner>,
}

struct EggContextInner {
    id: u64,
    renderer: Rc<EgvRenderer>,
    allocator: Rc<dyn Allocator>,
    state: Rc<State>,
    _client: Rc<Client>,
    con: Rc<UsrCon>,
    jay_compositor: Rc<UsrJayCompositor>,
    wl_compositor: Rc<UsrWlCompositor>,
    xdg_wm_base: Rc<UsrXdgWmBase>,
    _wl_data_device_manager: Rc<UsrWlDataDeviceManager>,
    _zwp_primary_selection_device_manager_v1: Rc<UsrZwpPrimarySelectionDeviceManagerV1>,
    wp_viewporter: Rc<UsrWpViewporter>,
    wp_cursor_shape_manager_v1: Rc<UsrWpCursorShapeManagerV1>,
    wp_fractional_scale_manager: Rc<UsrWpFractionalScaleManager>,
    zwp_linux_dmabuf_v1: Rc<UsrZwpLinuxDmabufV1>,
    registry: Rc<UsrWlRegistry>,
    windows: CopyHashMap<WlSurfaceId, Rc<EggWindowInner>>,
    seats: CopyHashMap<GlobalName, EggSeat>,
}

struct EggSeat {
    inner: Rc<EggSeatInner>,
}

pub struct EggSeatInner {
    ctx: Rc<EggContextInner>,
    global_name: GlobalName,
    wl_seat: Rc<UsrWlSeat>,
    wl_pointer: Rc<UsrWlPointer>,
    pointer_window: CloneCell<Option<Rc<EggWindowInner>>>,
    pointer_enter_serial: Cell<u32>,
    pointer_serial: Cell<u32>,
    pointer_pos: Cell<Pos2>,
    kb_modifiers: Cell<Modifiers>,
    wp_cursor_shape_device_v1: Rc<UsrWpCursorShapeDeviceV1>,
    wl_keyboard: Rc<UsrWlKeyboard>,
    kb_window: CloneCell<Option<Rc<EggWindowInner>>>,
    kb_serial: Cell<u32>,
}

pub struct EggWindow {
    _ctx: Rc<EggContext>,
    inner: Rc<EggWindowInner>,
    _render_task: SpawnedFuture<()>,
    _timer_task: SpawnedFuture<()>,
}

pub trait EggWindowOwner {
    fn close(&self);
    fn render(self: Rc<Self>, ctx: &egui::Context);
}

struct EggWindowInner {
    ctx: Rc<EggContextInner>,
    egv: Rc<EgvContext>,
    egui: egui::Context,
    wl_surface: Rc<UsrWlSurface>,
    wp_viewport: Rc<UsrWpViewport>,
    wp_fractional_scale: Rc<UsrWpFractionalScale>,
    xdg_surface: Rc<UsrXdgSurface>,
    xdg_toplevel: Rc<UsrXdgToplevel>,
    jay_sync_file_surface: Rc<UsrJaySyncFileSurface>,
    frame_task: AsyncEvent,
    want_frame: Cell<bool>,
    have_frame: Cell<bool>,
    initial_commit_pending: Cell<bool>,
    owner: CloneCell<Option<Rc<dyn EggWindowOwner>>>,
    active_seat: CloneCell<Option<Rc<EggSeatInner>>>,
    raw_input: RefCell<Option<RawInput>>,
    close: Cell<bool>,
    repaint_timeout: Cell<u64>,
    repaint_timeout_changed: AsyncEvent,
    fonts_changed: Cell<bool>,

    buffers: DoubleBuffered<CloneCell<Option<Rc<EggFramebuffer>>>>,

    surface_pending: RefCell<PendingWindowState>,
    logical_size: Cell<[i32; 2]>,
    physical_size: Cell<[i32; 2]>,
    scale: Cell<Scale>,
}

struct EggFramebuffer {
    client_acquire_fence: CloneCell<Option<Option<SyncFile>>>,
    size: Cell<[i32; 2]>,
    bo: Rc<dyn BufferObject>,
    egv: Rc<EgvFramebuffer>,
    wl_buffer: Rc<UsrWlBuffer>,
    window: Weak<EggWindowInner>,
}

#[derive(Default)]
struct PendingWindowState {
    size: Option<(i32, i32)>,
}

const PROPORTIONAL_FONTS: &[&str] = &["sans-serif", "Noto Sans", "Noto Color Emoji"];

const MONOSPACE_FONTS: &[&str] = &["monospace", "Noto Sans Mono", "Noto Color Emoji"];

impl EggState {
    pub fn new() -> Result<Self, EggError> {
        let Some(fc) = Fontconfig::new() else {
            return Err(EggError::InitializeFontconfig);
        };
        let slf = Self {
            fc,
            fonts: Default::default(),
            ctx: Default::default(),
            next_context_id: Default::default(),
            cxts: Default::default(),
        };
        slf.set_proportional_fonts(PROPORTIONAL_FONTS.iter().map(|s| s.to_string()).collect());
        slf.set_monospace_fonts(MONOSPACE_FONTS.iter().map(|s| s.to_string()).collect());
        Ok(slf)
    }

    pub fn set_proportional_fonts(&self, fonts: Vec<String>) {
        self.change_fonts(fonts, |f| &mut f.proportional)
    }

    pub fn set_monospace_fonts(&self, fonts: Vec<String>) {
        self.change_fonts(fonts, |f| &mut f.monospace)
    }

    fn change_fonts(&self, fonts: Vec<String>, field: impl Fn(&mut EggFonts) -> &mut Vec<String>) {
        let f = &mut *self.fonts.borrow_mut();
        let field = field(f);
        if *field == fonts {
            return;
        }
        *field = fonts;
        f.definitions.take();
        for ctx in self.cxts.lock().values() {
            for window in ctx.windows.lock().values() {
                window.fonts_changed.set(true);
                window.want_frame();
            }
        }
    }

    pub fn clear(&self) {
        self.ctx.take();
    }

    fn font_definitions(&self) -> FontDefinitions {
        let f = &mut self.fonts.borrow_mut();
        if let Some(d) = &f.definitions {
            return d.clone();
        }
        let mut d = FontDefinitions::empty();
        for (ff, list) in [
            (FontFamily::Proportional, &f.proportional),
            (FontFamily::Monospace, &f.monospace),
        ] {
            for family in list {
                let Some(font) = self.fc.find(family, None) else {
                    log::warn!("Could not find font family {family}");
                    continue;
                };
                if let Entry::Vacant(e) = d.font_data.entry(font.name.clone()) {
                    let data = match fs::read(&font.path) {
                        Ok(f) => f,
                        Err(e) => {
                            log::error!("Could not read {}: {}", font.path.display(), ErrorFmt(e));
                            continue;
                        }
                    };
                    let data = Arc::new(FontData {
                        font: data.into(),
                        index: font.index.unwrap_or(0) as u32,
                        tweak: Default::default(),
                    });
                    e.insert(data);
                }
                let list = d.families.entry(ff.clone()).or_default();
                if list.not_contains(&font.name) {
                    list.push(font.name);
                }
            }
        }
        {
            let name = "material-icons";
            let list = d.families.entry(FontFamily::Proportional).or_default();
            if list.iter().all(|n| n != name) {
                if let Entry::Vacant(e) = d.font_data.entry(name.to_string()) {
                    let data = Arc::new(FontData {
                        font: egui_material_icons::FONT_DATA.into(),
                        index: 0,
                        tweak: Default::default(),
                    });
                    e.insert(data);
                }
                list.push(name.to_string());
            }
        }
        f.definitions = Some(d.clone());
        d
    }
}

impl State {
    pub fn get_egg_context(self: &Rc<Self>) -> Result<Rc<EggContext>, EggError> {
        if let Some(ctx) = self.egg_state.ctx.get() {
            return Ok(ctx);
        }
        let Some(ctx) = self.render_ctx.get() else {
            return Err(EggError::NoRenderContext);
        };
        let (client1, client2) = uapi::socketpair(c::AF_UNIX, c::SOCK_STREAM | c::SOCK_CLOEXEC, 0)
            .map_err(Into::into)
            .map_err(EggError::CreateSocketPair)?;
        let allocator = ctx.allocator();
        let dev = allocator.drm().map(|d| d.dev());
        let renderer =
            EgvRenderer::new(&self.eng, &self.ring, dev).map_err(EggError::CreateRenderer)?;
        let con = UsrCon::from_socket(
            &self.ring,
            &self.wheel,
            &self.eng,
            &self.dma_buf_ids,
            &Rc::new(client1),
            0,
        );
        let client = self
            .clients
            .spawn2(
                self.clients.id(),
                self,
                Rc::new(client2),
                uapi::getuid(),
                uapi::getpid(),
                ClientCaps::all(),
                true,
                false,
                &Rc::new(AcceptorMetadata::secure()),
            )
            .map_err(EggError::SpawnClient)?;
        let registry = con.get_registry();
        let jay_compositor = {
            let obj = Rc::new(UsrJayCompositor {
                id: con.id(),
                con: con.clone(),
                owner: Default::default(),
                caps: Default::default(),
                version: Version(25),
            });
            registry.bind(self.globals.singletons[Singleton::JayCompositor], &*obj);
            con.add_object(obj.clone());
            obj
        };
        macro_rules! add_singletons {
            ($($name:ident, $global:ident, $ty:ident, $version:expr;)*) => {
                $(
                    let $name = Rc::new($ty {
                        id: con.id(),
                        con: con.clone(),
                        version: Version($version),
                    });
                    registry.bind(self.globals.singletons[Singleton::$global], &*$name);
                    con.add_object($name.clone());
                )*
            };
        }
        add_singletons! {
            wl_compositor, WlCompositor, UsrWlCompositor, 6;
            xdg_wm_base, XdgWmBase, UsrXdgWmBase, 7;
            wl_data_device_manager, WlDataDeviceManager, UsrWlDataDeviceManager, 3;
            zwp_primary_selection_device_manager_v1, ZwpPrimarySelectionDeviceManagerV1, UsrZwpPrimarySelectionDeviceManagerV1, 1;
            wp_viewporter, WpViewporter, UsrWpViewporter, 1;
            wp_cursor_shape_manager_v1, WpCursorShapeManagerV1, UsrWpCursorShapeManagerV1, 2;
            wp_fractional_scale_manager, WpFractionalScaleManagerV1, UsrWpFractionalScaleManager, 1;
            zwp_linux_dmabuf_v1, ZwpLinuxDmabufV1, UsrZwpLinuxDmabufV1, 5;
        }
        let ctx = Rc::new(EggContext {
            inner: Rc::new(EggContextInner {
                id: self.egg_state.next_context_id.fetch_add(1),
                renderer,
                allocator,
                state: self.clone(),
                _client: client.clone(),
                con,
                jay_compositor,
                wl_compositor,
                xdg_wm_base,
                _wl_data_device_manager: wl_data_device_manager,
                _zwp_primary_selection_device_manager_v1: zwp_primary_selection_device_manager_v1,
                wp_viewporter,
                wp_cursor_shape_manager_v1,
                wp_fractional_scale_manager,
                zwp_linux_dmabuf_v1,
                registry,
                windows: Default::default(),
                seats: Default::default(),
            }),
        });
        self.egg_state.cxts.set(ctx.inner.id, ctx.inner.clone());
        for &global_name in self.globals.seats.lock().keys() {
            ctx.inner.add_seat(global_name);
        }
        self.egg_state.ctx.set(Some(ctx.clone()));
        Ok(ctx)
    }
}

impl EggContext {
    pub fn create_window(self: &Rc<Self>, title: &str) -> Rc<EggWindow> {
        let i = &self.inner;
        let wl_surface = i.wl_compositor.create_surface();
        let jay_sync_file_surface = i.jay_compositor.get_sync_file_surface(&wl_surface);
        let xdg_surface = i.xdg_wm_base.get_xdg_surface(&wl_surface);
        let xdg_toplevel = xdg_surface.get_toplevel();
        xdg_toplevel.set_title(title);
        let wp_fractional_scale = i
            .wp_fractional_scale_manager
            .get_fractional_scale(&wl_surface);
        let wp_viewport = i.wp_viewporter.get_viewport(&wl_surface);
        wl_surface.commit();
        let window = Rc::new(EggWindowInner {
            ctx: self.inner.clone(),
            egv: i.renderer.create_context(),
            egui: egui::Context::default(),
            wl_surface,
            wp_viewport,
            wp_fractional_scale,
            xdg_surface,
            xdg_toplevel,
            jay_sync_file_surface,
            frame_task: Default::default(),
            want_frame: Default::default(),
            have_frame: Cell::new(true),
            initial_commit_pending: Cell::new(true),
            owner: Default::default(),
            active_seat: Default::default(),
            raw_input: RefCell::new(None),
            close: Default::default(),
            repaint_timeout: Cell::new(u64::MAX),
            repaint_timeout_changed: Default::default(),
            fonts_changed: Cell::new(true),
            buffers: Default::default(),
            surface_pending: Default::default(),
            logical_size: Cell::new([800, 600]),
            physical_size: Cell::new([800, 600]),
            scale: Default::default(),
        });
        window.xdg_surface.owner.set(Some(window.clone()));
        window.xdg_toplevel.owner.set(Some(window.clone()));
        window.wp_fractional_scale.owner.set(Some(window.clone()));
        i.windows.set(window.wl_surface.id, window.clone());
        let render_task = i
            .state
            .eng
            .spawn("egui-window", window.clone().render_frames());
        let repaint_task = i.state.eng.spawn("egui-window", window.clone().repaint());
        let window = EggWindow {
            _ctx: self.clone(),
            inner: window,
            _render_task: render_task,
            _timer_task: repaint_task,
        };
        Rc::new(window)
    }
}

impl EggContextInner {
    pub fn add_seat(self: &Rc<Self>, global_name: GlobalName) {
        let wl_seat = Rc::new(UsrWlSeat {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: Version(10),
        });
        self.registry.bind(global_name, &*wl_seat);
        self.con.add_object(wl_seat.clone());
        let wl_pointer = wl_seat.get_pointer();
        let wl_keyboard = wl_seat.get_keyboard();
        let wp_cursor_shape_device_v1 = self.wp_cursor_shape_manager_v1.get_pointer(&wl_pointer);
        let seat = Rc::new(EggSeatInner {
            ctx: self.clone(),
            global_name,
            wl_seat,
            wl_pointer,
            pointer_window: Default::default(),
            pointer_enter_serial: Default::default(),
            pointer_serial: Default::default(),
            pointer_pos: Default::default(),
            kb_modifiers: Default::default(),
            wp_cursor_shape_device_v1,
            wl_keyboard,
            kb_window: Default::default(),
            kb_serial: Default::default(),
        });
        seat.wl_pointer.owner.set(Some(seat.clone()));
        seat.wl_keyboard.owner.set(Some(seat.clone()));
        let seat = EggSeat { inner: seat };
        self.seats.set(global_name, seat);
    }
}

impl EggWindow {
    pub fn request_redraw(&self) {
        self.inner.want_frame();
    }

    pub fn set_owner(&self, owner: Option<Rc<dyn EggWindowOwner>>) {
        self.inner.owner.set(owner);
    }
}

impl EggWindowInner {
    fn update_physical_size(&self) {
        let size = self.logical_size.get();
        let scale = self.scale.get();
        let physical_size = scale.pixel_size(size);
        if self.physical_size.replace(physical_size) != physical_size {
            self.want_frame();
        }
    }

    fn want_frame(&self) {
        self.want_frame.set(true);
        self.frame_task.trigger();
    }

    fn retry_frame(&self) {
        if self.want_frame.get() {
            self.frame_task.trigger();
        }
    }

    async fn repaint(self: Rc<Self>) {
        loop {
            let timeout = self.ctx.state.ring.timeout(self.repaint_timeout.get());
            let triggered = || self.repaint_timeout_changed.triggered();
            let timeout = select! {
                _ = timeout.fuse() => true,
                _ = triggered().fuse() => false,
            };
            if timeout {
                self.want_frame();
                triggered().await;
            }
        }
    }

    async fn render_frames(self: Rc<Self>) {
        loop {
            self.frame_task.triggered().await;
            if let Err(e) = self.render_frame() {
                log::error!("Could not render frame: {}", ErrorFmt(e));
                break;
            }
        }
    }

    fn render_frame(self: &Rc<Self>) -> Result<(), EggError> {
        if self.fonts_changed.take() {
            self.egui
                .set_fonts(self.ctx.state.egg_state.font_definitions());
        }
        if self.initial_commit_pending.get() {
            return Ok(());
        }
        if !self.have_frame.get() {
            return Ok(());
        }
        if !self.want_frame.get() {
            return Ok(());
        }
        let Some(owner) = self.owner.get() else {
            return Ok(());
        };
        let Some(render_ctx) = self.ctx.state.render_ctx.get() else {
            return Ok(());
        };
        let Some(format) = render_ctx.formats().get(&EGV_FORMAT.drm) else {
            return Ok(());
        };
        let logical_size = self.logical_size.get();
        let physical_size = self.physical_size.get();
        let mut fb_opt = self.buffers.back().get();
        'check: {
            if let Some(fb) = &fb_opt {
                if fb.size.get() != physical_size {
                    fb_opt = None;
                    break 'check;
                }
                if !format.read_modifiers.contains(&fb.bo.dmabuf().modifier) {
                    fb_opt = None;
                    break 'check;
                }
            }
        }
        let fb = match fb_opt {
            Some(fb) => fb,
            _ => {
                let modifiers: Vec<_> = self
                    .ctx
                    .renderer
                    .support()
                    .iter()
                    .filter(|s| {
                        s.max_width >= physical_size[0] as u32
                            && s.max_height >= physical_size[1] as u32
                            && format.read_modifiers.contains(&s.modifier)
                    })
                    .map(|s| s.modifier)
                    .collect();
                let bo = self
                    .ctx
                    .allocator
                    .create_bo(
                        &self.ctx.state.dma_buf_ids,
                        physical_size[0],
                        physical_size[1],
                        EGV_FORMAT,
                        &modifiers,
                        BO_USE_RENDERING,
                    )
                    .map_err(EggError::AllocateBuffer)?;
                let egv = self
                    .egv
                    .import_framebuffer(bo.dmabuf())
                    .map_err(EggError::ImportFramebuffer)?;
                let wl_buffer = self.ctx.zwp_linux_dmabuf_v1.create_buffer(bo.dmabuf());
                let fb = Rc::new(EggFramebuffer {
                    client_acquire_fence: CloneCell::new(Some(None)),
                    size: Cell::new(physical_size),
                    bo,
                    egv,
                    wl_buffer,
                    window: Rc::downgrade(self),
                });
                self.buffers.back().set(Some(fb.clone()));
                fb
            }
        };
        let Some(sync_file) = fb.client_acquire_fence.get() else {
            return Ok(());
        };
        log::info!("render");
        let raw_input = self
            .raw_input
            .take()
            .unwrap_or_else(|| self.default_raw_input());
        let full_output = self.egui.run(raw_input, |ctx| {
            owner.clone().render(ctx);
        });
        let FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output,
        } = full_output;
        let primitives = self.egui.tessellate(shapes, pixels_per_point);
        let sync_file = fb
            .egv
            .render(
                textures_delta,
                pixels_per_point,
                &primitives,
                (0.0, 0.0),
                sync_file.as_ref(),
            )
            .map_err(EggError::Render)?;
        let PlatformOutput {
            commands,
            cursor_icon,
            ..
        } = platform_output;
        if let Some(seat) = self.active_seat.get() {
            'set_icon: {
                let cursor = match cursor_icon {
                    CursorIcon::Default => KnownCursor::Default,
                    CursorIcon::None => {
                        seat.wl_pointer
                            .set_cursor(seat.pointer_serial.get(), None, 0, 0);
                        break 'set_icon;
                    }
                    CursorIcon::ContextMenu => KnownCursor::ContextMenu,
                    CursorIcon::Help => KnownCursor::Help,
                    CursorIcon::PointingHand => KnownCursor::Pointer,
                    CursorIcon::Progress => KnownCursor::Progress,
                    CursorIcon::Wait => KnownCursor::Wait,
                    CursorIcon::Cell => KnownCursor::Cell,
                    CursorIcon::Crosshair => KnownCursor::Crosshair,
                    CursorIcon::Text => KnownCursor::Text,
                    CursorIcon::VerticalText => KnownCursor::VerticalText,
                    CursorIcon::Alias => KnownCursor::Alias,
                    CursorIcon::Copy => KnownCursor::Copy,
                    CursorIcon::Move => KnownCursor::Move,
                    CursorIcon::NoDrop => KnownCursor::NoDrop,
                    CursorIcon::NotAllowed => KnownCursor::NotAllowed,
                    CursorIcon::Grab => KnownCursor::Grab,
                    CursorIcon::Grabbing => KnownCursor::Grabbing,
                    CursorIcon::AllScroll => KnownCursor::AllScroll,
                    CursorIcon::ResizeHorizontal => KnownCursor::EwResize,
                    CursorIcon::ResizeNeSw => KnownCursor::NeswResize,
                    CursorIcon::ResizeNwSe => KnownCursor::NwseResize,
                    CursorIcon::ResizeVertical => KnownCursor::NsResize,
                    CursorIcon::ResizeEast => KnownCursor::EResize,
                    CursorIcon::ResizeSouthEast => KnownCursor::SeResize,
                    CursorIcon::ResizeSouth => KnownCursor::SResize,
                    CursorIcon::ResizeSouthWest => KnownCursor::SwResize,
                    CursorIcon::ResizeWest => KnownCursor::WResize,
                    CursorIcon::ResizeNorthWest => KnownCursor::NwResize,
                    CursorIcon::ResizeNorth => KnownCursor::NResize,
                    CursorIcon::ResizeNorthEast => KnownCursor::NeResize,
                    CursorIcon::ResizeColumn => KnownCursor::ColResize,
                    CursorIcon::ResizeRow => KnownCursor::RowResize,
                    CursorIcon::ZoomIn => KnownCursor::ZoomIn,
                    CursorIcon::ZoomOut => KnownCursor::ZoomOut,
                };
                seat.wp_cursor_shape_device_v1
                    .set_shape(seat.pointer_serial.get(), cursor);
            }
        }
        for command in commands {
            match command {
                OutputCommand::CopyText(_) => {}
                OutputCommand::CopyImage(_) => {}
                OutputCommand::OpenUrl(url) => {
                    if let Some(forker) = self.ctx.state.forker.get() {
                        forker.spawn("xdg-open".to_string(), vec![url.url], vec![], vec![]);
                    }
                }
            }
        }
        let Some(viewport) = viewport_output.get(&ViewportId::ROOT) else {
            return Err(EggError::NoViewportOutput);
        };
        for command in &viewport.commands {
            match command {
                ViewportCommand::Close => self.close.set(true),
                ViewportCommand::CancelClose => self.close.set(false),
                ViewportCommand::Title(s) => self.xdg_toplevel.set_title(s),
                ViewportCommand::Fullscreen(b) => self.xdg_toplevel.set_fullscreen(*b),
                _ => {}
            }
        }
        let repaint_delay = u64::try_from(viewport.repaint_delay.as_nanos()).unwrap_or(u64::MAX);
        let repaint_timeout = self.ctx.state.now_nsec().saturating_add(repaint_delay);
        self.repaint_timeout.set(repaint_timeout);
        if repaint_timeout != u64::MAX {
            self.repaint_timeout_changed.trigger();
        }
        self.wl_surface.attach(&fb.wl_buffer);
        self.wl_surface.damage();
        self.jay_sync_file_surface.set_acquire(sync_file.as_ref());
        self.jay_sync_file_surface
            .get_release()
            .owner
            .set(Some(fb.clone()));
        self.wl_surface.frame().owner.set(Some(self.clone()));
        self.wp_viewport
            .set_destination(logical_size[0], logical_size[1]);
        self.wl_surface.commit();
        fb.client_acquire_fence.take();
        self.buffers.flip();
        self.have_frame.set(false);
        self.want_frame.set(false);
        if self.close.get() {
            owner.close();
        }
        Ok(())
    }
}

impl UsrXdgSurfaceOwner for EggWindowInner {
    fn configure(&self) {
        let pending = mem::take(&mut *self.surface_pending.borrow_mut());
        if let Some((mut w, mut h)) = pending.size {
            let [old_w, old_h] = self.logical_size.get();
            w = if w > 0 { w } else { old_w };
            h = if h > 0 { h } else { old_h };
            let size = [w, h];
            if self.logical_size.replace(size) != size {
                self.update_physical_size();
            }
        }
        if self.initial_commit_pending.take() {
            self.want_frame();
        }
    }
}

impl UsrXdgToplevelOwner for EggWindowInner {
    fn configure(&self, width: i32, height: i32) {
        self.surface_pending.borrow_mut().size = Some((width, height));
    }

    fn close(&self) {
        let raw_input = &mut *self.raw_input.borrow_mut();
        let raw_input = raw_input.get_or_insert_with(|| self.default_raw_input());
        raw_input
            .viewports
            .get_mut(&ViewportId::ROOT)
            .unwrap()
            .events
            .push(ViewportEvent::Close);
        self.close.set(true);
        self.want_frame();
    }
}

impl UsrWpFractionalScaleOwner for EggWindowInner {
    fn preferred_scale(self: Rc<Self>, ev: &PreferredScale) {
        let scale = Scale::from_wl(ev.scale);
        if self.scale.replace(scale) != scale {
            self.update_physical_size();
        }
    }
}

impl EggWindowInner {
    fn event(&self, event: Event) {
        let raw_input = &mut *self.raw_input.borrow_mut();
        let raw_input = raw_input.get_or_insert_with(|| self.default_raw_input());
        raw_input.events.push(event);
        self.want_frame();
    }

    fn default_raw_input(&self) -> RawInput {
        let viewport_info = ViewportInfo {
            native_pixels_per_point: Some(self.scale.get().to_f64() as _),
            ..Default::default()
        };
        let size = self.logical_size.get();
        let size =
            egui::Rect::from_min_size(Pos2::default(), Vec2::new(size[0] as f32, size[1] as f32));
        let mut modifiers = egui::Modifiers::default();
        if let Some(seat) = self.active_seat.get() {
            modifiers = seat.kb_modifiers.get();
        }
        RawInput {
            viewport_id: ViewportId::ROOT,
            viewports: std::iter::once((ViewportId::ROOT, viewport_info)).collect(),
            screen_rect: Some(size),
            max_texture_side: Some(self.ctx.renderer.max_texture_side()),
            time: Some(self.ctx.state.now_nsec() as f64 / 1_000_000_000.0),
            modifiers,
            ..Default::default()
        }
    }
}

impl EggSeatInner {
    fn activate_pointer_window(self: &Rc<Self>) -> Option<Rc<EggWindowInner>> {
        let window = self.pointer_window.get()?;
        window.active_seat.set(Some(self.clone()));
        Some(window)
    }

    fn activate_kb_window(self: &Rc<Self>) -> Option<Rc<EggWindowInner>> {
        let window = self.kb_window.get()?;
        window.active_seat.set(Some(self.clone()));
        Some(window)
    }

    fn leave(self: &Rc<Self>) {
        if let Some(window) = self.pointer_window.take()
            && let Some(active_seat) = window.active_seat.get()
            && rc_eq(&active_seat, &self)
        {
            window.active_seat.take();
        }
    }

    fn unfocus(self: &Rc<Self>) {
        if let Some(window) = self.kb_window.take()
            && let Some(active_seat) = window.active_seat.get()
            && rc_eq(&active_seat, &self)
        {
            window.active_seat.take();
        }
    }
}

impl UsrWlPointerOwner for EggSeatInner {
    fn enter(self: Rc<Self>, ev: &Enter) {
        let Some(window) = self.ctx.windows.get(&ev.surface) else {
            return;
        };
        self.pointer_window.set(Some(window.clone()));
        self.pointer_enter_serial.set(ev.serial);
        self.pointer_serial.set(ev.serial);
        window.active_seat.set(Some(self.clone()));
    }

    fn leave(self: Rc<Self>, _ev: &Leave) {
        (&self).leave();
    }

    fn motion(self: Rc<Self>, ev: &Motion) {
        let Some(window) = self.activate_pointer_window() else {
            return;
        };
        let pos = pos2(ev.surface_x.to_f32(), ev.surface_y.to_f32());
        self.pointer_pos.set(pos);
        window.event(egui::Event::PointerMoved(pos));
    }

    fn button(self: Rc<Self>, ev: &Button) {
        let Some(window) = self.activate_pointer_window() else {
            return;
        };
        self.pointer_serial.set(ev.serial);
        let button = match ev.button {
            BTN_LEFT => PointerButton::Primary,
            BTN_RIGHT => PointerButton::Secondary,
            _ => return,
        };
        window.event(egui::Event::PointerButton {
            pos: self.pointer_pos.get(),
            button,
            pressed: ev.state == wl_pointer::PRESSED,
            modifiers: self.kb_modifiers.get(),
        });
    }

    fn scroll(self: Rc<Self>, ps: &PendingScroll) {
        let Some(window) = self.activate_pointer_window() else {
            return;
        };
        let v120_x = ps.v120[HORIZONTAL_SCROLL].get();
        let v120_y = ps.v120[VERTICAL_SCROLL].get();
        let px_x = ps.px[HORIZONTAL_SCROLL].get();
        let px_y = ps.px[VERTICAL_SCROLL].get();
        let unit;
        let delta;
        if v120_x.is_some() || v120_y.is_some() {
            unit = MouseWheelUnit::Line;
            delta = vec2(
                -v120_x.unwrap_or_default() as f32 / 120.0,
                -v120_y.unwrap_or_default() as f32 / 120.0,
            );
        } else if px_x.is_some() || px_y.is_some() {
            unit = MouseWheelUnit::Point;
            delta = vec2(
                -px_x.unwrap_or_default().to_f32(),
                -px_y.unwrap_or_default().to_f32(),
            );
        } else {
            return;
        }
        window.event(egui::Event::MouseWheel {
            unit,
            delta,
            modifiers: self.kb_modifiers.get(),
        });
    }
}

impl EggSeatInner {
    fn handle_key(self: &Rc<Self>, lookup: Lookup<'_>, serial: u32, down: bool) {
        let Some(window) = self.activate_kb_window() else {
            return;
        };
        self.kb_serial.set(serial);
        if down {
            let mut text = String::new();
            for key in lookup {
                if let Some(c) = key.char()
                    && c.is_not_control()
                {
                    text.push(c);
                }
            }
            if text.is_not_empty() {
                window.event(Event::Text(text));
            }
        }
        for key in lookup {
            let Some(key) = map_key(key.keysym()) else {
                continue;
            };
            window.event(Event::Key {
                key,
                physical_key: None,
                pressed: down,
                repeat: false,
                modifiers: map_mods(lookup.remaining_mods()),
            });
        }
    }
}

impl UsrWlKeyboardOwner for EggSeatInner {
    fn focus(self: Rc<Self>, surface: WlSurfaceId, serial: u32) {
        let Some(window) = self.ctx.windows.get(&surface) else {
            return;
        };
        self.kb_window.set(Some(window.clone()));
        self.kb_serial.set(serial);
        window.active_seat.set(Some(self.clone()));
    }

    fn unfocus(self: Rc<Self>) {
        (&self).unfocus();
    }

    fn modifiers(self: Rc<Self>, mods: ModifierMask) {
        self.kb_modifiers.set(map_mods(mods));
    }

    fn down(self: Rc<Self>, lookup: Lookup<'_>, serial: u32) {
        self.handle_key(lookup, serial, true);
    }

    fn repeat(self: Rc<Self>, lookup: Lookup<'_>, serial: u32) {
        self.handle_key(lookup, serial, true);
    }

    fn up(self: Rc<Self>, lookup: Lookup<'_>, serial: u32) {
        self.handle_key(lookup, serial, false);
    }
}

fn map_mods(mods: ModifierMask) -> Modifiers {
    Modifiers {
        alt: mods.contains(ModifierMask::ALT),
        ctrl: mods.contains(ModifierMask::CONTROL),
        shift: mods.contains(ModifierMask::SHIFT),
        mac_cmd: false,
        command: mods.contains(ModifierMask::CONTROL),
    }
}

impl UsrJaySyncFileReleaseOwner for EggFramebuffer {
    fn release(&self, sync_file: Option<SyncFile>) {
        self.client_acquire_fence.set(Some(sync_file));
        if let Some(window) = self.window.upgrade() {
            window.retry_frame();
        }
    }
}

impl UsrWlCallbackOwner for EggWindowInner {
    fn done(self: Rc<Self>) {
        self.have_frame.set(true);
        self.retry_frame();
    }
}

fn map_key(kc: Keysym) -> Option<Key> {
    use {Key as K, kbvm::syms as s};
    let key = match kc {
        s::Down | s::KP_Down => K::ArrowDown,
        s::Left | s::KP_Left => K::ArrowLeft,
        s::Right | s::KP_Right => K::ArrowRight,
        s::Up | s::KP_Up => K::ArrowUp,
        s::Escape => K::Escape,
        s::Tab | s::KP_Tab => K::Tab,
        s::BackSpace => K::Backspace,
        s::Return | s::KP_Enter => K::Enter,
        s::space | s::KP_Space => K::Space,
        s::Insert | s::KP_Insert => K::Insert,
        s::Delete | s::KP_Delete => K::Delete,
        s::Home | s::KP_Home | s::KP_Begin => K::Home,
        s::End | s::KP_End => K::End,
        s::Page_Up | s::KP_Page_Up => K::PageUp,
        s::Page_Down | s::KP_Page_Down => K::PageDown,
        s::XF86Copy => K::Copy,
        s::XF86Cut => K::Cut,
        s::XF86Paste => K::Paste,
        s::colon => K::Colon,
        s::comma => K::Comma,
        s::backslash => K::Backslash,
        s::slash | s::KP_Divide => K::Slash,
        s::bar => K::Pipe,
        s::question => K::Questionmark,
        s::exclam => K::Exclamationmark,
        s::bracketleft => K::OpenBracket,
        s::bracketright => K::CloseBracket,
        s::braceleft => K::OpenCurlyBracket,
        s::braceright => K::CloseCurlyBracket,
        s::grave => K::Backtick,
        s::minus | s::KP_Subtract => K::Minus,
        s::period | s::KP_Decimal => K::Period,
        s::plus | s::KP_Add => K::Plus,
        s::equal | s::KP_Equal => K::Equals,
        s::semicolon => K::Semicolon,
        s::quotedbl => K::Quote,
        s::KP_0 | s::_0 => K::Num0,
        s::KP_1 | s::_1 => K::Num1,
        s::KP_2 | s::_2 => K::Num2,
        s::KP_3 | s::_3 => K::Num3,
        s::KP_4 | s::_4 => K::Num4,
        s::KP_5 | s::_5 => K::Num5,
        s::KP_6 | s::_6 => K::Num6,
        s::KP_7 | s::_7 => K::Num7,
        s::KP_8 | s::_8 => K::Num8,
        s::KP_9 | s::_9 => K::Num9,
        s::A => K::A,
        s::B => K::B,
        s::C => K::C,
        s::D => K::D,
        s::E => K::E,
        s::F => K::F,
        s::G => K::G,
        s::H => K::H,
        s::I => K::I,
        s::J => K::J,
        s::K => K::K,
        s::L => K::L,
        s::M => K::M,
        s::N => K::N,
        s::O => K::O,
        s::P => K::P,
        s::Q => K::Q,
        s::R => K::R,
        s::S => K::S,
        s::T => K::T,
        s::U => K::U,
        s::V => K::V,
        s::W => K::W,
        s::X => K::X,
        s::Y => K::Y,
        s::Z => K::Z,
        s::F1 | s::KP_F1 => K::F1,
        s::F2 | s::KP_F2 => K::F2,
        s::F3 | s::KP_F3 => K::F3,
        s::F4 | s::KP_F4 => K::F4,
        s::F5 => K::F5,
        s::F6 => K::F6,
        s::F7 => K::F7,
        s::F8 => K::F8,
        s::F9 => K::F9,
        s::F10 => K::F10,
        s::F11 => K::F11,
        s::F12 => K::F12,
        s::F13 => K::F13,
        s::F14 => K::F14,
        s::F15 => K::F15,
        s::F16 => K::F16,
        s::F17 => K::F17,
        s::F18 => K::F18,
        s::F19 => K::F19,
        s::F20 => K::F20,
        s::F21 => K::F21,
        s::F22 => K::F22,
        s::F23 => K::F23,
        s::F24 => K::F24,
        s::F25 => K::F25,
        s::F26 => K::F26,
        s::F27 => K::F27,
        s::F28 => K::F28,
        s::F29 => K::F29,
        s::F30 => K::F30,
        s::F31 => K::F31,
        s::F32 => K::F32,
        s::F33 => K::F33,
        s::F34 => K::F34,
        s::F35 => K::F35,
        s::XF86Back => K::BrowserBack,
        _ => return None,
    };
    Some(key)
}

impl Drop for EggSeat {
    fn drop(&mut self) {
        let s = &self.inner;
        s.leave();
        s.unfocus();
        s.ctx.seats.remove(&s.global_name);
        s.ctx.con.remove_obj(&*s.wl_keyboard);
        s.ctx.con.remove_obj(&*s.wp_cursor_shape_device_v1);
        s.ctx.con.remove_obj(&*s.wl_pointer);
        s.ctx.con.remove_obj(&*s.wl_seat);
    }
}

impl Drop for EggContext {
    fn drop(&mut self) {
        let i = &self.inner;
        i.state.egg_state.cxts.remove(&self.inner.id);
        i.seats.clear();
        i.windows.clear();
        i.con.kill();
    }
}

impl Drop for EggWindow {
    fn drop(&mut self) {
        let i = &self.inner;
        i.ctx.windows.remove(&i.wl_surface.id);
        if let Some(seat) = i.active_seat.take() {
            for field in [&seat.kb_window, &seat.pointer_window] {
                if let Some(w) = field.get()
                    && rc_eq(&w, i)
                {
                    field.take();
                }
            }
        }
        i.owner.take();
        i.ctx.con.remove_obj(&*i.jay_sync_file_surface);
        i.ctx.con.remove_obj(&*i.xdg_toplevel);
        i.ctx.con.remove_obj(&*i.xdg_surface);
        i.ctx.con.remove_obj(&*i.wp_fractional_scale);
        i.ctx.con.remove_obj(&*i.wp_viewport);
        i.ctx.con.remove_obj(&*i.wl_surface);
    }
}
