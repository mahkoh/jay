#![allow(clippy::await_holding_refcell_ref)] // all borrows are to data that is only used by this task

use {
    crate::{
        async_engine::SpawnedFuture,
        client::Client,
        criteria::tlm::TL_CHANGED_CLASS_INST,
        ifs::{
            ipc::{
                DataOfferId, DataSourceId, DynDataOffer, DynDataSource, IpcLocation, IpcVtable,
                SourceData, add_data_source_mime_type, destroy_data_device, destroy_data_offer,
                destroy_data_source, receive_data_offer,
                x_data_device::{XClipboardIpc, XIpc, XIpcDevice, XPrimarySelectionIpc},
                x_data_offer::XDataOffer,
                x_data_source::XDataSource,
            },
            wl_seat::{SeatId, WlSeatGlobal},
            wl_surface::{
                WlSurface,
                x_surface::xwindow::{XInputModel, Xwindow, XwindowData},
            },
        },
        io_uring::{IoUring, IoUringError},
        rect::Rect,
        state::State,
        tree::{Node, ToplevelNode},
        utils::{
            bitflags::BitflagsExt, buf::Buf, cell_ext::CellExt, clonecell::CloneCell,
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, hash_map_ext::HashMapExt,
            linkedlist::LinkedList, numcell::NumCell, oserror::OsError, rc_eq::rc_eq,
        },
        wire::WlSurfaceId,
        wire_xcon::{
            ChangeProperty, ChangeWindowAttributes, ClientMessage, CompositeRedirectSubwindows,
            ConfigureNotify, ConfigureRequest, ConfigureWindow, ConfigureWindowValues,
            ConvertSelection, CreateNotify, CreateWindow, CreateWindowValues, DestroyNotify,
            Extension, FocusIn, GetAtomName, GetGeometry, InternAtom, KillClient, MapNotify,
            MapRequest, MapWindow, PropertyNotify, ResClientIdSpec, ResQueryClientIds,
            SelectSelectionInput, SelectionNotify, SelectionRequest, SetInputFocus,
            SetSelectionOwner, UnmapNotify, XfixesQueryVersion, XfixesSelectionNotify,
        },
        xcon::{
            Event, XEvent, Xcon, XconError,
            consts::{
                _NET_WM_STATE_ADD, _NET_WM_STATE_REMOVE, _NET_WM_STATE_TOGGLE, ATOM_ATOM,
                ATOM_NONE, ATOM_STRING, ATOM_WINDOW, ATOM_WM_CLASS, ATOM_WM_NAME,
                ATOM_WM_SIZE_HINTS, ATOM_WM_TRANSIENT_FOR, COMPOSITE_REDIRECT_MANUAL,
                CONFIG_WINDOW_HEIGHT, CONFIG_WINDOW_WIDTH, CONFIG_WINDOW_X, CONFIG_WINDOW_Y,
                EVENT_MASK_FOCUS_CHANGE, EVENT_MASK_PROPERTY_CHANGE,
                EVENT_MASK_SUBSTRUCTURE_NOTIFY, EVENT_MASK_SUBSTRUCTURE_REDIRECT,
                ICCCM_WM_HINT_INPUT, ICCCM_WM_STATE_ICONIC, ICCCM_WM_STATE_NORMAL,
                ICCCM_WM_STATE_WITHDRAWN, INPUT_FOCUS_POINTER_ROOT, MWM_HINTS_DECORATIONS_FIELD,
                MWM_HINTS_FLAGS_FIELD, NOTIFY_DETAIL_POINTER, NOTIFY_MODE_GRAB, NOTIFY_MODE_UNGRAB,
                PROP_MODE_APPEND, PROP_MODE_REPLACE, RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID,
                SELECTION_CLIENT_CLOSE_MASK, SELECTION_WINDOW_DESTROY_MASK,
                SET_SELECTION_OWNER_MASK, STACK_MODE_ABOVE, STACK_MODE_BELOW,
                WINDOW_CLASS_INPUT_OUTPUT,
            },
        },
        xwayland::{XWaylandError, XWaylandEvent},
    },
    ahash::{AHashMap, AHashSet},
    bstr::ByteSlice,
    futures_util::{FutureExt, select},
    smallvec::SmallVec,
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        marker::PhantomData,
        mem::{self},
        ops::{Deref, DerefMut},
        rc::Rc,
        time::Duration,
    },
    uapi::{OwnedFd, c},
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
    WL_SURFACE_SERIAL,
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

struct EnhancedOffer {
    offer: Rc<XDataOffer>,
    mime_types: RefCell<Vec<u32>>,
    active: Cell<bool>,
}

#[derive(Default)]
struct SelectionData<T: XIpc> {
    sources: CopyHashMap<SeatId, Rc<XDataSource>>,
    offers: CopyHashMap<SeatId, Rc<EnhancedOffer>>,
    active_offer: CloneCell<Option<Rc<EnhancedOffer>>>,
    win: Cell<u32>,
    selection: Cell<u32>,
    pending_transfers: RefCell<Vec<PendingTransfer>>,
    _phantom: PhantomData<T>,
}

impl<T: XIpc> SelectionData<T> {
    fn destroy(&self) {
        for offer in self.offers.lock().drain_values() {
            destroy_data_offer::<T>(&offer.offer);
        }
        self.active_offer.take();
        self.destroy_sources();
    }

    fn destroy_sources(&self) {
        for source in self.sources.lock().drain_values() {
            destroy_data_source::<T>(&source);
        }
    }

