use crate::client::Client;
use crate::ifs::wl_surface::xwindow::{Xwindow, XwindowData};
use crate::ifs::wl_surface::WlSurface;
use crate::rect::Rect;
use crate::wire::WlSurfaceId;
use crate::wire_xcon::{
    ChangeWindowAttributes, ClientMessage, CompositeRedirectSubwindows, ConfigureNotify,
    ConfigureRequest, ConfigureWindow, ConfigureWindowValues, CreateGC, CreateNotify, CreatePixmap,
    CreateWindow, CreateWindowValues, DestroyNotify, FreeGC, FreePixmap, InternAtom, MapRequest,
    MapWindow, PutImage, RenderCreateCursor, RenderCreatePicture, SetSelectionOwner,
};
use crate::xcon::consts::{
    COMPOSITE_REDIRECT_MANUAL, EVENT_MASK_PROPERTY_CHANGE, EVENT_MASK_SUBSTRUCTURE_NOTIFY,
    EVENT_MASK_SUBSTRUCTURE_REDIRECT, IMAGE_FORMAT_Z_PIXMAP, WINDOW_CLASS_INPUT_OUTPUT,
};
use crate::xcon::{Event, XEvent, Xcon};
use crate::xwayland::{XWaylandError, XWaylandEvent};
use crate::{AsyncQueue, ErrorFmt, State};
use ahash::AHashMap;
use futures_util::{select, FutureExt};
use std::mem;
use std::rc::Rc;
use uapi::OwnedFd;

