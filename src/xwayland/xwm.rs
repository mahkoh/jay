use {
    crate::{
        client::Client,
        ifs::wl_surface::{
            xwindow::{XInputModel, Xwindow, XwindowData},
            WlSurface,
        },
        rect::Rect,
        state::State,
        tree::{Node, SizedNode},
        utils::{
            bitflags::BitflagsExt, errorfmt::ErrorFmt, linkedlist::LinkedList, queue::AsyncQueue,
        },
        wire::WlSurfaceId,
        wire_xcon::{
            ChangeProperty, ChangeWindowAttributes, ClientMessage, CompositeRedirectSubwindows,
            ConfigureNotify, ConfigureRequest, ConfigureWindow, ConfigureWindowValues,
            CreateNotify, CreateWindow, CreateWindowValues, DestroyNotify, FocusIn, GetAtomName,
            GetGeometry, InternAtom, KillClient, MapNotify, MapRequest, MapWindow, PropertyNotify,
            ResClientIdSpec, ResQueryClientIds, SetInputFocus, SetSelectionOwner, UnmapNotify,
        },
        xcon::{
            consts::{
                ATOM_ATOM, ATOM_STRING, ATOM_WINDOW, ATOM_WM_CLASS, ATOM_WM_NAME,
                ATOM_WM_SIZE_HINTS, ATOM_WM_TRANSIENT_FOR, COMPOSITE_REDIRECT_MANUAL,
                CONFIG_WINDOW_HEIGHT, CONFIG_WINDOW_WIDTH, CONFIG_WINDOW_X, CONFIG_WINDOW_Y,
                EVENT_MASK_FOCUS_CHANGE, EVENT_MASK_PROPERTY_CHANGE,
                EVENT_MASK_SUBSTRUCTURE_NOTIFY, EVENT_MASK_SUBSTRUCTURE_REDIRECT,
                ICCCM_WM_HINT_INPUT, ICCCM_WM_STATE_ICONIC, ICCCM_WM_STATE_NORMAL,
                ICCCM_WM_STATE_WITHDRAWN, INPUT_FOCUS_POINTER_ROOT, MWM_HINTS_DECORATIONS_FIELD,
                MWM_HINTS_FLAGS_FIELD, NOTIFY_DETAIL_POINTER, NOTIFY_MODE_GRAB, NOTIFY_MODE_UNGRAB,
                PROP_MODE_REPLACE, RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID, STACK_MODE_ABOVE,
                STACK_MODE_BELOW, WINDOW_CLASS_INPUT_OUTPUT, _NET_WM_STATE_ADD,
                _NET_WM_STATE_REMOVE, _NET_WM_STATE_TOGGLE,
            },
            Event, XEvent, Xcon, XconError,
        },
        xwayland::{XWaylandError, XWaylandEvent},
    },
    ahash::{AHashMap, AHashSet},
    bstr::ByteSlice,
    futures_util::{select, FutureExt},
    smallvec::SmallVec,
    std::{
        borrow::Cow,
        mem,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    uapi::OwnedFd,
};

atoms! {
    Atoms;

    CLIPBOARD,
    CLIPBOARD_MANAGER,
    COMPOUND_TEXT,
    DELETE,
    INCR,
    _MOTIF_WM_HINTS,
    _NET_ACTIVE_WINDOW,
    _NET_CLIENT_LIST,
    _NET_CLIENT_LIST_STACKING,
    _NET_STARTUP_ID,
    _NET_STARTUP_INFO,
    _NET_STARTUP_INFO_BEGIN,
    _NET_SUPPORTED,
    _NET_SUPPORTING_WM_CHECK,
    _NET_WM_CM_S0,
    _NET_WM_MOVERESIZE,
    _NET_WM_NAME,
    _NET_WM_PID,
    _NET_WM_PING,
    _NET_WM_STATE,
    _NET_WM_STATE_FOCUSED,
    _NET_WM_STATE_FULLSCREEN,
    _NET_WM_STATE_HIDDEN,
    _NET_WM_STATE_MAXIMIZED_HORZ,
    _NET_WM_STATE_MAXIMIZED_VERT,
    _NET_WM_STATE_MODAL,
    _NET_WM_WINDOW_TYPE,
    _NET_WM_WINDOW_TYPE_COMBO,
    _NET_WM_WINDOW_TYPE_DIALOG,
    _NET_WM_WINDOW_TYPE_DND,
    _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
    _NET_WM_WINDOW_TYPE_MENU,
    _NET_WM_WINDOW_TYPE_NORMAL,
    _NET_WM_WINDOW_TYPE_NOTIFICATION,
    _NET_WM_WINDOW_TYPE_POPUP_MENU,
    _NET_WM_WINDOW_TYPE_SPLASH,
    _NET_WM_WINDOW_TYPE_TOOLBAR,
    _NET_WM_WINDOW_TYPE_TOOLTIP,
    _NET_WM_WINDOW_TYPE_UTILITY,
    PRIMARY,
    TARGETS,
    TEXT,
    TIMESTAMP,
    UTF8_STRING,
    WINDOW,
    _WL_SELECTION,
    WL_SURFACE_ID,
    WM_CHANGE_STATE,
    WM_DELETE_WINDOW,
    WM_HINTS,
    WM_NORMAL_HINTS,
    WM_PROTOCOLS,
    WM_S0,
    WM_SIZE_HINTS,
    WM_STATE,
    WM_TAKE_FOCUS,
    WM_WINDOW_ROLE,
    XdndActionAsk,
    XdndActionCopy,
    XdndActionMove,
    XdndActionPrivate,
    XdndAware,
    XdndDrop,
    XdndEnter,
    XdndFinished,
    XdndLeave,
    XdndPosition,
    XdndProxy,
    XdndSelection,
    XdndStatus,
    XdndTypeList,
}

pub struct Wm {
    state: Rc<State>,
    c: Rc<Xcon>,
    atoms: Atoms,
    never_focus: AHashSet<u32>,
    root: u32,
    xwin: u32,
    client: Rc<Client>,
    windows: AHashMap<u32, Rc<XwindowData>>,
    windows_by_surface_id: AHashMap<WlSurfaceId, Rc<XwindowData>>,
    queue: Rc<AsyncQueue<XWaylandEvent>>,
    focus_window: Option<Rc<XwindowData>>,
    last_input_serial: u64,

    stack_list: LinkedList<Rc<XwindowData>>,
    num_stacked: usize,

    map_list: LinkedList<Rc<XwindowData>>,
    num_mapped: usize,
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
        let never_focus = {
            let mut nf = AHashSet::new();
            nf.insert(atoms._NET_WM_WINDOW_TYPE_COMBO);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_DND);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_DROPDOWN_MENU);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_MENU);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_NOTIFICATION);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_POPUP_MENU);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_SPLASH);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_TOOLTIP);
            nf.insert(atoms._NET_WM_WINDOW_TYPE_UTILITY);
            nf
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
            let xwin = c.generate_id()?;
            let cw = CreateWindow {
                depth: 0,
                wid: xwin,
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
            c.call(&ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: xwin,
                property: atoms._NET_WM_NAME,
                ty: atoms.UTF8_STRING,
                format: 8,
                data: "jay wm".as_bytes(),
            });
            c.call(&ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: root,
                property: atoms._NET_SUPPORTING_WM_CHECK,
                ty: ATOM_WINDOW,
                format: 32,
                data: uapi::as_bytes(&xwin),
            });
            c.call(&ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: xwin,
                property: atoms._NET_SUPPORTING_WM_CHECK,
                ty: ATOM_WINDOW,
                format: 32,
                data: uapi::as_bytes(&xwin),
            });
            c.call(&SetSelectionOwner {
                owner: xwin,
                selection: atoms.WM_S0,
                time: 0,
            });
            c.call(&SetSelectionOwner {
                owner: xwin,
                selection: atoms._NET_WM_CM_S0,
                time: 0,
            });
            xwin
        };
        {
            let supported_atoms = [
                atoms._NET_WM_STATE,
                atoms._NET_ACTIVE_WINDOW,
                atoms._NET_WM_MOVERESIZE,
                atoms._NET_WM_STATE_FOCUSED,
                atoms._NET_WM_STATE_MODAL,
                atoms._NET_WM_STATE_FULLSCREEN,
                atoms._NET_WM_STATE_MAXIMIZED_VERT,
                atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                atoms._NET_WM_STATE_HIDDEN,
                atoms._NET_CLIENT_LIST,
                atoms._NET_CLIENT_LIST_STACKING,
            ];
            c.call(&ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: root,
                property: atoms._NET_SUPPORTED,
                ty: ATOM_ATOM,
                format: 32,
                data: uapi::as_bytes(&supported_atoms[..]),
            });
        }
        {
            c.call(&ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: root,
                property: atoms._NET_ACTIVE_WINDOW,
                ty: ATOM_ATOM,
                format: 32,
                data: uapi::as_bytes(&0u32),
            });
        }
        'set_root_cursor: {
            let cursors = match state.cursors.get() {
                Some(g) => g,
                _ => break 'set_root_cursor,
            };
            let first = match cursors.default.xcursor.first() {
                Some(f) => f,
                _ => break 'set_root_cursor,
            };
            let cursor = match c
                .create_cursor(
                    &first.pixels,
                    first.width,
                    first.height,
                    first.xhot,
                    first.yhot,
                )
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Could not create a root cursor: {}", ErrorFmt(e));
                    break 'set_root_cursor;
                }
            };
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
            never_focus,
            root,
            xwin,
            client,
            windows: Default::default(),
            windows_by_surface_id: Default::default(),
            queue,
            focus_window: Default::default(),
            last_input_serial: 0,
            stack_list: Default::default(),
            num_stacked: 0,
            map_list: Default::default(),
            num_mapped: 0,
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
            XWaylandEvent::SurfaceCreated(event) => {
                self.handle_xwayland_surface_created(event).await
            }
            XWaylandEvent::Configure(event) => self.handle_xwayland_configure(event).await,
            XWaylandEvent::SurfaceDestroyed(event) => self.handle_xwayland_surface_destroyed(event),
            XWaylandEvent::Activate(window) => self.activate_window(Some(&window)).await,
            XWaylandEvent::Close(window) => self.close_window(&window).await,
        }
    }

    async fn handle_xwayland_configure(&mut self, window: Rc<Xwindow>) {
        if window.data.destroyed.get() {
            return;
        }
        self.send_configure(window).await;
    }

    async fn send_configure(&mut self, window: Rc<Xwindow>) {
        let extents = window.data.info.extents.get();
        // log::info!("xwin {} send_configure {:?}", window.data.window_id, extents);
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

    async fn set_minimized(&self, data: &Rc<XwindowData>, minimized: bool) {
        data.info.minimized.set(minimized);
        let state = match minimized {
            true => ICCCM_WM_STATE_ICONIC,
            false => ICCCM_WM_STATE_NORMAL,
        };
        self.set_wm_state(data, state).await;
        self.set_net_wm_state(data).await;
    }

    async fn set_maximized(&self, data: &Rc<XwindowData>, maximized: bool) {
        data.info.maximized_vert.set(maximized);
        data.info.maximized_horz.set(maximized);
        self.set_net_wm_state(data).await;
    }

    async fn set_fullscreen(&self, data: &Rc<XwindowData>, fullscreen: bool) {
        data.info.fullscreen.set(fullscreen);
        self.set_net_wm_state(data).await;
    }

    async fn send_wm_message(&self, window: &Rc<XwindowData>, event_mask: u32, data: &[u32]) {
        let event = ClientMessage {
            format: 32,
            window: window.window_id,
            ty: self.atoms.WM_PROTOCOLS,
            data,
        };
        if let Err(e) = self
            .c
            .send_event(false, window.window_id, event_mask, &event)
            .await
        {
            log::error!("Could not send WM_PROTOCOLS message: {}", ErrorFmt(e));
        }
    }

    async fn focus_window(&mut self, window: Option<&Rc<XwindowData>>) {
        log::info!("xwm focus_window {:?}", window.map(|w| w.window_id));
        if let Some(old) = mem::replace(&mut self.focus_window, window.cloned()) {
            log::info!("xwm unfocus {:?}", old.window_id);
            self.set_net_wm_state(&old).await;
        }
        let window = match window {
            Some(w) => w,
            _ => {
                if let Err(e) = self
                    .c
                    .call(&SetInputFocus {
                        revert_to: INPUT_FOCUS_POINTER_ROOT,
                        focus: 0,
                        time: 0,
                    })
                    .await
                {
                    log::error!("Could not unset pointer focus: {}", ErrorFmt(e));
                }
                return;
            }
        };
        if window.info.override_redirect.get() {
            log::info!("xwm or => return");
            return;
        }
        if let Some(window) = window.window.get() {
            let seats = self.state.globals.seats.lock();
            for seat in seats.values() {
                seat.focus_toplevel(window.clone());
            }
        }
        let accepts_input = window.info.icccm_hints.input.get();
        let mask = if accepts_input {
            EVENT_MASK_SUBSTRUCTURE_REDIRECT
        } else {
            0
        };
        self.send_wm_message(window, mask, &[self.atoms.WM_TAKE_FOCUS, 0])
            .await;
        if accepts_input {
            let sif = SetInputFocus {
                revert_to: INPUT_FOCUS_POINTER_ROOT,
                focus: window.window_id,
                time: 0,
            };
            let (_, serial) = self.c.call_with_serial(&sif);
            self.last_input_serial = serial;
        }
        self.set_net_wm_state(window).await;
    }

    async fn set_net_wm_state(&self, data: &Rc<XwindowData>) {
        let mut args = SmallVec::<[_; 6]>::new();
        if data.info.modal.get() {
            args.push(self.atoms._NET_WM_STATE_MODAL);
        }
        if data.info.fullscreen.get() {
            args.push(self.atoms._NET_WM_STATE_FULLSCREEN);
        }
        if data.info.maximized_vert.get() {
            args.push(self.atoms._NET_WM_STATE_MAXIMIZED_VERT);
        }
        if data.info.maximized_horz.get() {
            args.push(self.atoms._NET_WM_STATE_MAXIMIZED_HORZ);
        }
        if data.info.minimized.get() {
            args.push(self.atoms._NET_WM_STATE_HIDDEN);
        }
        if Some(data.window_id) == self.focus_window.as_ref().map(|w| w.window_id) {
            args.push(self.atoms._NET_WM_STATE_FOCUSED);
        }
        let cp = ChangeProperty {
            mode: PROP_MODE_REPLACE,
            window: data.window_id,
            property: self.atoms._NET_WM_STATE,
            ty: ATOM_ATOM,
            format: 32,
            data: uapi::as_bytes(&args[..]),
        };
        if let Err(e) = self.c.call(&cp).await {
            log::error!("Could not set _NET_WM_STATE: {}", ErrorFmt(e));
        }
    }

    fn compute_input_model(&self, data: &Rc<XwindowData>) {
        let has_wm_take_focus = data.info.protocols.contains(&self.atoms.WM_TAKE_FOCUS);
        let accepts_input = data.info.icccm_hints.input.get();
        let model = match (accepts_input, has_wm_take_focus) {
            (false, false) => XInputModel::None,
            (true, false) => XInputModel::Passive,
            (true, true) => XInputModel::Local,
            (false, true) => XInputModel::Global,
        };
        data.info.input_model.set(model);
    }

    async fn load_window_wm_window_role(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        match self
            .c
            .get_property::<u8>(data.window_id, self.atoms.WM_WINDOW_ROLE, 0, &mut buf)
            .await
        {
            Ok(ty) if ty == ATOM_STRING => {}
            Ok(ty) if ty == self.atoms.UTF8_STRING => {}
            Ok(ty) => {
                self.unexpected_type(data.window_id, "WM_WINDOW_ROLE", ty)
                    .await;
                return;
            }
            Err(XconError::PropertyUnavailable) => {
                data.info.role.borrow_mut().take();
                return;
            }
            Err(e) => {
                log::error!(
                    "Could not retrieve WM_WINDOW_ROLE property: {}",
                    ErrorFmt(e)
                );
                return;
            }
        }
        log::info!("{} role {}", data.window_id, buf.as_bstr());
        *data.info.role.borrow_mut() = Some(buf.into());
    }

    async fn load_window_wm_class(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        match self
            .c
            .get_property::<u8>(data.window_id, ATOM_WM_CLASS, 0, &mut buf)
            .await
        {
            Ok(ty) if ty == ATOM_STRING => {}
            Ok(ty) if ty == self.atoms.UTF8_STRING => {}
            Ok(ty) => {
                self.unexpected_type(data.window_id, "WM_CLASS", ty).await;
                return;
            }
            Err(XconError::PropertyUnavailable) => {
                data.info.instance.borrow_mut().take();
                data.info.class.borrow_mut().take();
                return;
            }
            Err(e) => {
                log::error!("Could not retrieve WM_CLASS property: {}", ErrorFmt(e));
                return;
            }
        }
        let mut iter = buf.split(|c| *c == 0);
        *data.info.instance.borrow_mut() = Some(iter.next().unwrap_or(&[]).to_vec().into());
        *data.info.class.borrow_mut() = Some(iter.next().unwrap_or(&[]).to_vec().into());
    }

    async fn load_window_wm_name2(&self, data: &Rc<XwindowData>, prop: u32, name: &str) {
        let mut buf = vec![];
        match self
            .c
            .get_property::<u8>(data.window_id, prop, 0, &mut buf)
            .await
        {
            Ok(ty) if ty == ATOM_STRING && data.info.utf8_title.get() => return,
            Ok(ty) if ty == ATOM_STRING => {}
            Ok(ty) if ty == self.atoms.COMPOUND_TEXT => return, // used by java.
            Ok(ty) if ty == self.atoms.UTF8_STRING => {
                data.info.utf8_title.set(true);
            }
            Ok(ty) => {
                self.unexpected_type(data.window_id, name, ty).await;
                return;
            }
            Err(XconError::PropertyUnavailable) => return,
            Err(e) => {
                log::error!("Could not retrieve {} property: {}", name, ErrorFmt(e));
                return;
            }
        }
        *data.info.title.borrow_mut() = Some(buf.as_bstr().to_string());
        data.title_changed();
    }

    async fn unexpected_type(&self, window: u32, prop: &str, ty: u32) {
        let mut ty_name = "unknown".as_bytes().as_bstr();
        let res = self.c.call(&GetAtomName { atom: ty }).await;
        if let Ok(res) = &res {
            ty_name = res.get().name;
        }
        log::error!(
            "Property {} of window {} has unexpected type {} ({})",
            prop,
            window,
            ty_name,
            ty
        );
    }

    async fn load_window_wm_name(&self, data: &Rc<XwindowData>) {
        self.load_window_wm_name2(data, ATOM_WM_NAME, "WM_NAME")
            .await;
    }

    async fn load_window_net_wm_name(&self, data: &Rc<XwindowData>) {
        self.load_window_wm_name2(data, self.atoms._NET_WM_NAME, "_NET_WM_NAME")
            .await;
    }

    async fn load_window_wm_transient_for(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(data.window_id, ATOM_WM_TRANSIENT_FOR, ATOM_WINDOW, &mut buf)
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!(
                    "Could not retrieve WM_TRANSIENT_FOR property: {}",
                    ErrorFmt(e)
                );
            }
        }
        if let Some(old) = data.parent.take() {
            old.children.remove(&data.window_id);
        }
        if let Some(w) = buf.first() {
            if let Some(w) = self.windows.get(w) {
                if data.is_ancestor_of(w.clone()) {
                    log::error!("Cannot set WM_TRANSIENT_FOR because it would create a cycle");
                    return;
                }
                w.children.set(data.window_id, data.clone());
                data.parent.set(Some(w.clone()));
            }
        }
    }

    async fn load_window_wm_protocols(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(data.window_id, self.atoms.WM_PROTOCOLS, ATOM_ATOM, &mut buf)
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!("Could not retrieve WM_PROTOCOLS property: {}", ErrorFmt(e));
            }
            return;
        }
        data.info.protocols.clear();
        data.info
            .protocols
            .lock()
            .extend(buf.iter().copied().map(|v| (v, ())));
        self.compute_input_model(data);
    }

    async fn load_window_wm_hints(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(data.window_id, self.atoms.WM_HINTS, 0, &mut buf)
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!("Could not retrieve WM_HINTS property: {}", ErrorFmt(e));
            }
            data.info.icccm_hints.input.set(true);
            self.compute_input_model(data);
            return;
        }
        let mut values = [0; 9];
        let len = values.len().min(buf.len());
        values[..len].copy_from_slice(&buf[..len]);
        data.info.icccm_hints.flags.set(values[0] as i32);
        data.info.icccm_hints.input.set(values[1] != 0);
        data.info.icccm_hints.initial_state.set(values[2] as i32);
        data.info.icccm_hints.icon_pixmap.set(values[3]);
        data.info.icccm_hints.icon_window.set(values[4]);
        data.info.icccm_hints.icon_x.set(values[5] as i32);
        data.info.icccm_hints.icon_y.set(values[6] as i32);
        data.info.icccm_hints.icon_mask.set(values[7]);
        data.info.icccm_hints.window_group.set(values[8]);
        if data
            .info
            .icccm_hints
            .flags
            .get()
            .not_contains(ICCCM_WM_HINT_INPUT)
        {
            data.info.icccm_hints.input.set(true);
        }
        self.compute_input_model(data);
    }

    async fn load_window_wm_normal_hints(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(
                data.window_id,
                self.atoms.WM_NORMAL_HINTS,
                ATOM_WM_SIZE_HINTS,
                &mut buf,
            )
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!(
                    "Could not retrieve WM_NORMAL_HINTS property: {}",
                    ErrorFmt(e)
                );
            }
            return;
        }
        let mut values = [0; 18];
        let len = values.len().min(buf.len());
        values[..len].copy_from_slice(&buf[..len]);
        data.info.normal_hints.flags.set(values[0]);
        data.info.normal_hints.x.set(values[1] as i32);
        data.info.normal_hints.y.set(values[2] as i32);
        data.info.normal_hints.width.set(values[3] as i32);
        data.info.normal_hints.height.set(values[4] as i32);
        data.info.normal_hints.min_width.set(values[5] as i32);
        data.info.normal_hints.min_height.set(values[6] as i32);
        data.info.normal_hints.max_width.set(values[7] as i32);
        data.info.normal_hints.max_height.set(values[8] as i32);
        data.info.normal_hints.width_inc.set(values[9] as i32);
        data.info.normal_hints.height_inc.set(values[10] as i32);
        data.info.normal_hints.min_aspect_num.set(values[11] as i32);
        data.info.normal_hints.min_aspect_den.set(values[12] as i32);
        data.info.normal_hints.max_aspect_num.set(values[13] as i32);
        data.info.normal_hints.max_aspect_den.set(values[14] as i32);
        data.info.normal_hints.base_width.set(values[15] as i32);
        data.info.normal_hints.base_height.set(values[16] as i32);
        data.info.normal_hints.win_gravity.set(values[17]);
        self.update_wants_floating(data);
    }

    async fn load_window_motif_wm_hints(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(data.window_id, self.atoms._MOTIF_WM_HINTS, 0, &mut buf)
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!(
                    "Could not retrieve _MOTIF_WM_HINTS property: {}",
                    ErrorFmt(e)
                );
            }
            return;
        }
        let mut values = [0; 5];
        let len = values.len().min(buf.len());
        values[..len].copy_from_slice(&buf[..len]);
        data.info
            .motif_hints
            .flags
            .set(values[MWM_HINTS_FLAGS_FIELD]);
        data.info
            .motif_hints
            .decorations
            .set(values[MWM_HINTS_DECORATIONS_FIELD]);
    }

    async fn load_window_net_startup_id(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        match self
            .c
            .get_property::<u8>(data.window_id, self.atoms._NET_STARTUP_ID, 0, &mut buf)
            .await
        {
            Ok(ty) if ty == ATOM_STRING => {}
            Ok(ty) if ty == self.atoms.UTF8_STRING => {}
            Ok(ty) => {
                self.unexpected_type(data.window_id, "_NET_STARTUP_ID", ty)
                    .await;
                return;
            }
            Err(XconError::PropertyUnavailable) => return,
            Err(e) => {
                log::error!(
                    "Could not retrieve _NET_STARTUP_ID property: {}",
                    ErrorFmt(e)
                );
                return;
            }
        }
        *data.info.startup_id.borrow_mut() = Some(buf.into());
    }

    async fn load_window_net_wm_state(&self, data: &Rc<XwindowData>) {
        data.info.fullscreen.set(false);
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(data.window_id, self.atoms._NET_WM_STATE, 0, &mut buf)
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!("Could not retrieve _NET_WM_STATE property: {}", ErrorFmt(e));
            }
            return;
        }
        for prop in buf {
            if prop == self.atoms._NET_WM_STATE_MODAL {
                data.info.modal.set(true);
                self.update_wants_floating(data);
            } else if prop == self.atoms._NET_WM_STATE_FULLSCREEN {
                data.info.fullscreen.set(true);
            } else if prop == self.atoms._NET_WM_STATE_MAXIMIZED_VERT {
                data.info.maximized_vert.set(true);
            } else if prop == self.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                data.info.maximized_horz.set(true);
            } else if prop == self.atoms._NET_WM_STATE_HIDDEN {
                data.info.minimized.set(true);
            }
        }
    }

    async fn load_window_net_wm_window_type(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        if let Err(e) = self
            .c
            .get_property::<u32>(
                data.window_id,
                self.atoms._NET_WM_WINDOW_TYPE,
                ATOM_ATOM,
                &mut buf,
            )
            .await
        {
            if !matches!(e, XconError::PropertyUnavailable) {
                log::error!(
                    "Could not retrieve _NET_WM_WINDOW_TYPE property: {}",
                    ErrorFmt(e)
                );
            }
            return;
        }
        data.info
            .never_focus
            .set(buf.iter().any(|t| self.never_focus.contains(t)));
        data.info.window_types.clear();
        data.info
            .window_types
            .lock()
            .extend(buf.iter().copied().map(|v| (v, ())));
        self.update_wants_floating(data);
    }

    async fn create_window(&mut self, data: &Rc<XwindowData>, surface: Rc<WlSurface>) {
        if data.window.get().is_some() {
            log::error!("The xwindow has already been constructed");
            return;
        }
        let window = Rc::new(Xwindow::new(data, &surface, &self.queue));
        if let Err(e) = window.install() {
            log::error!(
                "Could not attach the xwindow to the surface: {}",
                ErrorFmt(e)
            );
            return;
        }
        data.window.set(Some(window.clone()));
        {
            self.load_window_wm_class(data).await;
            self.load_window_wm_name(data).await;
            self.load_window_wm_transient_for(data).await;
            self.load_window_wm_protocols(data).await;
            self.load_window_wm_hints(data).await;
            self.load_window_wm_normal_hints(data).await;
            self.load_window_motif_wm_hints(data).await;
            self.load_window_net_startup_id(data).await;
            self.load_window_net_wm_state(data).await;
            self.load_window_net_wm_window_type(data).await;
            self.load_window_net_wm_name(data).await;
            self.load_window_wm_window_role(data).await;
        }
        {
            let specs = [ResClientIdSpec {
                client: data.window_id,
                mask: RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID,
            }];
            let c = ResQueryClientIds {
                specs: Cow::Borrowed(&specs),
            };
            if let Ok(res) = self.c.call(&c).await {
                for id in res.get().ids.iter() {
                    if id.spec.mask.contains(RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID) {
                        if let Some(first) = id.value.first() {
                            data.info.pid.set(Some(*first));
                            break;
                        }
                    }
                }
            }
        }
        window.map_status_changed();
    }

    async fn handle_xwayland_surface_created(&mut self, surface: Rc<WlSurface>) {
        let data = match self.windows_by_surface_id.get(&surface.id) {
            Some(w) => w.clone(),
            _ => return,
        };
        self.create_window(&data, surface).await;
    }

    fn handle_xwayland_surface_destroyed(&mut self, surface: WlSurfaceId) {
        self.windows_by_surface_id.remove(&surface);
    }

    async fn handle_event(&mut self, event: &Event) {
        match event.ext() {
            Some(_) => {}
            _ => self.handle_core_event(event).await,
        }
    }

    async fn handle_core_event(&mut self, event: &Event) {
        let res = match event.code() {
            MapRequest::OPCODE => self.handle_map_request(event).await,
            MapNotify::OPCODE => self.handle_map_notify(event).await,
            ConfigureRequest::OPCODE => self.handle_configure_request(event).await,
            ConfigureNotify::OPCODE => self.handle_configure_notify(event),
            ClientMessage::OPCODE => self.handle_client_message(event).await,
            CreateNotify::OPCODE => self.handle_create_notify(event).await,
            DestroyNotify::OPCODE => self.handle_destroy_notify(event).await,
            PropertyNotify::OPCODE => self.handle_property_notify(event).await,
            FocusIn::OPCODE => self.handle_focus_in(event).await,
            UnmapNotify::OPCODE => self.handle_unmap_notify(event).await,
            _ => Ok(()),
        };
        if let Err(e) = res {
            log::warn!("Could not handle an event: {}", ErrorFmt(e));
        }
    }

    async fn handle_unmap_notify(&mut self, revent: &Event) -> Result<(), XWaylandError> {
        let event: UnmapNotify = revent.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(w) => w,
            _ => return Ok(()),
        };
        if data.map_link.replace(None).is_some() {
            self.num_mapped -= 1;
            self.set_net_client_list().await;
        }
        data.info.mapped.set(false);
        if let Some(win) = data.window.get() {
            win.map_status_changed();
        }
        self.set_wm_state(data, ICCCM_WM_STATE_WITHDRAWN).await;
        Ok(())
    }

    async fn handle_focus_in(&mut self, revent: &Event) -> Result<(), XWaylandError> {
        let event: FocusIn = revent.parse()?;
        log::info!("xwm focus_in {}", event.event);
        if matches!(event.mode, NOTIFY_MODE_GRAB | NOTIFY_MODE_UNGRAB) {
            log::info!("xwm GRAB/UNGRAB");
            return Ok(());
        }
        if matches!(event.detail, NOTIFY_DETAIL_POINTER) {
            log::info!("xwm POINTER");
            return Ok(());
        }
        let new_window = self.windows.get(&event.event);
        let mut focus_window = self.focus_window.as_ref();
        if let Some(window) = new_window {
            if let Some(prev) = focus_window {
                let prev_pid = prev.info.pid.get();
                let new_pid = window.info.pid.get();
                if prev_pid.is_some()
                    && prev_pid == new_pid
                    && revent.serial() >= self.last_input_serial
                {
                    log::info!("xwm ACCEPT");
                    focus_window = new_window;
                }
            }
        }
        let fw = focus_window.cloned();
        self.focus_window(fw.as_ref()).await;
        Ok(())
    }

    async fn close_window(&mut self, window: &Rc<XwindowData>) {
        if window.info.protocols.contains(&self.atoms.WM_DELETE_WINDOW) {
            self.send_wm_message(window, 0, &[self.atoms.WM_DELETE_WINDOW])
                .await;
        } else {
            self.c.call(&KillClient {
                resource: window.window_id,
            });
        }
    }

    async fn activate_window(&mut self, window: Option<&Rc<XwindowData>>) {
        log::info!("xwm activate_window {:?}", window.map(|w| w.window_id));
        if self.focus_window.as_ref().map(|w| w.window_id) == window.map(|w| w.window_id) {
            return;
        }
        if let Some(w) = window {
            if w.destroyed.get() || w.info.override_redirect.get() {
                return;
            }
            if w.info.minimized.get() {
                self.set_minimized(w, false).await;
            }
        }
        self.set_net_active_window(window).await;
        self.focus_window(window).await;
        if let Some(w) = window {
            self.stack_window(w, None, true).await;
        }
    }

    async fn set_net_active_window(&mut self, window: Option<&Rc<XwindowData>>) {
        let id = window.map(|w| w.window_id).unwrap_or(0);
        let cp = ChangeProperty {
            mode: PROP_MODE_REPLACE,
            window: self.root,
            property: self.atoms._NET_ACTIVE_WINDOW,
            ty: self.atoms.WINDOW,
            format: 32,
            data: uapi::as_bytes(&id),
        };
        if let Err(e) = self.c.call(&cp).await {
            log::error!("Could not set active window: {}", ErrorFmt(e));
        }
    }

    async fn handle_property_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: PropertyNotify = event.parse()?;
        // let name = self.c.call(&GetAtomName { atom: event.atom }).await;
        // if let Ok(name) = name {
        //     log::info!("{}", name.get().name);
        // }
        let data = match self.windows.get(&event.window) {
            Some(w) => w,
            _ => return Ok(()),
        };
        if event.atom == ATOM_WM_CLASS {
            // log::debug!("ATOM_WM_CLASS changed");
            self.load_window_wm_class(data).await;
        } else if event.atom == ATOM_WM_NAME {
            // log::debug!("ATOM_WM_NAME changed");
            self.load_window_wm_name(data).await;
        } else if event.atom == ATOM_WM_TRANSIENT_FOR {
            // log::debug!("ATOM_WM_TRANSIENT_FOR changed");
            self.load_window_wm_transient_for(data).await;
        } else if event.atom == self.atoms.WM_PROTOCOLS {
            // log::debug!("WM_PROTOCOLS changed");
            self.load_window_wm_protocols(data).await;
        } else if event.atom == self.atoms.WM_HINTS {
            // log::debug!("WM_HINTS changed");
            self.load_window_wm_hints(data).await;
        } else if event.atom == self.atoms.WM_NORMAL_HINTS {
            // log::debug!("WM_NORMAL_HINTS changed");
            self.load_window_wm_normal_hints(data).await;
        } else if event.atom == self.atoms._MOTIF_WM_HINTS {
            // log::debug!("_MOTIF_WM_HINTS changed");
            self.load_window_motif_wm_hints(data).await;
        } else if event.atom == self.atoms._NET_STARTUP_ID {
            // log::debug!("_NET_STARTUP_ID changed");
            self.load_window_net_startup_id(data).await;
        } else if event.atom == self.atoms._NET_WM_STATE {
            // log::debug!("_NET_WM_STATE changed");
            self.load_window_net_wm_state(data).await;
        } else if event.atom == self.atoms._NET_WM_WINDOW_TYPE {
            // log::debug!("_NET_WM_WINDOW_TYPE changed");
            self.load_window_net_wm_window_type(data).await;
        } else if event.atom == self.atoms._NET_WM_NAME {
            // log::debug!("_NET_WM_NAME changed");
            self.load_window_net_wm_name(data).await;
        } else if event.atom == self.atoms.WM_WINDOW_ROLE {
            // log::debug!("WM_WINDOW_ROLE changed");
            self.load_window_wm_window_role(data).await;
        }
        Ok(())
    }

    async fn handle_destroy_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: DestroyNotify = event.parse()?;
        let data = match self.windows.remove(&event.window) {
            Some(w) => w,
            _ => return Ok(()),
        };
        log::info!("xwm destroy_notify {}", event.window);
        data.destroyed.set(true);
        if let Some(sid) = data.surface_id.take() {
            self.windows_by_surface_id.remove(&sid);
        }
        if let Some(window) = data.window.take() {
            window.destroy();
        }
        if let Some(parent) = data.parent.take() {
            parent.children.remove(&data.window_id);
        }
        {
            let mut children = data.children.lock();
            for (_, child) in children.drain() {
                child.parent.set(None);
            }
        }
        if self.focus_window.as_ref().map(|w| w.window_id) == Some(event.window) {
            self.activate_window(None).await;
        }
        Ok(())
    }

    async fn handle_create_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: CreateNotify = event.parse()?;
        if event.window == self.xwin {
            return Ok(());
        }
        let data = Rc::new(XwindowData::new(&self.state, &event, &self.client));
        let cwa = ChangeWindowAttributes {
            window: event.window,
            values: CreateWindowValues {
                event_mask: Some(EVENT_MASK_PROPERTY_CHANGE | EVENT_MASK_FOCUS_CHANGE),
                ..Default::default()
            },
        };
        if let Err(e) = self.c.call(&cwa).await {
            log::error!(
                "Could not subscribe to events of new window: {}",
                ErrorFmt(e)
            );
        }
        if let Ok(res) = self
            .c
            .call(&GetGeometry {
                drawable: event.window,
            })
            .await
        {
            data.info.has_alpha.set(res.get().depth == 32);
        }
        self.windows.insert(event.window, data);
        Ok(())
    }

    async fn handle_client_message(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: ClientMessage = event.parse()?;
        if event.ty == self.atoms.WL_SURFACE_ID {
            self.handle_wl_surface_id(&event).await?;
        } else if event.ty == self.atoms._NET_WM_STATE {
            self.handle_net_wm_state(&event).await?;
        } else if event.ty == self.atoms._NET_ACTIVE_WINDOW {
            self.handle_net_active_window(&event).await?;
        } else if event.ty == self.atoms._NET_STARTUP_INFO
            || event.ty == self.atoms._NET_STARTUP_INFO_BEGIN
        {
            self.handle_net_startup_info(&event).await?;
        } else if event.ty == self.atoms.WM_CHANGE_STATE {
            self.handle_wm_change_state(&event).await?;
        } else if event.ty == self.atoms._NET_WM_MOVERESIZE {
            self.handle_net_wm_moveresize(&event).await?;
        }
        Ok(())
    }

    async fn set_net_client_list_stacking(&mut self) {
        let mut windows = Vec::with_capacity(self.num_stacked);
        for w in self.stack_list.iter() {
            windows.push(w.window_id);
        }
        let cp = ChangeProperty {
            mode: PROP_MODE_REPLACE,
            window: self.root,
            property: self.atoms._NET_CLIENT_LIST_STACKING,
            ty: ATOM_WINDOW,
            format: 32,
            data: uapi::as_bytes(&windows[..]),
        };
        if let Err(e) = self.c.call(&cp).await {
            log::error!("Could not set _NET_CLIENT_LIST_STACKING: {}", ErrorFmt(e));
        }
    }

    async fn set_net_client_list(&self) {
        let mut windows = Vec::with_capacity(self.num_mapped);
        for w in self.map_list.iter() {
            windows.push(w.window_id);
        }
        let cp = ChangeProperty {
            mode: PROP_MODE_REPLACE,
            window: self.root,
            property: self.atoms._NET_CLIENT_LIST,
            ty: ATOM_WINDOW,
            format: 32,
            data: uapi::as_bytes(&windows[..]),
        };
        if let Err(e) = self.c.call(&cp).await {
            log::error!("Could not set _NET_CLIENT_LIST: {}", ErrorFmt(e));
        }
    }

    fn update_override_redirect(&self, data: &Rc<XwindowData>, or: u8) {
        let or = or != 0;
        if data.info.override_redirect.replace(or) != or {
            // log::info!("xwin {} or {}", data.window_id, or);
            if let Some(window) = data.window.get() {
                window.node_destroy(true);
                window.map_status_changed();
            }
        }
    }

    async fn handle_map_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: MapNotify = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        self.update_override_redirect(data, event.override_redirect);
        data.info.mapped.set(true);
        if let Some(win) = data.window.get() {
            win.map_status_changed();
        }
        Ok(())
    }

    async fn handle_map_request(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: MapRequest = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(w) => w.clone(),
            _ => return Ok(()),
        };
        self.set_wm_state(&data, ICCCM_WM_STATE_NORMAL).await;
        self.set_net_wm_state(&data).await;
        if data
            .map_link
            .replace(Some(self.map_list.add_last(data.clone())))
            .is_none()
        {
            self.num_mapped += 1;
        }
        self.set_net_client_list().await;
        self.stack_window(&data, None, true).await;
        let mw = MapWindow {
            window: event.window,
        };
        if let Err(e) = self.c.call(&mw).await {
            log::error!("Could not map window: {}", ErrorFmt(e));
        }
        Ok(())
    }

    async fn stack_window(
        &mut self,
        window: &Rc<XwindowData>,
        sibling: Option<&Rc<XwindowData>>,
        above: bool,
    ) {
        let link = 'link: {
            if let Some(s) = sibling {
                if s.window_id == window.window_id {
                    log::warn!("trying to stack window above itself");
                } else {
                    let sl = s.stack_link.borrow_mut();
                    if let Some(sl) = sl.deref() {
                        break 'link if above {
                            sl.append(window.clone())
                        } else {
                            sl.prepend(window.clone())
                        };
                    }
                }
            }
            if above {
                self.stack_list.add_last(window.clone())
            } else {
                self.stack_list.add_first(window.clone())
            }
        };
        *window.stack_link.borrow_mut() = Some(link);
        let res = self
            .c
            .call(&ConfigureWindow {
                window: window.window_id,
                values: ConfigureWindowValues {
                    sibling: sibling.map(|s| s.window_id),
                    stack_mode: Some(match above {
                        true => STACK_MODE_ABOVE,
                        false => STACK_MODE_BELOW,
                    }),
                    ..Default::default()
                },
            })
            .await;
        if let Err(e) = res {
            log::warn!("Could not restack window: {}", ErrorFmt(e));
        }
        self.set_net_client_list_stacking().await;
    }

    async fn set_wm_state(&self, data: &Rc<XwindowData>, state: u32) {
        let property = [state, 0];
        let cp = ChangeProperty {
            mode: PROP_MODE_REPLACE,
            window: data.window_id,
            property: self.atoms.WM_STATE,
            ty: self.atoms.WM_STATE,
            format: 32,
            data: uapi::as_bytes(&property[..]),
        };
        self.c.call(&cp);
    }

    fn handle_configure_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: ConfigureNotify = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        self.update_override_redirect(data, event.override_redirect);
        if data.info.override_redirect.get() {
            let extents = Rect::new_sized(
                event.x as _,
                event.y as _,
                event.width as _,
                event.height as _,
            )
            .unwrap();
            if let Some(window) = data.window.get() {
                window.change_extents(&extents);
                self.state.tree_changed();
            } else {
                data.info.pending_extents.set(extents);
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
        if let Some(window) = data.window.get() {
            if window.is_mapped() {
                return Ok(());
            }
        }
        let de = data.info.pending_extents.get();
        let mut x1 = de.x1();
        let mut y1 = de.y1();
        let mut width = de.width();
        let mut height = de.height();
        if event.value_mask.contains(CONFIG_WINDOW_X) {
            x1 = event.x as _;
        }
        if event.value_mask.contains(CONFIG_WINDOW_Y) {
            y1 = event.y as _;
        }
        if event.value_mask.contains(CONFIG_WINDOW_WIDTH) {
            width = event.width as _;
        }
        if event.value_mask.contains(CONFIG_WINDOW_HEIGHT) {
            height = event.height as _;
        }
        data.info
            .pending_extents
            .set(Rect::new_sized(x1, y1, width, height).unwrap());
        Ok(())
    }

    async fn handle_net_wm_moveresize(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let _data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        let _detail = event.data[2];
        Ok(())
    }

    async fn handle_wm_change_state(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        let minimize = match event.data[0] {
            ICCCM_WM_STATE_NORMAL => false,
            ICCCM_WM_STATE_ICONIC => self.handle_minimize_requested(data).await,
            _ => return Ok(()),
        };
        data.info.minimized.set(minimize);
        self.set_net_wm_state(data).await;
        Ok(())
    }

    async fn handle_minimize_requested(&self, data: &Rc<XwindowData>) -> bool {
        if let Some(w) = data.window.get() {
            if w.toplevel_data.active_surfaces.get() > 0 {
                self.set_wm_state(data, ICCCM_WM_STATE_NORMAL).await;
                return false;
            }
        }
        self.set_wm_state(data, ICCCM_WM_STATE_ICONIC).await;
        true
    }

    async fn handle_net_startup_info(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        let mut startup_info = data.startup_info.borrow_mut();
        let mut msg = uapi::as_bytes(event.data);
        let mut end = false;
        if let Some(pos) = msg.find_byte(0) {
            end = true;
            msg = &msg[..pos];
        }
        startup_info.extend_from_slice(msg);
        if !end {
            return Ok(());
        }
        if let Some(id) = startup_info.strip_prefix(b"remove: ID=") {
            log::info!("Got startup id {}", id.as_bstr());
        } else {
            log::warn!("Unhandled startup info: {}", startup_info.as_bstr());
        }
        mem::take(startup_info.deref_mut());
        Ok(())
    }

    async fn handle_net_active_window(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        let fw = match &self.focus_window {
            Some(w) => w,
            _ => return Ok(()),
        };
        if data.info.pid.get().is_none() || data.info.pid.get() != fw.info.pid.get() {
            return Ok(());
        }
        let win = match data.window.get() {
            Some(w) => w,
            _ => return Ok(()),
        };
        let seats = self.state.globals.seats.lock();
        for (_, seat) in seats.deref() {
            seat.focus_toplevel(win.clone());
        }
        Ok(())
    }

    async fn handle_net_wm_state(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let data = match self.windows.get(&event.window) {
            Some(d) => d,
            _ => return Ok(()),
        };
        let mut changed = false;
        let mut fullscreen = data.info.fullscreen.get();
        let mut maximized_horz = data.info.maximized_horz.get();
        let mut maximized_vert = data.info.maximized_vert.get();
        let mut minimized = data.info.minimized.get();
        let mut modal = data.info.modal.get();
        let action = event.data[0];
        let mut update = |prop: &mut bool| {
            let new = match action {
                _NET_WM_STATE_REMOVE => false,
                _NET_WM_STATE_ADD => true,
                _NET_WM_STATE_TOGGLE => !*prop,
                _ => return,
            };
            if mem::replace(prop, new) != new {
                changed = true;
            }
        };
        for p in [event.data[1], event.data[2]] {
            if p == self.atoms._NET_WM_STATE_MODAL {
                update(&mut modal);
            } else if p == self.atoms._NET_WM_STATE_FULLSCREEN {
                update(&mut fullscreen);
            } else if p == self.atoms._NET_WM_STATE_MAXIMIZED_VERT {
                update(&mut maximized_vert);
            } else if p == self.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                update(&mut maximized_horz);
            } else if p == self.atoms._NET_WM_STATE_HIDDEN {
                update(&mut minimized);
            }
        }
        if !changed {
            return Ok(());
        }
        if minimized != data.info.minimized.get() {
            if minimized {
                minimized = self.handle_minimize_requested(data).await;
            }
        }
        data.info.fullscreen.set(fullscreen);
        data.info.maximized_horz.set(maximized_horz);
        data.info.maximized_vert.set(maximized_vert);
        data.info.minimized.set(minimized);
        data.info.modal.set(modal);
        self.update_wants_floating(data);
        self.set_net_wm_state(data).await;
        Ok(())
    }

    async fn handle_wl_surface_id(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
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
            self.create_window(&data, surface).await;
        }
        Ok(())
    }

    fn update_wants_floating(&self, data: &Rc<XwindowData>) {
        let res = data.info.modal.get()
            || data
                .info
                .window_types
                .contains(&self.atoms._NET_WM_WINDOW_TYPE_DIALOG)
            || data
                .info
                .window_types
                .contains(&self.atoms._NET_WM_WINDOW_TYPE_UTILITY)
            || data
                .info
                .window_types
                .contains(&self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR)
            || data
                .info
                .window_types
                .contains(&self.atoms._NET_WM_WINDOW_TYPE_SPLASH)
            || {
                let max_w = data.info.normal_hints.max_width.get();
                let min_w = data.info.normal_hints.min_width.get();
                let max_h = data.info.normal_hints.max_height.get();
                let min_h = data.info.normal_hints.min_height.get();
                max_w > 0 && max_h > 0 && max_w == min_w && max_h == min_h
            };
        data.info.wants_floating.set(res);
    }
}