    fn seat_removed(&self, id: SeatId) {
        if let Some(offer) = self.active_offer.get() {
            if offer.offer.get_seat().id() == id {
                self.active_offer.take();
            }
        }
        self.offers.remove(&id);
        self.sources.remove(&id);
    }
}

#[derive(Default)]
pub struct XwmShared {
    devices: CopyHashMap<SeatId, Rc<XIpcDevice>>,
    data: SelectionData<XClipboardIpc>,
    primary_selection: SelectionData<XPrimarySelectionIpc>,
    transfers: CopyHashMap<u64, SpawnedFuture<()>>,
}

impl Drop for XwmShared {
    fn drop(&mut self) {
        self.data.destroy();
        self.primary_selection.destroy();
        for device in self.devices.lock().drain_values() {
            destroy_data_device::<XClipboardIpc>(&device);
            destroy_data_device::<XPrimarySelectionIpc>(&device);
            device.seat.unset_x_data_device(device.id);
        }
        self.transfers.clear();
    }
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
    windows_by_surface_serial: AHashMap<u64, Rc<XwindowData>>,
    last_surface_serial: u64,
    focus_window: Option<Rc<XwindowData>>,
    last_input_serial: u64,
    atom_cache: AHashMap<String, u32>,
    atom_name_cache: AHashMap<u32, String>,

    transfer_ids: NumCell<u64>,
    known_seats: AHashMap<SeatId, Rc<WlSeatGlobal>>,
    shared: Rc<XwmShared>,

    stack_list: LinkedList<Rc<XwindowData>>,
    num_stacked: usize,

    map_list: LinkedList<Rc<XwindowData>>,
    num_mapped: usize,
}

struct PendingTransfer {
    mime_type: u32,
    fd: Rc<OwnedFd>,
}

const TEXT_PLAIN_UTF_8: &str = "text/plain;charset=utf-8";
const TEXT_PLAIN: &str = "text/plain";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Initiator {
    X,
    Wayland,
}

impl Drop for Wm {
    fn drop(&mut self) {
        for window in self.windows.drain_values() {
            if let Some(window) = window.window.take() {
                window.break_loops();
            }
            window.children.clear();
            window.parent.take();
            window.stack_link.take();
            window.map_link.take();
        }
        self.windows_by_surface_id.clear();
        self.windows_by_surface_serial.clear();
        self.focus_window.take();
        self.known_seats.clear();
    }
}

impl Wm {
    pub(super) async fn get(
        state: &Rc<State>,
        client: Rc<Client>,
        socket: OwnedFd,
        shared: &Rc<XwmShared>,
    ) -> Result<Self, XWaylandError> {
        let c = match Xcon::connect_to_fd(state, &Rc::new(socket), &[], &[]).await {
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
        let root = c.root_window();
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
            let first = match first.iter().find(|i| i.0.0 == 1) {
                Some(f) => f.1,
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
        {
            let qv = XfixesQueryVersion {
                client_major_version: 1,
                client_minor_version: 0,
            };
            if let Err(e) = c.call(&qv).await {
                return Err(XWaylandError::XfixesQueryVersion(e));
            }
        }
        let mut clipboard_wins = [0, 0];
        for (idx, atom) in [atoms.CLIPBOARD, atoms.PRIMARY].into_iter().enumerate() {
            let win = c.generate_id()?;
            let cw = CreateWindow {
                depth: 0,
                wid: win,
                parent: root,
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                border_width: 0,
                class: WINDOW_CLASS_INPUT_OUTPUT,
                visual: 0,
                values: CreateWindowValues {
                    event_mask: None,
                    ..Default::default()
                },
            };
            if let Err(e) = c.call(&cw).await {
                return Err(XWaylandError::CreateSelectionWindow(e));
            }
            let ssi = SelectSelectionInput {
                window: win,
                selection: atom,
                event_mask: SET_SELECTION_OWNER_MASK
                    | SELECTION_CLIENT_CLOSE_MASK
                    | SELECTION_WINDOW_DESTROY_MASK,
            };
            if let Err(e) = c.call(&ssi).await {
                return Err(XWaylandError::WatchSelection(e));
            }
            clipboard_wins[idx] = win;
        }
        shared.data.win.set(clipboard_wins[0]);
        shared.data.selection.set(atoms.CLIPBOARD);
        shared.primary_selection.win.set(clipboard_wins[1]);
        shared.primary_selection.selection.set(atoms.PRIMARY);
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
            windows_by_surface_serial: Default::default(),
            last_surface_serial: 0,
            focus_window: Default::default(),
            last_input_serial: 0,
            atom_cache: Default::default(),
            atom_name_cache: Default::default(),
            transfer_ids: Default::default(),
            known_seats: Default::default(),
            shared: shared.clone(),
            stack_list: Default::default(),
            num_stacked: 0,
            map_list: Default::default(),
            num_mapped: 0,
        })
    }

    fn seats_changed(&mut self) {
        let current_seats: AHashMap<_, _> = self
            .state
            .globals
            .seats
            .lock()
            .values()
            .map(|s| (s.id(), s.clone()))
            .collect();
        let mut new_seats = vec![];
        let mut removed_seats = vec![];
        for (id, seat) in &current_seats {
            if !self.known_seats.contains_key(id) {
                new_seats.push(seat.clone());
            }
        }
        for id in self.known_seats.keys() {
            if !current_seats.contains_key(id) {
                removed_seats.push(*id);
            }
        }
        for seat in removed_seats {
            self.shared.data.seat_removed(seat);
            self.shared.primary_selection.seat_removed(seat);
            self.shared.devices.remove(&seat);
        }
        for seat in new_seats {
            let dd = Rc::new(XIpcDevice {
                id: self.state.xwayland.ipc_device_ids.next(),
                clipboard: Default::default(),
                primary_selection: Default::default(),
                seat: seat.clone(),
                state: self.state.clone(),
                client: self.client.clone(),
            });
            seat.set_x_data_device(&dd);
            self.shared.devices.set(seat.id(), dd.clone());
        }
        self.known_seats = current_seats;
    }