atom_manager! {
    Atoms;

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

pub struct Wm {
    state: Rc<State>,
    c: Rc<Xcon>,
    atoms: Atoms,
    _root: u32,
    _xwin: u32,
    client: Rc<Client>,
    windows: AHashMap<u32, Rc<XwindowData>>,
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
    pub(super) async fn get(
        state: &Rc<State>,
        client: Rc<Client>,
        socket: OwnedFd,
        queue: Rc<AsyncQueue<XWaylandEvent>>,
    ) -> Result<Self, XWaylandError> {
        let c = match Xcon::connect_to_fd(&state.eng, &Rc::new(socket), &[], &[]).await {
            Ok(c) => c,
            Err(e) => return Err(XWaylandError::Connect(e)),
        };
        let atoms = match Atoms::get(&c).await {
            Ok(a) => a,
            Err(e) => return Err(XWaylandError::LoadAtoms(e)),
        };
        let root = c.setup().screens[0].root;
        {
            let events = 0
                | EVENT_MASK_SUBSTRUCTURE_NOTIFY
                | EVENT_MASK_SUBSTRUCTURE_REDIRECT
                | EVENT_MASK_PROPERTY_CHANGE;
            let cwa = ChangeWindowAttributes {
                window: root,
                values: CreateWindowValues {
                    event_mask: Some(events),
                    ..Default::default()
                },
            };
            if let Err(e) = c.call(&cwa).await {
                return Err(XWaylandError::SelectRootEvents(e));
            }
        }
        {
            let crs = CompositeRedirectSubwindows {
                window: root,
                update: COMPOSITE_REDIRECT_MANUAL,
            };
            if let Err(e) = c.call(&crs).await {
                return Err(XWaylandError::CompositeRedirectSubwindows(e));
            }
        }
        let xwin = {
            let cw = CreateWindow {
                depth: 0,
                wid: c.generate_id()?,
                parent: root,
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                border_width: 0,
                class: WINDOW_CLASS_INPUT_OUTPUT,
                visual: 0,
                values: Default::default(),
            };
            if let Err(e) = c.call(&cw).await {
                return Err(XWaylandError::CreateXWindow(e));
            }
            cw.wid
        };
        {
            let sso = SetSelectionOwner {
                owner: xwin,
                selection: atoms.WM_S0,
                time: 0,
            };
            if let Err(e) = c.call(&sso).await {
                return Err(XWaylandError::SelectionOwner(e));
            }
        }
        'set_root_cursor: {
            let cursor_format = c.find_cursor_format().await?;
            let cursors = match state.cursors.get() {
                Some(g) => g,
                _ => break 'set_root_cursor,
            };
            let first = match cursors.default.xcursor.first() {
                Some(f) => f,
                _ => break 'set_root_cursor,
            };
            let pixmap = c.generate_id()?;
            let gc = c.generate_id()?;
            let picture = c.generate_id()?;
            let cursor = c.generate_id()?;
            let create_pixmap = c.call(&CreatePixmap {
                depth: 32,
                pid: pixmap,
                drawable: root,
                width: first.width as _,
                height: first.height as _,
            });
            let create_gc = c.call(&CreateGC {
                cid: gc,
                drawable: pixmap,
                values: Default::default(),
            });
            let put_image = c.call(&PutImage {
                format: IMAGE_FORMAT_Z_PIXMAP,
                drawable: pixmap,
                gc,
                width: first.width as _,
                height: first.height as _,
                dst_x: 0,
                dst_y: 0,
                left_pad: 0,
                depth: 32,
                data: unsafe { mem::transmute(&first.pixels[..]) },
            });
            c.call(&FreeGC { gc });
            let create_picture = c.call(&RenderCreatePicture {
                pid: picture,
                drawable: pixmap,
                format: cursor_format,
                values: Default::default(),
            });
            c.call(&FreePixmap { pixmap });
            let create_cursor = c.call(&RenderCreateCursor {
                cid: cursor,
                source: picture,
                x: first.xhot as _,
                y: first.yhot as _,
            });
            if let Err(e) = create_pixmap.await {
                log::warn!(
                    "Could not create a pixmap for the root cursor: {}",
                    ErrorFmt(e)
                );
                break 'set_root_cursor;
            }
            if let Err(e) = create_gc.await {
                log::warn!(
                    "Could not create a graphics context for the root cursor: {}",
                    ErrorFmt(e)
                );
                break 'set_root_cursor;
            }
            if let Err(e) = put_image.await {
                log::warn!(
                    "Could not upload the image for the root cursor: {}",
                    ErrorFmt(e)
                );
                break 'set_root_cursor;
            }
            if let Err(e) = create_picture.await {
                log::warn!(
                    "Could not create a picture for the root cursor: {}",
                    ErrorFmt(e)
                );
                break 'set_root_cursor;
            }
            if let Err(e) = create_cursor.await {
                log::warn!("Could not create the root cursor: {}", ErrorFmt(e));
                break 'set_root_cursor;
            }
            let cwa = ChangeWindowAttributes {
                window: root,
                values: CreateWindowValues {
                    cursor: Some(cursor),
                    ..Default::default()
                },
            };
            if let Err(e) = c.call(&cwa).await {
                return Err(XWaylandError::SetCursor(e));
            }
        }
        Ok(Self {
            state: state.clone(),
            c,
            atoms,
            _root: root,
            _xwin: xwin,
            client,
            windows: Default::default(),
            windows_by_surface_id: Default::default(),
            queue,
        })
    }

    pub async fn run(mut self) {
        loop {
            select! {
                e = self.queue.pop().fuse() => self.handle_xwayland_event(e).await,
                e = self.c.event().fuse() => self.handle_event(&e).await,
            }
        }
    }

    async fn handle_xwayland_event(&mut self, e: XWaylandEvent) {
        match e {
            XWaylandEvent::SurfaceCreated(event) => self.handle_xwayland_surface_created(event),
            XWaylandEvent::Configure(event) => self.handle_xwayland_configure(event).await,
            XWaylandEvent::SurfaceDestroyed(event) => self.handle_xwayland_surface_destroyed(event),
        }
    }

    async fn handle_xwayland_configure(&mut self, window: Rc<Xwindow>) {
        self.send_configure(window).await;
    }

    async fn send_configure(&mut self, window: Rc<Xwindow>) {
        let extents = window.data.extents.get();
        let cw = ConfigureWindow {
            window: window.data.window_id,
            values: ConfigureWindowValues {
                x: Some(extents.x1()),
                y: Some(extents.y1()),
                width: Some(extents.width() as u32),
                height: Some(extents.height() as u32),
                border_width: Some(0),
                ..Default::default()
            },
        };
        if let Err(e) = self.c.call(&cw).await {
            log::error!("Could not configure window: {}", ErrorFmt(e));
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

    async fn handle_event(&mut self, event: &Event) {
        match event.ext() {
            Some(_) => {}
            _ => self.handle_core_event(&event).await,
        }
    }

    async fn handle_core_event(&mut self, event: &Event) {
        let res = match event.code() {
            MapRequest::OPCODE => self.handle_map_request(event).await,
            ConfigureRequest::OPCODE => self.handle_configure_request(event).await,
            ConfigureNotify::OPCODE => self.handle_configure_notify(event),
            ClientMessage::OPCODE => self.handle_client_message(event),
            CreateNotify::OPCODE => self.handle_create_notify(event),
            DestroyNotify::OPCODE => self.handle_destroy_notify(event),
            _ => Ok(()),
        };
        if let Err(e) = res {
            log::warn!("Could not handle an event: {}", ErrorFmt(e));
        }
    }

    fn handle_destroy_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: DestroyNotify = event.parse()?;
        let data = match self.windows.remove(&event.window) {
            Some(w) => w,
            _ => return Ok(()),
        };
        if let Some(sid) = data.surface_id.take() {
            self.windows_by_surface_id.remove(&sid);
        }
        if let Some(window) = data.window.take() {
            window.destroy();
        }
        Ok(())
    }

    fn handle_create_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: CreateNotify = event.parse()?;
        let data = Rc::new(XwindowData::new(&self.state, &event, &self.client));
        self.windows.insert(event.window, data);
        Ok(())
    }

    fn handle_client_message(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: ClientMessage = event.parse()?;
        if event.ty == self.atoms.WL_SURFACE_ID {
            self.handle_wl_surface_id(&event)?;
        }
        Ok(())
    }

    async fn handle_map_request(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: MapRequest = event.parse()?;
        let mw = MapWindow {
            window: event.window,
        };
        match self.c.call(&mw).await {
            Ok(_) => Ok(()),
            Err(e) => Err(XWaylandError::MapWindow(e)),
        }
    }

    fn handle_configure_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: ConfigureNotify = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
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
        Ok(())
    }

    async fn handle_configure_request(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: ConfigureRequest = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        if let Some(w) = data.window.get() {
            self.send_configure(w).await;
        }
        Ok(())
    }

    fn handle_wl_surface_id(&mut self, event: &ClientMessage) -> Result<(), XWaylandError> {
        let data = match self.windows.get(&event.window) {
            Some(d) => d.clone(),
            _ => return Ok(()),
        };
        if data.surface_id.get().is_some() {
            log::error!("Surface id is already set");
            return Ok(());
        }
        let surface_id = event.data[0];
        let surface_id = WlSurfaceId::from_raw(surface_id);
        data.surface_id.set(Some(surface_id));
        self.windows_by_surface_id.insert(surface_id, data.clone());
        if let Ok(surface) = self.client.lookup(surface_id) {
            self.create_window(&data, surface);
        }
        Ok(())
    }
}
