use crate::async_engine::AsyncFd;
use crate::client::Client;
use crate::ifs::wl_surface::xwindow::{Xwindow, XwindowData};
use crate::ifs::wl_surface::WlSurface;
use crate::rect::Rect;
use crate::wire::WlSurfaceId;
use crate::xwayland::{XWaylandError, XWaylandEvent};
use crate::{AsyncQueue, ErrorFmt, State};
use ahash::AHashMap;
use futures::FutureExt;
use std::error::Error;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use uapi::OwnedFd;
use x11rb::atom_manager;
use x11rb::connection::Connection;
use x11rb::cursor::Handle;
use x11rb::errors::ConnectionError;
use x11rb::protocol::composite::{ConnectionExt as _, Redirect};
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ClientMessageEvent, ConfigureNotifyEvent, ConfigureRequestEvent,
    ConfigureWindowAux, ConnectionExt as _, CreateNotifyEvent, CreateWindowAux, DestroyNotifyEvent,
    EventMask, MapRequestEvent, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::resource_manager::Database;
use x11rb::rust_connection::{DefaultStream, RustConnection};

atom_manager! {
    pub Atoms: AtomsCookie {
        WL_SURFACE_ID,
        WM_DELETE_WINDOW,
        WM_PROTOCOLS,
        WM_HINTS,
        WM_NORMAL_HINTS,
        WM_SIZE_HINTS,
        WM_WINDOW_ROLE,
        MOTIF_WM_HINTS,
        UTF8_STRING,
        WM_S0,
        NET_SUPPORTED,
        NET_WM_CM_S0,
        NET_WM_PID,
        NET_WM_NAME,
        NET_WM_STATE,
        NET_WM_WINDOW_TYPE,
        WM_TAKE_FOCUS,
        WINDOW,
        NET_ACTIVE_WINDOW,
        NET_WM_MOVERESIZE,
        NET_SUPPORTING_WM_CHECK,
        NET_WM_STATE_FOCUSED,
        NET_WM_STATE_MODAL,
        NET_WM_STATE_FULLSCREEN,
        NET_WM_STATE_MAXIMIZED_VERT,
        NET_WM_STATE_MAXIMIZED_HORZ,
        NET_WM_STATE_HIDDEN,
        NET_WM_PING,
        WM_CHANGE_STATE,
        WM_STATE,
        CLIPBOARD,
        PRIMARY,
        WL_SELECTION,
        TARGETS,
        CLIPBOARD_MANAGER,
        INCR,
        TEXT,
        TIMESTAMP,
        DELETE,
        NET_STARTUP_ID,
        NET_STARTUP_INFO,
        NET_STARTUP_INFO_BEGIN,
        NET_WM_WINDOW_TYPE_NORMAL,
        NET_WM_WINDOW_TYPE_UTILITY,
        NET_WM_WINDOW_TYPE_TOOLTIP,
        NET_WM_WINDOW_TYPE_DND,
        NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
        NET_WM_WINDOW_TYPE_POPUP_MENU,
        NET_WM_WINDOW_TYPE_COMBO,
        NET_WM_WINDOW_TYPE_MENU,
        NET_WM_WINDOW_TYPE_NOTIFICATION,
        NET_WM_WINDOW_TYPE_SPLASH,
        DND_SELECTION,
        DND_AWARE,
        DND_STATUS,
        DND_POSITION,
        DND_ENTER,
        DND_LEAVE,
        DND_DROP,
        DND_FINISHED,
        DND_PROXY,
        DND_TYPE_LIST,
        DND_ACTION_MOVE,
        DND_ACTION_COPY,
        DND_ACTION_ASK,
        DND_ACTION_PRIVATE,
        NET_CLIENT_LIST,
        NET_CLIENT_LIST_STACKING,
    }
}

type Res<T> = Result<T, Box<dyn Error>>;

pub struct Wm {
    state: Rc<State>,
    c: RustConnection,
    atoms: Atoms,
    socket: AsyncFd,
    root: Window,
    xwin: Window,
    client: Rc<Client>,
    windows: AHashMap<Window, Rc<XwindowData>>,
    windows_by_surface_id: AHashMap<WlSurfaceId, Rc<XwindowData>>,
    queue: Rc<AsyncQueue<XWaylandEvent>>,
}

impl Drop for Wm {
    fn drop(&mut self) {
        for (_, window) in self.windows.drain() {
            if let Some(window) = window.window.take() {
                window.break_loops();
            }
        }
    }
}