    pub async fn run(mut self) {
        self.seats_changed();
        loop {
            select! {
                e = self.state.xwayland.queue.pop().fuse() => self.handle_xwayland_event(e).await,
                e = self.c.event().fuse() => self.handle_event(&e).await,
            }
        }
    }

    async fn handle_xwayland_event(&mut self, e: XWaylandEvent) {
        match e {
            XWaylandEvent::SurfaceCreated(event) => {
                self.handle_xwayland_surface_created(event).await
            }
            XWaylandEvent::SurfaceSerialAssigned(event) => {
                self.handle_xwayland_surface_serial_assigned(event).await
            }
            XWaylandEvent::Configure(event) => self.handle_xwayland_configure(event).await,
            XWaylandEvent::SurfaceDestroyed(surface_id, serial) => {
                self.handle_xwayland_surface_destroyed(surface_id, serial)
            }
            XWaylandEvent::Activate(window) => {
                self.activate_window(Some(&window), Initiator::Wayland)
                    .await
            }
            XWaylandEvent::ActivateRoot => self.activate_window(None, Initiator::Wayland).await,
            XWaylandEvent::Close(window) => self.close_window(&window).await,
            XWaylandEvent::SeatChanged => self.seats_changed(),
            XWaylandEvent::IpcCancelSource {
                location,
                seat,
                source,
            } => match location {
                IpcLocation::Clipboard => {
                    self.dd_cancel_source::<XClipboardIpc>(&self.shared.clone().data, seat, source)
                }
                IpcLocation::PrimarySelection => self.dd_cancel_source::<XPrimarySelectionIpc>(
                    &self.shared.clone().primary_selection,
                    seat,
                    source,
                ),
            },
            XWaylandEvent::IpcSendSource {
                location,
                seat,
                source,
                mime_type,
                fd,
            } => match location {
                IpcLocation::Clipboard => {
                    self.dd_send_source::<XClipboardIpc>(
                        &self.shared.clone().data,
                        seat,
                        source,
                        mime_type,
                        fd,
                    )
                    .await
                }
                IpcLocation::PrimarySelection => {
                    self.dd_send_source::<XPrimarySelectionIpc>(
                        &self.shared.clone().primary_selection,
                        seat,
                        source,
                        mime_type,
                        fd,
                    )
                    .await
                }
            },
            XWaylandEvent::IpcSetOffer {
                location,
                seat,
                offer,
            } => match location {
                IpcLocation::Clipboard => {
                    self.dd_set_offer::<XClipboardIpc>(&self.shared.clone().data, seat, offer)
                        .await
                }
                IpcLocation::PrimarySelection => {
                    self.dd_set_offer::<XPrimarySelectionIpc>(
                        &self.shared.clone().primary_selection,
                        seat,
                        offer,
                    )
                    .await
                }
            },
            XWaylandEvent::IpcSetSelection {
                seat,
                location,
                offer,
            } => match location {
                IpcLocation::Clipboard => {
                    self.dd_set_selection::<XClipboardIpc>(&self.shared.clone().data, seat, offer)
                        .await
                }
                IpcLocation::PrimarySelection => {
                    self.dd_set_selection::<XPrimarySelectionIpc>(
                        &self.shared.clone().primary_selection,
                        seat,
                        offer,
                    )
                    .await
                }
            },
            XWaylandEvent::IpcAddOfferMimeType {
                location,
                seat,
                offer,
                mime_type,
            } => match location {
                IpcLocation::Clipboard => {
                    self.dd_add_offer_mime_type::<XClipboardIpc>(
                        &self.shared.clone().data,
                        seat,
                        offer,
                        mime_type,
                    )
                    .await
                }
                IpcLocation::PrimarySelection => {
                    self.dd_add_offer_mime_type::<XPrimarySelectionIpc>(
                        &self.shared.clone().primary_selection,
                        seat,
                        offer,
                        mime_type,
                    )
                    .await
                }
            },
        }
    }