impl Wm {
    pub(super) fn get(
        state: &Rc<State>,
        client: Rc<Client>,
        socket: OwnedFd,
        queue: Rc<AsyncQueue<XWaylandEvent>>,
    ) -> Result<Self, XWaylandError> {
        let socket_dup = match uapi::fcntl_dupfd_cloexec(socket.raw(), 0) {
            Ok(s) => state.eng.fd(&Rc::new(s))?,
            Err(e) => return Err(XWaylandError::Dupfd(e.into())),
        };
        let c = try {
            RustConnection::connect_to_stream(
                DefaultStream::from_unix_stream(unsafe {
                    UnixStream::from_raw_fd(socket.unwrap())
                })?,
                0,
            )?
        };
        let c: RustConnection = match c {
            Ok(c) => c,
            Err(e) => return Err(XWaylandError::Connect(e)),
        };
        let atoms: Atoms = match try { Atoms::new(&c)?.reply()? } {
            Ok(a) => a,
            Err(e) => return Err(XWaylandError::LoadAtoms(e)),
        };
        let root = c.setup().roots[0].root;
        {
            let cwa = ChangeWindowAttributesAux::new().event_mask(
                EventMask::SUBSTRUCTURE_NOTIFY
                    | EventMask::SUBSTRUCTURE_REDIRECT
                    | EventMask::PROPERTY_CHANGE,
            );
            let res = try { c.change_window_attributes(root, &cwa)?.check()? };
            if let Err(e) = res {
                return Err(XWaylandError::SelectRootEvents(e));
            }
        }
        {
            let res = try {
                c.composite_redirect_subwindows(root, Redirect::MANUAL)?
                    .check()?
            };
            if let Err(e) = res {
                return Err(XWaylandError::CompositeRedirectSubwindows(e));
            }
        }
        let xwin = c.generate_id().unwrap_or(0);
        {
            let res = try {
                c.create_window(
                    0,
                    xwin,
                    root,
                    0,
                    0,
                    10,
                    10,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    0,
                    &CreateWindowAux::new(),
                )?
                .check()?;
            };
            if let Err(e) = res {
                return Err(XWaylandError::CreateXWindow(e));
            }
        }
        {
            let res = try {
                c.set_selection_owner(xwin, atoms.WM_S0, 0u32)?.check()?;
            };
            if let Err(e) = res {
                return Err(XWaylandError::SelectionOwner(e));
            }
        }
        {
            let rdb = match Database::new_from_default(&c) {
                Ok(rdb) => rdb,
                Err(e) => return Err(XWaylandError::ResourceDatabase(e.into())),
            };
            let handle: Res<Handle> = try { Handle::new(&c, 0, &rdb)?.reply()? };
            let handle = match handle {
                Ok(h) => h,
                Err(e) => return Err(XWaylandError::CursorHandle(e)),
            };
            let cursor = match handle.load_cursor(&c, "left_ptr") {
                Ok(c) => c,
                Err(e) => return Err(XWaylandError::LoadCursor(e.into())),
            };
            let cwa = ChangeWindowAttributesAux::new().cursor(cursor);
            let res: Res<_> = try { c.change_window_attributes(root, &cwa)?.check()? };
            if let Err(e) = res {
                return Err(XWaylandError::SetCursor(e));
            }
        }
        Ok(Self {
            state: state.clone(),
            c,
            atoms,
            socket: socket_dup,
            root,
            xwin,
            client,
            windows: Default::default(),
            windows_by_surface_id: Default::default(),
            queue,
        })
    }

    pub async fn run(mut self) {
        loop {
            while let Some(e) = self.queue.try_pop() {
                self.handle_xwayland_event(e);
            }
            if let Err(e) = self.handle_events() {
                log::error!("Connection failed: {}", ErrorFmt(e));
                return;
            }
            futures::select! {
                res = self.socket.readable().fuse() => {
                    if let Err(e) = res {
                        log::error!("Cannot wait for xwm fd to become readable: {}", ErrorFmt(e));
                        return;
                    }
                }
                _ = self.queue.non_empty().fuse() => { },
            }
        }
    }

    fn handle_xwayland_event(&mut self, e: XWaylandEvent) {
        match e {
            XWaylandEvent::SurfaceCreated(event) => self.handle_xwayland_surface_created(event),
            XWaylandEvent::Configure(event) => self.handle_xwayland_configure(event),
            XWaylandEvent::SurfaceDestroyed(event) => self.handle_xwayland_surface_destroyed(event),
        }
    }

    fn handle_xwayland_configure(&mut self, window: Rc<Xwindow>) {
        self.send_configure(window);
    }

    fn send_configure(&mut self, window: Rc<Xwindow>) {
        let extents = window.data.extents.get();
        let cfg = ConfigureWindowAux::new()
            .x(extents.x1())
            .y(extents.y1())
            .width(extents.width() as u32)
            .height(extents.height() as u32)
            .border_width(0);
        let res: Res<()> = try {
            self.c
                .configure_window(window.data.window_id, &cfg)?
                .check()?;
        };
        if let Err(e) = res {
            log::error!("Could not configure window: {}", ErrorFmt(&*e));
        }
    }

    fn create_window(&mut self, data: &Rc<XwindowData>, surface: Rc<WlSurface>) {
        if data.window.get().is_some() {
            log::error!("The xwindow has already been constructed");
            return;
        }
        let window = Rc::new(Xwindow::new(&data, &surface, &self.queue));
        if let Err(e) = window.install() {
            log::error!(
                "Could not attach the xwindow to the surface: {}",
                ErrorFmt(e)
            );
            return;
        }
        data.window.set(Some(window.clone()));
        if surface.buffer.get().is_some() {
            self.state.map_tiled(window);
        }
    }

    fn handle_xwayland_surface_created(&mut self, surface: Rc<WlSurface>) {
        let data = match self.windows_by_surface_id.get(&surface.id) {
            Some(w) => w.clone(),
            _ => return,
        };
        self.create_window(&data, surface);
    }

    fn handle_xwayland_surface_destroyed(&mut self, surface: WlSurfaceId) {
        self.windows_by_surface_id.remove(&surface);
    }

    fn handle_events(&mut self) -> Result<(), ConnectionError> {
        while let Some(e) = self.c.poll_for_event()? {
            self.handle_event(e);
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        log::info!("{:?}", event);
        match event {
            Event::MapRequest(event) => self.handle_map_request(event),
            Event::ConfigureRequest(event) => self.handle_configure_request(event),
            Event::ConfigureNotify(event) => self.handle_configure_notify(event),
            Event::ClientMessage(event) => self.handle_client_message(event),
            Event::CreateNotify(event) => self.handle_create_notify(event),
            Event::DestroyNotify(event) => self.handle_destroy_notify(event),
            _ => {}
        }
    }

    fn handle_destroy_notify(&mut self, event: DestroyNotifyEvent) {
        let data = match self.windows.remove(&event.window) {
            Some(w) => w,
            _ => return,
        };
        if let Some(sid) = data.surface_id.take() {
            self.windows_by_surface_id.remove(&sid);
        }
        if let Some(window) = data.window.take() {
            window.destroy();
        }
    }

    fn handle_create_notify(&mut self, event: CreateNotifyEvent) {
        let data = Rc::new(XwindowData::new(&self.state, &event, &self.client));
        self.windows.insert(event.window, data);
    }

    fn handle_client_message(&mut self, event: ClientMessageEvent) {
        if event.type_ == self.atoms.WL_SURFACE_ID {
            self.handle_wl_surface_id(event);
        }
    }

    fn handle_map_request(&mut self, event: MapRequestEvent) {
        let res: Res<_> = try { self.c.map_window(event.window)?.check()? };
        if let Err(e) = res {
            log::error!("Could not map window: {}", ErrorFmt(&*e));
        }
    }

    fn handle_configure_notify(&mut self, event: ConfigureNotifyEvent) {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return,
        };
        if data.override_redirect {
            let extents = Rect::new_sized(
                event.x as _,
                event.y as _,
                event.width as _,
                event.height as _,
            )
            .unwrap();
            let changed = data.extents.replace(extents) != extents;
            if changed {
                self.state.tree_changed();
            }
        }
    }

    fn handle_configure_request(&mut self, event: ConfigureRequestEvent) {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return,
        };
        if let Some(w) = data.window.get() {
            self.send_configure(w);
        }
    }

    fn handle_wl_surface_id(&mut self, event: ClientMessageEvent) {
        let data = match self.windows.get(&event.window) {
            Some(d) => d.clone(),
            _ => return,
        };
        if data.surface_id.get().is_some() {
            log::error!("Surface id is already set");
            return;
        }
        let [surface_id, ..] = event.data.as_data32();
        let surface_id = WlSurfaceId::from_raw(surface_id);
        data.surface_id.set(Some(surface_id));
        self.windows_by_surface_id.insert(surface_id, data.clone());
        if let Ok(surface) = self.client.lookup(surface_id) {
            self.create_window(&data, surface);
        }
    }
}