    async fn dd_add_offer_mime_type<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        seat: SeatId,
        offer: DataOfferId,
        mt: String,
    ) {
        let enhanced = match sd.offers.get(&seat) {
            Some(r) if r.offer.offer_id != offer => {
                return;
            }
            None => {
                return;
            }
            Some(r) => r,
        };
        let mt = match self.mime_type_to_atom(mt).await {
            Ok(mt) => mt,
            Err(e) => {
                log::error!("Could not get mime type atom: {}", ErrorFmt(e));
                return;
            }
        };
        enhanced.mime_types.borrow_mut().push(mt);
    }

    async fn dd_set_offer<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        seat: SeatId,
        offer: Rc<XDataOffer>,
    ) {
        let mut mime_types = vec![];
        if let Some(offer) = sd.offers.remove(&seat) {
            destroy_data_offer::<T>(&offer.offer);
            mime_types = mem::take(offer.mime_types.borrow_mut().deref_mut());
        }
        sd.offers.set(
            seat,
            Rc::new(EnhancedOffer {
                offer,
                mime_types: RefCell::new(mime_types),
                active: Cell::new(false),
            }),
        );
    }

    async fn dd_set_selection<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        seat: SeatId,
        offer: Option<Rc<XDataOffer>>,
    ) {
        let offer = match offer {
            None => {
                if let Some(offer) = sd.offers.remove(&seat) {
                    destroy_data_offer::<T>(&offer.offer);
                    if offer.active.get() {
                        sd.active_offer.take();
                    }
                }
                return;
            }
            Some(offer) => offer,
        };
        let enhanced = match sd.offers.get(&seat) {
            None => {
                destroy_data_offer::<T>(&offer);
                return;
            }
            Some(e) => e,
        };
        if !rc_eq(&enhanced.offer, &offer) {
            destroy_data_offer::<T>(&offer);
            return;
        }
        if !enhanced.active.replace(true) {
            if let Some(old) = sd.active_offer.set(Some(enhanced)) {
                old.active.set(false);
            }
        }
        let so = SetSelectionOwner {
            owner: sd.win.get(),
            selection: sd.selection.get(),
            time: 0,
        };
        if let Err(err) = self.c.call(&so).await {
            log::error!("Could not set primary selection owner: {}", ErrorFmt(err));
        }
    }

    async fn get_atom_name(&mut self, atom: u32) -> Result<String, XconError> {
        if let Some(name) = self.atom_name_cache.get(&atom) {
            return Ok(name.clone());
        }
        let gan = GetAtomName { atom };
        match self.c.call(&gan).await {
            Ok(name) => {
                let name = name.get().name.to_string();
                self.atom_name_cache.insert(atom, name.clone());
                Ok(name)
            }
            Err(e) => Err(e),
        }
    }

    async fn get_atom(&mut self, name: String) -> Result<u32, XconError> {
        if let Some(atom) = self.atom_cache.get(&name) {
            return Ok(*atom);
        }
        let ia = InternAtom {
            only_if_exists: 0,
            name: name.as_bytes().as_bstr(),
        };
        match self.c.call(&ia).await {
            Ok(id) => {
                let atom = id.get().atom;
                self.atom_cache.insert(name, atom);
                Ok(atom)
            }
            Err(e) => Err(e),
        }
    }

    async fn mime_type_to_atom(&mut self, mime_type: String) -> Result<u32, XconError> {
        match mime_type.as_str() {
            TEXT_PLAIN_UTF_8 => Ok(self.atoms.UTF8_STRING),
            TEXT_PLAIN => Ok(ATOM_STRING),
            _ => self.get_atom(mime_type).await,
        }
    }

    async fn atom_to_mime_type(&mut self, atom: u32) -> Result<String, XconError> {
        if atom == self.atoms.UTF8_STRING {
            Ok(TEXT_PLAIN_UTF_8.to_string())
        } else if atom == ATOM_STRING {
            Ok(TEXT_PLAIN.to_string())
        } else {
            self.get_atom_name(atom).await
        }
    }

    async fn dd_send_source<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        seat: SeatId,
        src: DataSourceId,
        mime_type: String,
        fd: Rc<OwnedFd>,
    ) {
        let actual_src = match sd.sources.get(&seat) {
            None => return,
            Some(src) => src,
        };
        if actual_src.source_data().id != src {
            return;
        }
        let mime_type = match self.mime_type_to_atom(mime_type).await {
            Ok(mt) => mt,
            Err(e) => {
                log::error!("Could not intern mime type: {}", ErrorFmt(e));
                return;
            }
        };
        let cs = ConvertSelection {
            requestor: sd.win.get(),
            selection: sd.selection.get(),
            target: mime_type,
            property: self.atoms._WL_SELECTION,
            time: 0,
        };
        if let Err(e) = self.c.call(&cs).await {
            log::error!(
                "Could not perform convert selection request: {}",
                ErrorFmt(e)
            );
            return;
        }
        sd.pending_transfers
            .borrow_mut()
            .push(PendingTransfer { mime_type, fd });
    }

    fn dd_cancel_source<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        seat: SeatId,
        source: DataSourceId,
    ) {
        if let Some(cur) = sd.sources.get(&seat) {
            if cur.source_data().id == source {
                sd.sources.remove(&seat);
                destroy_data_source::<T>(&cur);
            }
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
        let mut x = extents.x1();
        let mut y = extents.y1();
        let mut width = extents.width();
        let mut height = extents.height();
        logical_to_client_wire_scale!(self.client, x, y, width, height);
        let cw = ConfigureWindow {
            window: window.data.window_id,
            values: ConfigureWindowValues {
                x: Some(x),
                y: Some(y),
                width: Some(width as u32),
                height: Some(height as u32),
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

    #[expect(dead_code)]
    async fn set_maximized(&self, data: &Rc<XwindowData>, maximized: bool) {
        data.info.maximized_vert.set(maximized);
        data.info.maximized_horz.set(maximized);
        self.set_net_wm_state(data).await;
    }

    #[expect(dead_code)]
    async fn set_fullscreen(&self, data: &Rc<XwindowData>, fullscreen: bool) {
        if false {
            // NOTE: We do not want to inform the program if the user changes the fullscreen
            // status of the window. Programs usually provide an in-program way to enter/exit
            // fullscreen mode.
            data.info.fullscreen.set(fullscreen);
            self.set_net_wm_state(data).await;
        }
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

    async fn focus_window(
        &mut self,
        window: Option<&Rc<XwindowData>>,
        initiator: Initiator,
        send_to_x: bool,
    ) {
        // log::info!("xwm focus_window {:?}", window.map(|w| w.window_id));
        if let Some(old) = mem::replace(&mut self.focus_window, window.cloned()) {
            // log::info!("xwm unfocus {:?}", old.window_id);
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
            // log::info!("xwm or => return");
            return;
        }
        if initiator == Initiator::X {
            if let Some(window) = window.window.get() {
                let seats = self.state.globals.seats.lock();
                for seat in seats.values() {
                    seat.focus_toplevel(window.clone());
                }
            }
        }
        if send_to_x {
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
        // log::info!("{} role {}", data.window_id, buf.as_bstr());
        *data.info.role.borrow_mut() = Some(buf.into());
    }

    async fn load_window_wm_class(&self, data: &Rc<XwindowData>) {
        let mut buf = vec![];
        let property_changed = || {
            if let Some(window) = data.window.get() {
                window.toplevel_data.property_changed(TL_CHANGED_CLASS_INST);
            }
        };
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
                property_changed();
                return;
            }
            Err(e) => {
                log::error!("Could not retrieve WM_CLASS property: {}", ErrorFmt(e));
                return;
            }
        }
        let mut iter = buf.split(|c| *c == 0);
        let mut map = || Some(iter.next().unwrap_or(&[]).to_str_lossy().into_owned());
        *data.info.instance.borrow_mut() = map();
        *data.info.class.borrow_mut() = map();
        property_changed();
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
        let title = buf.as_bstr().to_string();
        if let Some(window) = data.window.get() {
            window.toplevel_data.set_title(&title);
            window.tl_title_changed();
        }
        *data.info.title.borrow_mut() = Some(title);
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
        if data.window.is_some() {
            log::error!("The xwindow has already been constructed");
            return;
        }
        let window = match Xwindow::install(data, &surface) {
            Ok(w) => w,
            Err(e) => {
                log::error!(
                    "Could not attach the xwindow to the surface: {}",
                    ErrorFmt(e)
                );
                return;
            }
        };
        self.state.xwayland.windows.set(window.id, window.clone());
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

    async fn handle_xwayland_surface_created(&mut self, surface: WlSurfaceId) {
        let surface = match self.client.lookup(surface) {
            Ok(s) => s,
            _ => return,
        };
        let data = match self.windows_by_surface_id.get(&surface.id) {
            Some(w) => w.clone(),
            _ => return,
        };
        self.create_window(&data, surface).await;
    }

    async fn handle_xwayland_surface_serial_assigned(&mut self, surface: WlSurfaceId) {
        let surface = match self.client.lookup(surface) {
            Ok(s) => s,
            _ => return,
        };
        let serial = match surface.xwayland_serial() {
            Some(s) => s,
            _ => return,
        };
        let data = match self.windows_by_surface_serial.get(&serial) {
            Some(w) => w.clone(),
            _ => return,
        };
        self.create_window(&data, surface).await;
    }

    fn handle_xwayland_surface_destroyed(&mut self, surface: WlSurfaceId, serial: Option<u64>) {
        self.windows_by_surface_id.remove(&surface);
        if let Some(serial) = serial {
            self.windows_by_surface_serial.remove(&serial);
        }
    }

    async fn handle_event(&mut self, event: &Event) {
        let res = match event.ext() {
            Some(ex) => self.handle_extension_event(ex, event).await,
            _ => self.handle_core_event(event).await,
        };
        if let Err(e) = res {
            log::warn!("Could not handle an event: {}", ErrorFmt(e));
        }
    }

    async fn handle_extension_event(
        &mut self,
        ex: Extension,
        event: &Event,
    ) -> Result<(), XWaylandError> {
        match ex {
            Extension::XFIXES => self.handle_xfixes_event(event).await,
            _ => Ok(()),
        }
    }

    async fn handle_xfixes_event(&mut self, event: &Event) -> Result<(), XWaylandError> {
        match event.code() {
            XfixesSelectionNotify::OPCODE => self.handle_xfixes_selection_notify(event).await,
            _ => Ok(()),
        }
    }

    async fn handle_xfixes_selection_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: XfixesSelectionNotify = event.parse()?;
        let shared = self.shared.clone();
        if event.selection == self.atoms.PRIMARY {
            self.handle_xfixes_selection_notify_(&shared.primary_selection, &event)
                .await
        } else if event.selection == self.atoms.CLIPBOARD {
            self.handle_xfixes_selection_notify_(&shared.data, &event)
                .await
        } else {
            Ok(())
        }
    }

    async fn handle_xfixes_selection_notify_<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        event: &XfixesSelectionNotify,
    ) -> Result<(), XWaylandError> {
        if event.owner == sd.win.get() {
            return Ok(());
        }
        sd.destroy_sources();
        let cs = ConvertSelection {
            requestor: sd.win.get(),
            selection: sd.selection.get(),
            target: self.atoms.TARGETS,
            property: self.atoms._WL_SELECTION,
            time: event.timestamp,
        };
        if let Err(e) = self.c.call(&cs).await {
            log::error!("Could not convert selection: {}", ErrorFmt(e));
        }
        Ok(())
    }

    async fn handle_core_event(&mut self, event: &Event) -> Result<(), XWaylandError> {
        match event.code() {
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
            SelectionNotify::OPCODE => self.handle_selection_notify(event).await,
            SelectionRequest::OPCODE => self.handle_selection_request(event).await,
            _ => Ok(()),
        }
    }

    async fn handle_selection_request(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: SelectionRequest = event.parse()?;
        let shared = self.shared.clone();
        if event.selection == self.atoms.PRIMARY {
            self.handle_selection_request_(&shared.primary_selection, &event)
                .await
        } else if event.selection == self.atoms.CLIPBOARD {
            self.handle_selection_request_(&shared.data, &event).await
        } else {
            log::warn!("Unknown selection request");
            Ok(())
        }
    }

    async fn handle_selection_request_<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        event: &SelectionRequest,
    ) -> Result<(), XWaylandError> {
        let mut success = Some(false);
        if let Some(offer) = sd.active_offer.get() {
            let mt = offer.mime_types.borrow_mut();
            if event.target == self.atoms.TARGETS {
                let cp = ChangeProperty {
                    mode: PROP_MODE_REPLACE,
                    window: event.requestor,
                    property: event.property,
                    ty: ATOM_ATOM,
                    format: 32,
                    data: uapi::as_bytes(&mt[..]),
                };
                match self.c.call(&cp).await {
                    Ok(_) => success = Some(true),
                    Err(e) => {
                        log::error!("Could not set selection property: {}", ErrorFmt(e));
                    }
                }
            } else {
                'convert: {
                    let present = mt.contains(&event.target);
                    drop(mt);
                    let mt = match self.atom_to_mime_type(event.target).await {
                        Ok(mt) => mt,
                        Err(e) => {
                            log::error!("Could not get mime type name: {}", ErrorFmt(e));
                            break 'convert;
                        }
                    };
                    if !present {
                        log::error!("Peer requested unavailable target {}", mt);
                        break 'convert;
                    }
                    let (rx, tx) = match uapi::pipe2(c::O_CLOEXEC) {
                        Ok(p) => p,
                        Err(e) => {
                            log::error!("Could not create pipe: {}", OsError::from(e));
                            break 'convert;
                        }
                    };
                    success = None;
                    receive_data_offer::<T>(&offer.offer, &mt, Rc::new(tx));
                    let id = self.transfer_ids.fetch_add(1);
                    let wtx = WaylandToXTransfer {
                        id,
                        fd: Rc::new(rx),
                        ring: self.state.ring.clone(),
                        c: self.c.clone(),
                        window: event.requestor,
                        time: event.time,
                        property: event.property,
                        ty: event.target,
                        selection: sd.selection.get(),
                        shared: self.shared.clone(),
                    };
                    self.shared
                        .transfers
                        .set(id, self.state.eng.spawn("wayland to X transfer", wtx.run()));
                }
            }
        }
        if let Some(success) = success {
            let target = match success {
                true => event.target,
                false => ATOM_NONE,
            };
            let sn = SelectionNotify {
                time: event.time,
                requestor: event.requestor,
                selection: sd.selection.get(),
                target,
                property: event.property,
            };
            if let Err(e) = self.c.send_event(false, event.requestor, 0, &sn).await {
                log::error!("Could not send event: {}", ErrorFmt(e));
            }
        }
        Ok(())
    }

    async fn handle_selection_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: SelectionNotify = event.parse()?;
        if event.property != self.atoms._WL_SELECTION {
            return Ok(());
        }
        let shared = self.shared.clone();
        if event.selection == self.atoms.PRIMARY {
            self.handle_selection_notify_(&shared.primary_selection, &event)
                .await
        } else if event.selection == self.atoms.CLIPBOARD {
            self.handle_selection_notify_(&shared.data, &event).await
        } else {
            Ok(())
        }
    }

    async fn handle_selection_notify_<T: XIpc>(
        &mut self,
        sd: &SelectionData<T>,
        event: &SelectionNotify,
    ) -> Result<(), XWaylandError> {
        if event.property != self.atoms._WL_SELECTION {
            return Ok(());
        }
        if event.target == ATOM_NONE {
            return Ok(());
        }
        if event.target == self.atoms.TARGETS {
            let targets = self.get_selection_mime_types(sd.win.get()).await?;
            for dev in self.shared.devices.lock().values() {
                let seat = T::get_device_seat(dev);
                if !seat.may_modify_primary_selection(&self.client, None) {
                    continue;
                }
                let source = Rc::new(XDataSource {
                    state: self.state.clone(),
                    device: dev.clone(),
                    data: SourceData::new(&self.client),
                    location: T::LOCATION,
                });
                for target in &targets {
                    add_data_source_mime_type::<T>(&source, target);
                }
                let res = match source.location {
                    IpcLocation::Clipboard => seat.set_selection(Some(source.clone())),
                    IpcLocation::PrimarySelection => {
                        seat.set_primary_selection(Some(source.clone()))
                    }
                };
                if let Err(e) = res {
                    log::error!("Could not set selection: {}", ErrorFmt(e));
                    return Ok(());
                }
                sd.sources.set(seat.id(), source);
            }
        } else {
            let mut transfers = sd.pending_transfers.borrow_mut();
            let transfers = transfers.drain(..);
            let mut data = vec![];
            let gp = self
                .c
                .get_property(
                    sd.win.get(),
                    self.atoms._WL_SELECTION,
                    event.target,
                    &mut data,
                )
                .await;
            if let Err(e) = gp {
                log::error!("Could not get converted property: {}", e);
                return Ok(());
            }
            let mut data = Buf::from_slice(&data);
            for transfer in transfers {
                if event.target != transfer.mime_type {
                    log::error!("Conversion yielded an incompatible mime type");
                    continue;
                }
                let id = self.transfer_ids.fetch_add(1);
                let transfer = XToWaylandTransfer {
                    id,
                    data: data.clone(),
                    fd: transfer.fd,
                    state: self.state.clone(),
                    shared: self.shared.clone(),
                };
                self.shared.transfers.set(
                    id,
                    self.state
                        .eng
                        .spawn("X to wayland transfer", transfer.run()),
                );
            }
        }

        Ok(())
    }

    async fn get_selection_mime_types(
        &mut self,
        window: u32,
    ) -> Result<Vec<String>, XWaylandError> {
        let mut buf = vec![];
        self.c
            .get_property3::<u32>(window, self.atoms._WL_SELECTION, ATOM_ATOM, true, &mut buf)
            .await?;
        let mut res = vec![];
        for atom in buf {
            let name = match self.atom_to_mime_type(atom).await {
                Ok(n) => n,
                Err(e) => {
                    log::error!("Could not get atom name: {}", ErrorFmt(e));
                    continue;
                }
            };
            res.push(name);
        }
        Ok(res)
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
        // log::info!("xwm focus_in {}", event.event);
        if matches!(event.mode, NOTIFY_MODE_GRAB | NOTIFY_MODE_UNGRAB) {
            // log::info!("xwm GRAB/UNGRAB");
            return Ok(());
        }
        if matches!(event.detail, NOTIFY_DETAIL_POINTER) {
            // log::info!("xwm POINTER");
            return Ok(());
        }
        let new_window = self.windows.get(&event.event);
        let mut focus_window = self.focus_window.as_ref();
        let mut send_to_x = true;
        if let Some(window) = new_window {
            if let Some(w) = window.window.get() {
                if let Some(prev) = focus_window {
                    let prev_pid = prev.info.pid.get();
                    let new_pid = window.info.pid.get();
                    if prev_pid.is_some()
                        && prev_pid == new_pid
                        && revent.serial() >= self.last_input_serial
                        && w.x.surface.node_visible()
                    {
                        // log::info!("xwm ACCEPT");
                        focus_window = new_window;
                        send_to_x = false;
                    }
                }
            }
        }
        let fw = focus_window.cloned();
        self.focus_window(fw.as_ref(), Initiator::X, send_to_x)
            .await;
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

    async fn activate_window(&mut self, window: Option<&Rc<XwindowData>>, initiator: Initiator) {
        // log::info!("xwm activate_window {:?}", window.map(|w| w.window_id));
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
        self.focus_window(window, initiator, true).await;
        if let Some(w) = window {
            self.move_to_top_of_stack(w);
            self.configure_stack_position(w).await;
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
        // log::info!("xwm destroy_notify {}", event.window);
        data.destroyed.set(true);
        data.stack_link.borrow_mut().take();
        data.map_link.take();
        if let Some(sid) = data.surface_id.take() {
            self.windows_by_surface_id.remove(&sid);
        }
        if let Some(serial) = data.surface_serial.take() {
            self.windows_by_surface_serial.remove(&serial);
        }
        if let Some(window) = data.window.take() {
            window.destroy();
        }
        if let Some(parent) = data.parent.take() {
            parent.children.remove(&data.window_id);
        }
        {
            let mut children = data.children.lock();
            for child in children.drain_values() {
                child.parent.set(None);
            }
        }
        if self.focus_window.as_ref().map(|w| w.window_id) == Some(event.window) {
            self.activate_window(None, Initiator::X).await;
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
        } else if event.ty == self.atoms.WL_SURFACE_SERIAL {
            self.handle_wl_surface_serial(&event).await?;
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
                window.tl_destroy();
                window.update_toplevel();
                window.map_status_changed();
            }
        }
    }

    async fn handle_map_notify(&mut self, event: &Event) -> Result<(), XWaylandError> {
        let event: MapNotify = event.parse()?;
        let data = match self.windows.get(&event.window) {
            Some(d) => d.clone(),
            _ => return Ok(()),
        };
        self.update_override_redirect(&data, event.override_redirect);
        data.info.mapped.set(true);
        if let Some(win) = data.window.get() {
            win.map_status_changed();
        }
        self.configure_stack_position(&data).await;
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
        let pending = data.info.pending_extents.get();
        if pending.width() > 0 && pending.height() > 0 {
            let dummy = Rect::new_sized(0, 0, 1, 1).unwrap();
            for rect in [dummy, pending] {
                let mut x = rect.x1();
                let mut y = rect.y1();
                let mut width = rect.width();
                let mut height = rect.height();
                logical_to_client_wire_scale!(self.client, x, y, width, height);
                let cw = ConfigureWindow {
                    window: data.window_id,
                    values: ConfigureWindowValues {
                        x: Some(x),
                        y: Some(y),
                        width: Some(width as _),
                        height: Some(height as _),
                        ..Default::default()
                    },
                };
                let _ = self.c.call(&cw).await;
            }
        }
        self.move_to_top_of_stack(&data);
        let mw = MapWindow {
            window: event.window,
        };
        if let Err(e) = self.c.call(&mw).await {
            log::error!("Could not map window: {}", ErrorFmt(e));
        }
        Ok(())
    }

    fn move_to_top_of_stack(&mut self, window: &Rc<XwindowData>) {
        let link = self.stack_list.add_last(window.clone());
        *window.stack_link.borrow_mut() = Some(link);
    }

    async fn configure_stack_position(&mut self, window: &Rc<XwindowData>) {
        let sl = window.stack_link.borrow_mut();
        let sl = match sl.deref() {
            Some(sl) => sl,
            _ => return,
        };
        let (sibling, stack_mode) = match sl.prev() {
            Some(n) => (Some(n), STACK_MODE_ABOVE),
            _ => match sl.next() {
                Some(n) => (Some(n), STACK_MODE_BELOW),
                _ => (None, STACK_MODE_ABOVE),
            },
        };
        let res = self
            .c
            .call(&ConfigureWindow {
                window: window.window_id,
                values: ConfigureWindowValues {
                    sibling: sibling.map(|s| s.window_id),
                    stack_mode: Some(stack_mode),
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
            let mut x = event.x as i32;
            let mut y = event.y as i32;
            let mut width = event.width as i32;
            let mut height = event.height as i32;
            client_wire_scale_to_logical!(self.client, x, y, width, height);
            let extents = Rect::new_sized(x, y, width, height).unwrap();
            if let Some(window) = data.window.get() {
                window.tl_change_extents(&extents);
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
            client_wire_scale_to_logical!(self.client, x1);
        }
        if event.value_mask.contains(CONFIG_WINDOW_Y) {
            y1 = event.y as _;
            client_wire_scale_to_logical!(self.client, y1);
        }
        if event.value_mask.contains(CONFIG_WINDOW_WIDTH) {
            width = event.width as _;
            client_wire_scale_to_logical!(self.client, width);
        }
        if event.value_mask.contains(CONFIG_WINDOW_HEIGHT) {
            height = event.height as _;
            client_wire_scale_to_logical!(self.client, height);
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
            if w.toplevel_data.active_surfaces.active() {
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
        if data.info.pid.is_none() || data.info.pid.get() != fw.info.pid.get() {
            return Ok(());
        }
        let win = match data.window.get() {
            Some(w) => w,
            _ => return Ok(()),
        };
        if win.toplevel_data.visible.get() {
            let seats = self.state.globals.seats.lock();
            for (_, seat) in seats.deref() {
                seat.focus_toplevel(win.clone());
            }
        } else {
            win.x.surface.request_activation();
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
        if fullscreen != data.info.fullscreen.get() {
            if let Some(w) = data.window.get() {
                w.tl_set_fullscreen(fullscreen);
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

    async fn handle_wl_surface_serial(
        &mut self,
        event: &ClientMessage<'_>,
    ) -> Result<(), XWaylandError> {
        let serial = event.data[0] as u64 | ((event.data[1] as u64) << 32);
        if serial <= self.last_surface_serial {
            log::error!(
                "Surface serial is not monotonic: {} <= {}",
                serial,
                self.last_surface_serial
            );
            return Ok(());
        }
        self.last_surface_serial = serial;
        let data = match self.windows.get(&event.window) {
            Some(d) => d.clone(),
            _ => return Ok(()),
        };
        if let Some(old) = data.surface_serial.replace(Some(serial)) {
            self.windows_by_surface_serial.remove(&old);
        }
        if let Some(old) = data.window.take() {
            old.break_loops();
        }
        self.windows_by_surface_serial.insert(serial, data.clone());
        if let Some(surface) = self.client.surfaces_by_xwayland_serial.get(&serial) {
            self.create_window(&data, surface).await;
        }
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
        if data.surface_id.is_some() {
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

struct XToWaylandTransfer {
    id: u64,
    data: Buf,
    fd: Rc<OwnedFd>,
    state: Rc<State>,
    shared: Rc<XwmShared>,
}

impl XToWaylandTransfer {
    async fn run(mut self) {
        let timeout = self.state.now() + Duration::from_millis(5000);
        let mut pos = 0;
        while pos < self.data.len() {
            let res = self
                .state
                .ring
                .write(&self.fd, self.data.slice(pos..), Some(timeout));
            match res.await {
                Ok(n) => pos += n,
                Err(IoUringError::OsError(OsError(c::ECANCELED))) => {
                    log::error!("Transfer timed out");
                    break;
                }
                Err(e) => {
                    log::error!("Could not write to wayland client: {}", ErrorFmt(e));
                    break;
                }
            }
        }
        self.shared.transfers.remove(&self.id);
    }
}

struct WaylandToXTransfer {
    id: u64,
    fd: Rc<OwnedFd>,
    ring: Rc<IoUring>,
    c: Rc<Xcon>,
    window: u32,
    time: u32,
    property: u32,
    ty: u32,
    selection: u32,
    shared: Rc<XwmShared>,
}

impl WaylandToXTransfer {
    async fn run(self) {
        let mut success = false;
        let mut buf = Buf::new(1024);
        loop {
            match self.ring.read(&self.fd, buf.clone()).await {
                Ok(0) => {
                    success = true;
                    break;
                }
                Ok(n) => {
                    let cp = ChangeProperty {
                        mode: PROP_MODE_APPEND,
                        window: self.window,
                        property: self.property,
                        ty: self.ty,
                        format: 8,
                        data: &buf[..n],
                    };
                    if let Err(e) = self.c.call(&cp).await {
                        log::error!("Could not append data to property: {}", ErrorFmt(e));
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Could not read from wayland client: {}", ErrorFmt(e));
                    break;
                }
            }
        }
        let target = match success {
            true => self.ty,
            false => ATOM_NONE,
        };
        let sn = SelectionNotify {
            time: self.time,
            requestor: self.window,
            selection: self.selection,
            target,
            property: self.property,
        };
        if let Err(e) = self.c.send_event(false, self.window, 0, &sn).await {
            log::error!("Could not send event: {}", ErrorFmt(e));
        }
        self.shared.transfers.remove(&self.id);
    }
}
