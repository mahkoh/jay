mod event_handling;
pub mod ext_transient_seat_manager_v1;
pub mod ext_transient_seat_v1;
mod gesture_owner;
mod kb_owner;
mod pointer_owner;
pub mod tablet;
pub mod text_input;
mod touch_owner;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;
pub mod zwp_pointer_constraints_v1;
pub mod zwp_pointer_gesture_hold_v1;
pub mod zwp_pointer_gesture_pinch_v1;
pub mod zwp_pointer_gesture_swipe_v1;
pub mod zwp_pointer_gestures_v1;
pub mod zwp_relative_pointer_manager_v1;
pub mod zwp_relative_pointer_v1;
pub mod zwp_virtual_keyboard_manager_v1;
pub mod zwp_virtual_keyboard_v1;

use {
    crate::{
        async_engine::SpawnedFuture,
        backend::KeyState,
        client::{Client, ClientError, ClientId},
        cursor_user::{CursorUser, CursorUserGroup, CursorUserOwner},
        ei::ei_ifs::ei_seat::EiSeat,
        fixed::Fixed,
        globals::{Global, GlobalName},
        ifs::{
            ext_idle_notification_v1::ExtIdleNotificationV1,
            ipc::{
                self,
                data_control::{DataControlDeviceId, DynDataControlDevice},
                offer_source_to_regular_client,
                wl_data_device::{ClipboardIpc, WlDataDevice},
                wl_data_source::WlDataSource,
                x_data_device::{XClipboardIpc, XIpcDevice, XIpcDeviceId, XPrimarySelectionIpc},
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
                zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
                DynDataSource, IpcError, IpcLocation,
            },
            wl_output::WlOutputGlobal,
            wl_seat::{
                gesture_owner::GestureOwnerHolder,
                kb_owner::KbOwnerHolder,
                pointer_owner::PointerOwnerHolder,
                tablet::TabletSeatData,
                text_input::{
                    zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
                    zwp_input_method_v2::ZwpInputMethodV2, zwp_text_input_v3::ZwpTextInputV3,
                },
                touch_owner::TouchOwnerHolder,
                wl_keyboard::{WlKeyboard, WlKeyboardError, REPEAT_INFO_SINCE},
                wl_pointer::WlPointer,
                wl_touch::WlTouch,
                zwp_pointer_constraints_v1::{SeatConstraint, SeatConstraintStatus},
                zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1,
                zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
                zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
                zwp_relative_pointer_v1::ZwpRelativePointerV1,
            },
            wl_surface::{
                dnd_icon::DndIcon,
                tray::{DynTrayItem, TrayItemId},
                xdg_surface::xdg_popup::XdgPopup,
                WlSurface,
            },
            xdg_toplevel_drag_v1::XdgToplevelDragV1,
        },
        kbvm::{KbvmMap, KbvmMapId, KbvmState, PhysicalKeyboardState},
        keyboard::{DynKeyboardState, KeyboardState, KeyboardStateId, KeymapFd},
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        state::{DeviceHandlerData, State},
        tree::{
            generic_node_visitor, ContainerNode, ContainerSplit, Direction, FoundNode, Node,
            OutputNode, ToplevelNode, WorkspaceNode,
        },
        utils::{
            asyncevent::AsyncEvent, bindings::PerClientBindings, clonecell::CloneCell,
            copyhashmap::CopyHashMap, linkedlist::LinkedNode, numcell::NumCell, rc_eq::rc_eq,
            smallmap::SmallMap,
        },
        wire::{
            wl_seat::*, ExtIdleNotificationV1Id, WlDataDeviceId, WlKeyboardId, WlPointerId,
            WlSeatId, WlTouchId, XdgPopupId, ZwpPrimarySelectionDeviceV1Id, ZwpRelativePointerV1Id,
            ZwpTextInputV3Id,
        },
        wire_ei::EiSeatId,
    },
    ahash::AHashMap,
    smallvec::SmallVec,
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        mem,
        ops::{Deref, DerefMut},
        rc::{Rc, Weak},
    },
    thiserror::Error,
};
pub use {
    event_handling::NodeSeatState,
    pointer_owner::{ToplevelSelector, WorkspaceSelector},
};

pub const POINTER: u32 = 1;
const KEYBOARD: u32 = 2;
const TOUCH: u32 = 4;

#[expect(dead_code)]
const MISSING_CAPABILITY: u32 = 0;

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;

pub const SEAT_NAME_SINCE: Version = Version(2);

pub const PX_PER_SCROLL: f64 = 15.0;

#[derive(Clone)]
pub struct Dnd {
    pub seat: Rc<WlSeatGlobal>,
    client: Rc<Client>,
    src: Option<Rc<WlDataSource>>,
}

pub struct DroppedDnd {
    dnd: Dnd,
}

impl Drop for DroppedDnd {
    fn drop(&mut self) {
        if let Some(src) = self.dnd.src.take() {
            ipc::detach_seat(&*src, &self.dnd.seat);
        }
    }
}

linear_ids!(PhysicalKeyboardIds, PhysicalKeyboardId, u64);

pub struct PhysicalKeyboard {
    has_custom_map: Cell<bool>,
    pub phy_state: PhysicalKeyboardState,
}

linear_ids!(SeatIds, SeatId);

pub struct WlSeatGlobal {
    id: SeatId,
    name: GlobalName,
    state: Rc<State>,
    seat_name: String,
    capabilities: Cell<u32>,
    num_touch_devices: NumCell<u32>,
    pos_time_usec: Cell<u64>,
    pointer_stack: RefCell<Vec<Rc<dyn Node>>>,
    pointer_stack_modified: Cell<bool>,
    found_tree: RefCell<Vec<FoundNode>>,
    keyboard_node: CloneCell<Rc<dyn Node>>,
    bindings: RefCell<AHashMap<ClientId, AHashMap<WlSeatId, Rc<WlSeat>>>>,
    x_data_devices: SmallMap<XIpcDeviceId, Rc<XIpcDevice>, 1>,
    data_devices: RefCell<AHashMap<ClientId, AHashMap<WlDataDeviceId, Rc<WlDataDevice>>>>,
    primary_selection_devices: RefCell<
        AHashMap<
            ClientId,
            AHashMap<ZwpPrimarySelectionDeviceV1Id, Rc<ZwpPrimarySelectionDeviceV1>>,
        >,
    >,
    data_control_devices: CopyHashMap<DataControlDeviceId, Rc<dyn DynDataControlDevice>>,
    repeat_rate: Cell<(i32, i32)>,
    seat_kb_map: CloneCell<Rc<KbvmMap>>,
    seat_kb_state: CloneCell<Rc<RefCell<KbvmState>>>,
    latest_kb_state: CloneCell<Rc<dyn DynKeyboardState>>,
    latest_kb_state_id: Cell<KeyboardStateId>,
    kb_states: CopyHashMap<KbvmMapId, Weak<RefCell<KbvmState>>>,
    kb_devices: CopyHashMap<PhysicalKeyboardId, Rc<PhysicalKeyboard>>,
    cursor_user_group: Rc<CursorUserGroup>,
    pointer_cursor: Rc<CursorUser>,
    tree_changed: Rc<AsyncEvent>,
    tree_changed_needs_layout: Cell<bool>,
    selection: CloneCell<Option<Rc<dyn DynDataSource>>>,
    selection_serial: Cell<u64>,
    primary_selection: CloneCell<Option<Rc<dyn DynDataSource>>>,
    primary_selection_serial: Cell<u64>,
    pointer_owner: PointerOwnerHolder,
    kb_owner: KbOwnerHolder,
    gesture_owner: GestureOwnerHolder,
    touch_owner: TouchOwnerHolder,
    dropped_dnd: RefCell<Option<DroppedDnd>>,
    shortcuts: RefCell<AHashMap<u32, SmallMap<u32, u32, 2>>>,
    queue_link: RefCell<Option<LinkedNode<Rc<Self>>>>,
    tree_changed_handler: Cell<Option<SpawnedFuture<()>>>,
    changes: NumCell<u32>,
    constraint: CloneCell<Option<Rc<SeatConstraint>>>,
    idle_notifications: CopyHashMap<(ClientId, ExtIdleNotificationV1Id), Rc<ExtIdleNotificationV1>>,
    last_input_usec: Cell<u64>,
    text_inputs: RefCell<AHashMap<ClientId, CopyHashMap<ZwpTextInputV3Id, Rc<ZwpTextInputV3>>>>,
    text_input: CloneCell<Option<Rc<ZwpTextInputV3>>>,
    input_method: CloneCell<Option<Rc<ZwpInputMethodV2>>>,
    input_method_grab: CloneCell<Option<Rc<ZwpInputMethodKeyboardGrabV2>>>,
    forward: Cell<bool>,
    focus_follows_mouse: Cell<bool>,
    swipe_bindings: PerClientBindings<ZwpPointerGestureSwipeV1>,
    pinch_bindings: PerClientBindings<ZwpPointerGesturePinchV1>,
    hold_bindings: PerClientBindings<ZwpPointerGestureHoldV1>,
    tablet: TabletSeatData,
    ei_seats: CopyHashMap<(ClientId, EiSeatId), Rc<EiSeat>>,
    ui_drag_highlight: Cell<Option<Rect>>,
    keyboard_node_serial: Cell<u64>,
    tray_popups: CopyHashMap<(TrayItemId, XdgPopupId), Rc<dyn DynTrayItem>>,
}

const CHANGE_CURSOR_MOVED: u32 = 1 << 0;
const CHANGE_TREE: u32 = 1 << 1;

impl WlSeatGlobal {
    pub fn new(name: GlobalName, seat_name: &str, state: &Rc<State>) -> Rc<Self> {
        let seat_kb_state = state.default_keymap.state(state.keyboard_state_ids.next());
        let latest_kb_state_id = seat_kb_state.kb_state.id;
        let seat_kb_state = Rc::new(RefCell::new(seat_kb_state));
        let kb_states = CopyHashMap::new();
        kb_states.set(state.default_keymap.id, Rc::downgrade(&seat_kb_state));
        let cursor_user_group = CursorUserGroup::create(state);
        let cursor_user = cursor_user_group.create_user();
        cursor_user.activate();
        let slf = Rc::new(Self {
            id: state.seat_ids.next(),
            name,
            state: state.clone(),
            seat_name: seat_name.to_string(),
            capabilities: Cell::new(0),
            num_touch_devices: Default::default(),
            pos_time_usec: Cell::new(0),
            pointer_stack: RefCell::new(vec![]),
            pointer_stack_modified: Cell::new(false),
            found_tree: RefCell::new(vec![]),
            keyboard_node: CloneCell::new(state.root.clone()),
            keyboard_node_serial: Default::default(),
            bindings: Default::default(),
            x_data_devices: Default::default(),
            data_devices: RefCell::new(Default::default()),
            primary_selection_devices: RefCell::new(Default::default()),
            repeat_rate: Cell::new((25, 250)),
            seat_kb_map: CloneCell::new(state.default_keymap.clone()),
            seat_kb_state: CloneCell::new(seat_kb_state.clone()),
            latest_kb_state: CloneCell::new(seat_kb_state.clone()),
            latest_kb_state_id: Cell::new(latest_kb_state_id),
            kb_states,
            kb_devices: Default::default(),
            cursor_user_group,
            pointer_cursor: cursor_user,
            tree_changed: Default::default(),
            tree_changed_needs_layout: Default::default(),
            selection: Default::default(),
            selection_serial: Cell::new(0),
            primary_selection: Default::default(),
            primary_selection_serial: Cell::new(0),
            pointer_owner: Default::default(),
            kb_owner: Default::default(),
            gesture_owner: Default::default(),
            touch_owner: Default::default(),
            dropped_dnd: RefCell::new(None),
            shortcuts: Default::default(),
            queue_link: Default::default(),
            tree_changed_handler: Cell::new(None),
            changes: NumCell::new(CHANGE_CURSOR_MOVED | CHANGE_TREE),
            constraint: Default::default(),
            idle_notifications: Default::default(),
            last_input_usec: Cell::new(state.now_usec()),
            data_control_devices: Default::default(),
            text_inputs: Default::default(),
            text_input: Default::default(),
            input_method: Default::default(),
            input_method_grab: Default::default(),
            forward: Cell::new(false),
            focus_follows_mouse: Cell::new(true),
            swipe_bindings: Default::default(),
            pinch_bindings: Default::default(),
            hold_bindings: Default::default(),
            tablet: Default::default(),
            ei_seats: Default::default(),
            ui_drag_highlight: Default::default(),
            tray_popups: Default::default(),
        });
        slf.pointer_cursor.set_owner(slf.clone());
        let seat = slf.clone();
        let future = state.eng.spawn("seat handler", async move {
            loop {
                seat.tree_changed.triggered().await;
                if seat.tree_changed_needs_layout.take() {
                    seat.state.eng.yield_now().await;
                }
                seat.state.tree_changed_sent.set(false);
                seat.changes.or_assign(CHANGE_TREE);
                // log::info!("tree_changed");
                seat.apply_changes();
            }
        });
        slf.tree_changed_handler.set(Some(future));
        slf.update_capabilities();
        slf
    }

    pub fn update_capabilities(&self) {
        let mut caps = POINTER | KEYBOARD;
        if self.num_touch_devices.get() > 0 {
            caps |= TOUCH;
        } else {
            if self.ei_seats.lock().values().any(|s| s.is_touch_input()) {
                caps |= TOUCH;
            }
        }
        if self.capabilities.replace(caps) != caps {
            for client in self.bindings.borrow().values() {
                for seat in client.values() {
                    seat.send_capabilities();
                }
            }
        }
    }

    pub fn keymap(&self) -> Rc<KbvmMap> {
        self.seat_kb_map.get()
    }

    pub fn input_method(&self) -> Option<Rc<ZwpInputMethodV2>> {
        self.input_method.get()
    }

    pub fn toplevel_drag(&self) -> Option<Rc<XdgToplevelDragV1>> {
        self.pointer_owner.toplevel_drag()
    }

    pub fn ui_drag_highlight(&self) -> Option<Rect> {
        self.ui_drag_highlight.get()
    }

    pub fn add_data_device(&self, device: &Rc<WlDataDevice>) {
        let mut dd = self.data_devices.borrow_mut();
        dd.entry(device.client.id)
            .or_default()
            .insert(device.id, device.clone());
    }

    pub fn set_x_data_device(&self, device: &Rc<XIpcDevice>) {
        self.x_data_devices.insert(device.id, device.clone());
    }

    pub fn unset_x_data_device(&self, id: XIpcDeviceId) {
        self.x_data_devices.remove(&id);
    }

    pub fn for_each_x_data_device(&self, mut f: impl FnMut(&Rc<XIpcDevice>)) {
        for (_, dev) in &self.x_data_devices {
            f(&dev);
        }
    }

    pub fn remove_data_device(&self, device: &WlDataDevice) {
        let mut dd = self.data_devices.borrow_mut();
        if let Entry::Occupied(mut e) = dd.entry(device.client.id) {
            e.get_mut().remove(&device.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    pub fn add_primary_selection_device(&self, device: &Rc<ZwpPrimarySelectionDeviceV1>) {
        let mut dd = self.primary_selection_devices.borrow_mut();
        dd.entry(device.client.id)
            .or_default()
            .insert(device.id, device.clone());
    }

    pub fn remove_primary_selection_device(&self, device: &ZwpPrimarySelectionDeviceV1) {
        let mut dd = self.primary_selection_devices.borrow_mut();
        if let Entry::Occupied(mut e) = dd.entry(device.client.id) {
            e.get_mut().remove(&device.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    pub fn add_data_control_device(&self, device: Rc<dyn DynDataControlDevice>) {
        self.data_control_devices.set(device.id(), device.clone());
    }

    pub fn remove_data_control_device(&self, device: &dyn DynDataControlDevice) {
        self.data_control_devices.remove(&device.id());
    }

    pub fn get_output(&self) -> Rc<OutputNode> {
        self.cursor_user_group.latest_output()
    }

    pub fn set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        if tl.tl_data().is_fullscreen.get() {
            return;
        }
        let old_ws = match tl.tl_data().workspace.get() {
            Some(ws) => ws,
            _ => return,
        };
        if old_ws.id == ws.id {
            return;
        }
        let cn = match tl.tl_data().parent.get() {
            Some(cn) => cn,
            _ => return,
        };
        let kb_foci = collect_kb_foci(tl.clone().tl_into_node());
        cn.cnode_remove_child2(tl.tl_as_node(), true);
        if !ws.visible.get() {
            for focus in kb_foci {
                old_ws.clone().node_do_focus(&focus, Direction::Unspecified);
            }
        }
        if tl.tl_data().is_floating.get() {
            self.state.map_floating(
                tl.clone(),
                tl.tl_data().float_width.get(),
                tl.tl_data().float_height.get(),
                ws,
                None,
            );
        } else {
            self.state.map_tiled_on(tl, ws);
        }
    }

    pub fn mark_last_active(self: &Rc<Self>) {
        let link = &mut *self.queue_link.borrow_mut();
        if let Some(link) = link {
            self.state.seat_queue.add_last_existing(link)
        } else {
            *link = Some(self.state.seat_queue.add_last(self.clone()))
        }
    }

    pub fn disable_pointer_constraint(&self) {
        if let Some(constraint) = self.constraint.get() {
            constraint.deactivate();
            if constraint.status.get() == SeatConstraintStatus::Inactive {
                constraint
                    .status
                    .set(SeatConstraintStatus::ActivatableOnFocus);
            }
        }
    }

    fn maybe_constrain_pointer_node(&self) {
        if let Some(pn) = self.pointer_node() {
            if let Some(surface) = pn.node_into_surface() {
                let (mut x, mut y) = self.pointer_cursor.position();
                let (sx, sy) = surface.buffer_abs_pos.get().position();
                x -= Fixed::from_int(sx);
                y -= Fixed::from_int(sy);
                self.maybe_constrain(&surface, x, y);
            }
        }
    }

    fn maybe_constrain(&self, surface: &WlSurface, x: Fixed, y: Fixed) {
        if self.constraint.is_some() {
            return;
        }
        let candidate = match surface.constraints.get(&self.id) {
            Some(c) if c.status.get() == SeatConstraintStatus::Inactive => c,
            _ => return,
        };
        if !candidate.contains(x.round_down(), y.round_down()) {
            return;
        }
        candidate.status.set(SeatConstraintStatus::Active);
        if let Some(owner) = candidate.owner.get() {
            owner.send_enabled();
        }
        self.constraint.set(Some(candidate));
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            tl.tl_set_fullscreen(fullscreen);
        }
    }

    pub fn get_fullscreen(&self) -> bool {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            return tl.tl_data().is_fullscreen.get();
        }
        false
    }

    pub fn set_seat_keymap(&self, keymap: &Rc<KbvmMap>) {
        self.seat_kb_map.set(keymap.clone());
        let new = self.get_kb_state(keymap);
        let old = self.seat_kb_state.set(new.clone());
        if rc_eq(&old, &new) {
            return;
        }
        self.kb_devices.lock().retain(|_, p| p.has_custom_map.get());
        self.handle_keyboard_state_change(&old.borrow().kb_state, &new.borrow().kb_state);
    }

    fn handle_keyboard_state_change(&self, old: &KeyboardState, new: &KeyboardState) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.handle_keyboard_state_change(old.id, new);
        });
        let Some(surface) = self.keyboard_node.get().node_into_surface() else {
            return;
        };
        let serial = surface.client.next_serial();
        self.surface_kb_event(Version::ALL, &surface, |kb| {
            if kb.kb_state_id() == old.id {
                kb.send_leave(serial, surface.id);
                kb.enter(serial, surface.id, new);
            }
        });
    }

    pub fn get_kb_state(&self, keymap: &Rc<KbvmMap>) -> Rc<RefCell<KbvmState>> {
        if let Some(weak) = self.kb_states.get(&keymap.id) {
            if let Some(state) = weak.upgrade() {
                return state;
            }
        }
        self.kb_states
            .lock()
            .retain(|_, state| state.strong_count() > 0);
        let s = keymap.state(self.state.keyboard_state_ids.next());
        let s = Rc::new(RefCell::new(s));
        self.kb_states.set(keymap.id, Rc::downgrade(&s));
        s
    }

    pub fn prepare_for_lock(self: &Rc<Self>) {
        self.pointer_owner.revert_to_default(self);
        self.kb_owner.ungrab(self);
    }

    pub fn kb_parent_container(&self) -> Option<Rc<ContainerNode>> {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            if let Some(parent) = tl.tl_data().parent.get() {
                if let Some(container) = parent.node_into_container() {
                    return Some(container);
                }
            }
        }
        None
    }

    pub fn get_mono(&self) -> Option<bool> {
        self.kb_parent_container().map(|c| c.mono_child.is_some())
    }

    pub fn get_split(&self) -> Option<ContainerSplit> {
        self.kb_parent_container().map(|c| c.split.get())
    }

    pub fn set_mono(&self, mono: bool) {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            if let Some(parent) = tl.tl_data().parent.get() {
                if let Some(container) = parent.node_into_container() {
                    let node = if mono { Some(tl.deref()) } else { None };
                    container.set_mono(node);
                }
            }
        }
    }

    pub fn set_split(&self, axis: ContainerSplit) {
        if let Some(c) = self.kb_parent_container() {
            c.set_split(axis);
        }
    }

    pub fn create_split(&self, axis: ContainerSplit) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        if tl.tl_data().is_fullscreen.get() {
            return;
        }
        let ws = match tl.tl_data().workspace.get() {
            Some(ws) => ws,
            _ => return,
        };
        let pn = match tl.tl_data().parent.get() {
            Some(pn) => pn,
            _ => return,
        };
        if let Some(pn) = pn.node_into_containing_node() {
            let cn = ContainerNode::new(&self.state, &ws, tl.clone(), axis);
            pn.cnode_replace_child(tl.tl_as_node(), cn);
        }
    }

    pub fn focus_parent(self: &Rc<Self>) {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            if let Some(parent) = tl.tl_data().parent.get() {
                if let Some(tl) = parent.node_toplevel() {
                    self.focus_node(tl.tl_into_node());
                }
            }
        }
    }

    pub fn get_floating(self: &Rc<Self>) -> Option<bool> {
        match self.keyboard_node.get().node_toplevel() {
            Some(tl) => Some(tl.tl_data().is_floating.get()),
            _ => None,
        }
    }

    pub fn set_floating(self: &Rc<Self>, floating: bool) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        self.set_tl_floating(tl, floating);
    }

    pub fn set_tl_floating(self: &Rc<Self>, tl: Rc<dyn ToplevelNode>, floating: bool) {
        let data = tl.tl_data();
        if data.is_fullscreen.get() {
            return;
        }
        if data.is_floating.get() == floating {
            return;
        }
        let parent = match data.parent.get() {
            Some(p) => p,
            _ => return,
        };
        if !floating {
            parent.cnode_remove_child2(tl.tl_as_node(), true);
            self.state.map_tiled(tl);
        } else if let Some(ws) = data.workspace.get() {
            parent.cnode_remove_child2(tl.tl_as_node(), true);
            let (width, height) = data.float_size(&ws);
            self.state.map_floating(tl, width, height, &ws, None);
        }
    }

    pub fn get_rate(&self) -> (i32, i32) {
        self.repeat_rate.get()
    }

    pub fn set_rate(&self, rate: i32, delay: i32) {
        self.repeat_rate.set((rate, delay));
        let bindings = self.bindings.borrow_mut();
        for client in bindings.values() {
            for seat in client.values() {
                if seat.version >= REPEAT_INFO_SINCE {
                    let kbs = seat.keyboards.lock();
                    for kb in kbs.values() {
                        kb.send_repeat_info(rate, delay);
                    }
                }
            }
        }
        if let Some(grab) = self.input_method_grab.get() {
            grab.send_repeat_info();
        }
    }

    pub fn close(self: &Rc<Self>) {
        let kb_node = self.keyboard_node.get();
        if let Some(tl) = kb_node.node_toplevel() {
            tl.tl_close();
        }
    }

    pub fn move_focus(self: &Rc<Self>, direction: Direction) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        if direction == Direction::Down && tl.node_is_container() {
            tl.node_do_focus(self, direction);
        } else if let Some(p) = tl.tl_data().parent.get() {
            if let Some(c) = p.node_into_container() {
                c.move_focus_from_child(self, tl.deref(), direction);
            }
        }
    }

    pub fn move_focused(self: &Rc<Self>, direction: Direction) {
        let kb_node = self.keyboard_node.get();
        if let Some(tl) = kb_node.node_toplevel() {
            if let Some(parent) = tl.tl_data().parent.get() {
                if let Some(c) = parent.node_into_container() {
                    c.move_child(tl, direction);
                }
            }
        }
    }

    fn set_selection_<T, X, S>(
        self: &Rc<Self>,
        field: &CloneCell<Option<Rc<dyn DynDataSource>>>,
        src: Option<Rc<S>>,
        location: IpcLocation,
    ) -> Result<(), WlSeatError>
    where
        T: ipc::IterableIpcVtable,
        X: ipc::IpcVtable<Device = XIpcDevice>,
        S: DynDataSource,
    {
        if let (Some(new), Some(old)) = (&src, &field.get()) {
            if new.source_data().id == old.source_data().id {
                return Ok(());
            }
        }
        if let Some(new) = &src {
            ipc::attach_seat(&**new, self, ipc::Role::Selection)?;
        }
        let src_dyn = src.clone().map(|s| s as Rc<dyn DynDataSource>);
        if let Some(old) = field.set(src_dyn) {
            old.detach_seat(self);
        }
        if let Some(client) = self.keyboard_node.get().node_client() {
            self.offer_selection_to_client::<T, X>(src.clone().map(|v| v as Rc<_>), &client);
            // client.flush();
        }
        let dyn_source = src.map(|s| s as Rc<dyn DynDataSource>);
        for dd in self.data_control_devices.lock().values() {
            dd.clone().handle_new_source(location, dyn_source.clone());
        }
        Ok(())
    }

    fn offer_selection_to_client<T, X>(
        &self,
        selection: Option<Rc<dyn DynDataSource>>,
        client: &Rc<Client>,
    ) where
        T: ipc::IterableIpcVtable,
        X: ipc::IpcVtable<Device = XIpcDevice>,
    {
        if let Some(src) = &selection {
            src.cancel_unprivileged_offers();
        }
        if client.is_xwayland {
            self.for_each_x_data_device(|dd| match &selection {
                Some(src) => src.clone().offer_to_x(&dd),
                _ => X::send_selection(&dd, None),
            });
        } else {
            match selection {
                Some(src) => offer_source_to_regular_client::<T>(src, client),
                _ => T::for_each_device(self, client.id, |device| {
                    T::send_selection(device, None);
                }),
            }
        }
    }

    pub fn start_drag(
        self: &Rc<Self>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<DndIcon>>,
        serial: u64,
    ) -> Result<(), WlSeatError> {
        if let Some(icon) = &icon {
            icon.surface().set_output(&self.pointer_cursor.output());
        }
        self.pointer_owner
            .start_drag(self, origin, source, icon, serial)
    }

    pub fn start_tile_drag(self: &Rc<Self>, tl: &Rc<dyn ToplevelNode>) {
        if self.state.ui_drag_enabled.get() {
            self.pointer_owner.start_tile_drag(self, tl);
        }
    }

    pub fn start_workspace_drag(self: &Rc<Self>, ws: &Rc<WorkspaceNode>) {
        if self.state.ui_drag_enabled.get() {
            self.pointer_owner.start_workspace_drag(self, ws);
        }
    }

    pub fn cancel_dnd(self: &Rc<Self>) {
        self.pointer_owner.cancel_dnd(self);
    }

    pub fn unset_selection(self: &Rc<Self>) {
        let _ = self.set_wl_data_source_selection(None, None);
    }

    pub fn set_wl_data_source_selection(
        self: &Rc<Self>,
        selection: Option<Rc<WlDataSource>>,
        serial: Option<u64>,
    ) -> Result<(), WlSeatError> {
        if let Some(serial) = serial {
            self.selection_serial.set(serial);
        }
        if let Some(selection) = &selection {
            if selection.toplevel_drag.is_some() {
                return Err(WlSeatError::OfferHasDrag);
            }
        }
        self.set_selection(selection)
    }

    pub fn set_selection<S: DynDataSource>(
        self: &Rc<Self>,
        selection: Option<Rc<S>>,
    ) -> Result<(), WlSeatError> {
        self.set_selection_::<ClipboardIpc, XClipboardIpc, _>(
            &self.selection,
            selection,
            IpcLocation::Clipboard,
        )
    }

    pub fn get_selection(&self) -> Option<Rc<dyn DynDataSource>> {
        self.selection.get()
    }

    pub fn may_modify_selection(&self, client: &Rc<Client>, serial: u64) -> bool {
        if serial < self.selection_serial.get() {
            return false;
        }
        self.keyboard_node.get().node_client_id() == Some(client.id)
    }

    pub fn may_modify_primary_selection(&self, client: &Rc<Client>, serial: Option<u64>) -> bool {
        if let Some(serial) = serial {
            if serial < self.primary_selection_serial.get() {
                return false;
            }
        }
        self.keyboard_node.get().node_client_id() == Some(client.id)
            || self.pointer_node().and_then(|n| n.node_client_id()) == Some(client.id)
    }

    pub fn unset_primary_selection(self: &Rc<Self>) {
        let _ = self.set_zwp_primary_selection(None, None);
    }

    pub fn set_zwp_primary_selection(
        self: &Rc<Self>,
        selection: Option<Rc<ZwpPrimarySelectionSourceV1>>,
        serial: Option<u64>,
    ) -> Result<(), WlSeatError> {
        if let Some(serial) = serial {
            self.primary_selection_serial.set(serial);
        }
        self.set_primary_selection(selection)
    }

    pub fn set_primary_selection<S: DynDataSource>(
        self: &Rc<Self>,
        selection: Option<Rc<S>>,
    ) -> Result<(), WlSeatError> {
        self.set_selection_::<PrimarySelectionIpc, XPrimarySelectionIpc, _>(
            &self.primary_selection,
            selection,
            IpcLocation::PrimarySelection,
        )
    }

    pub fn get_primary_selection(&self) -> Option<Rc<dyn DynDataSource>> {
        self.primary_selection.get()
    }

    pub fn dnd_icon(&self) -> Option<Rc<DndIcon>> {
        self.pointer_owner.dnd_icon()
    }

    pub fn remove_dnd_icon(&self) {
        self.pointer_owner.remove_dnd_icon();
    }

    pub fn pointer_cursor(&self) -> &Rc<CursorUser> {
        &self.pointer_cursor
    }

    pub fn cursor_group(&self) -> &Rc<CursorUserGroup> {
        &self.cursor_user_group
    }

    pub fn clear(self: &Rc<Self>) {
        mem::take(self.pointer_stack.borrow_mut().deref_mut());
        mem::take(self.found_tree.borrow_mut().deref_mut());
        self.keyboard_node.set(self.state.root.clone());
        self.state
            .root
            .clone()
            .node_visit(&mut generic_node_visitor(|node| {
                node.node_seat_state().on_seat_remove(self);
            }));
        self.bindings.borrow_mut().clear();
        self.data_devices.borrow_mut().clear();
        self.primary_selection_devices.borrow_mut().clear();
        self.data_control_devices.clear();
        self.cursor_user_group.detach();
        self.selection.set(None);
        self.primary_selection.set(None);
        self.pointer_owner.clear();
        self.kb_owner.clear();
        self.touch_owner.clear();
        *self.dropped_dnd.borrow_mut() = None;
        self.queue_link.take();
        self.tree_changed_handler.set(None);
        self.constraint.take();
        self.text_inputs.borrow_mut().clear();
        self.text_input.take();
        self.input_method.take();
        self.input_method_grab.take();
        self.swipe_bindings.clear();
        self.pinch_bindings.clear();
        self.hold_bindings.clear();
        self.cursor_user_group.detach();
        self.tablet_clear();
        self.ei_seats.clear();
    }

    pub fn id(&self) -> SeatId {
        self.id
    }

    pub fn seat_name(&self) -> &str {
        &self.seat_name
    }

    fn bind_(
        self: Rc<Self>,
        id: WlSeatId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlSeatError> {
        let obj = Rc::new(WlSeat {
            global: self.clone(),
            id,
            client: client.clone(),
            pointers: Default::default(),
            relative_pointers: Default::default(),
            keyboards: Default::default(),
            touches: Default::default(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_capabilities();
        if version >= SEAT_NAME_SINCE {
            obj.send_name(&self.seat_name);
        }
        {
            let mut bindings = self.bindings.borrow_mut();
            let bindings = bindings.entry(client.id).or_default();
            bindings.insert(id, obj.clone());
        }
        Ok(())
    }

    pub fn add_idle_notification(&self, notification: &Rc<ExtIdleNotificationV1>) {
        self.idle_notifications.set(
            (notification.client.id, notification.id),
            notification.clone(),
        );
    }

    pub fn remove_idle_notification(&self, notification: &ExtIdleNotificationV1) {
        self.idle_notifications
            .remove(&(notification.client.id, notification.id));
    }

    pub fn last_input(&self) -> u64 {
        self.last_input_usec.get()
    }

    pub fn set_visible(&self, visible: bool) {
        self.cursor_user_group.set_visible(visible);
        if let Some(icon) = self.dnd_icon() {
            icon.surface().set_visible(visible);
        }
        if let Some(tl_drag) = self.toplevel_drag() {
            if let Some(tl) = tl_drag.toplevel.get() {
                tl.tl_set_visible(visible);
            }
        }
        if let Some(im) = self.input_method.get() {
            for (_, popup) in &im.popups {
                popup.update_visible();
            }
        }
    }

    pub fn set_forward(&self, forward: bool) {
        self.forward.set(forward);
    }

    pub fn select_toplevel(self: &Rc<Self>, selector: impl ToplevelSelector) {
        self.pointer_owner.select_toplevel(self, selector);
    }

    pub fn select_workspace(self: &Rc<Self>, selector: impl WorkspaceSelector) {
        self.pointer_owner.select_workspace(self, selector);
    }

    pub fn set_focus_follows_mouse(&self, focus_follows_mouse: bool) {
        self.focus_follows_mouse.set(focus_follows_mouse);
    }

    pub fn set_window_management_enabled(self: &Rc<Self>, enabled: bool) {
        self.pointer_owner
            .set_window_management_enabled(self, enabled);
    }

    pub fn add_ei_seat(&self, ei: &Rc<EiSeat>) {
        self.ei_seats.set((ei.client.id, ei.id), ei.clone());
        self.update_capabilities();
    }

    pub fn remove_ei_seat(self: &Rc<Self>, ei: &EiSeat) {
        self.ei_seats.remove(&(ei.client.id, ei.id));
        self.destroy_physical_keyboard(ei.keyboard_id);
        self.update_capabilities();
    }

    pub fn seat_kb_state(&self) -> Rc<dyn DynKeyboardState> {
        self.seat_kb_state.get()
    }

    pub fn latest_kb_state(&self) -> Rc<dyn DynKeyboardState> {
        self.latest_kb_state.get()
    }

    pub fn output_extents_changed(&self) {
        self.for_each_ei_seat(|ei_seat| {
            ei_seat.regions_changed();
        });
    }

    pub fn add_tray_item_popup<T: DynTrayItem>(&self, item: &Rc<T>, popup: &Rc<XdgPopup>) {
        self.tray_popups
            .set((item.data().tray_item_id, popup.id), item.clone());
    }

    pub fn remove_tray_item_popup<T: DynTrayItem>(&self, item: &T, popup: &Rc<XdgPopup>) {
        self.tray_popups
            .remove(&(item.data().tray_item_id, popup.id));
    }

    fn handle_node_button(
        self: &Rc<Self>,
        node: Rc<dyn Node>,
        time_usec: u64,
        button: u32,
        state: KeyState,
        serial: u64,
    ) {
        if self.tray_popups.is_not_empty() && state == KeyState::Pressed {
            let id = node.node_tray_item();
            self.tray_popups.lock().retain(|&(tray_item_id, _), item| {
                let retain = Some(tray_item_id) == id;
                if !retain {
                    item.destroy_popups();
                }
                retain
            })
        }
        node.node_on_button(self, time_usec, button, state, serial);
    }

    pub fn handle_focus_request(self: &Rc<Self>, client: &Client, node: Rc<dyn Node>, serial: u64) {
        let Some(max_serial) = client.focus_stealing_serial.get() else {
            return;
        };
        let serial = serial.min(max_serial);
        if serial <= self.keyboard_node_serial.get() {
            return;
        }
        self.focus_node_with_serial(node, serial);
    }

    pub fn get_physical_keyboard(
        &self,
        id: PhysicalKeyboardId,
        map: Option<&Rc<KbvmMap>>,
    ) -> Rc<PhysicalKeyboard> {
        if let Some(d) = self.kb_devices.get(&id) {
            return d;
        }
        let state = match map {
            Some(m) => self.get_kb_state(m),
            _ => self.get_kb_state(&self.seat_kb_map.get()),
        };
        let d = Rc::new(PhysicalKeyboard {
            has_custom_map: Cell::new(map.is_some()),
            phy_state: PhysicalKeyboardState::new(&state),
        });
        self.kb_devices.set(id, d.clone());
        d
    }

    pub fn destroy_physical_keyboard(self: &Rc<Self>, id: PhysicalKeyboardId) {
        let Some(kb) = self.kb_devices.remove(&id) else {
            return;
        };
        kb.phy_state.destroy(self.state.now_usec(), self);
    }
}

impl CursorUserOwner for WlSeatGlobal {
    fn output_changed(&self, output: &Rc<OutputNode>) {
        if let Some(dnd) = self.pointer_owner.dnd_icon() {
            dnd.surface().set_output(output);
        }
        if let Some(drag) = self.pointer_owner.toplevel_drag() {
            if let Some(tl) = drag.toplevel.get() {
                tl.xdg.set_output(output);
            }
        }
    }
}

global_base!(WlSeatGlobal, WlSeat, WlSeatError);

impl Global for WlSeatGlobal {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        9
    }
}

dedicated_add_global!(WlSeatGlobal, seats);

pub struct WlSeat {
    pub global: Rc<WlSeatGlobal>,
    pub id: WlSeatId,
    pub client: Rc<Client>,
    pointers: CopyHashMap<WlPointerId, Rc<WlPointer>>,
    relative_pointers: CopyHashMap<ZwpRelativePointerV1Id, Rc<ZwpRelativePointerV1>>,
    keyboards: CopyHashMap<WlKeyboardId, Rc<WlKeyboard>>,
    touches: CopyHashMap<WlTouchId, Rc<WlTouch>>,
    version: Version,
    tracker: Tracker<Self>,
}

const READ_ONLY_KEYMAP_SINCE: Version = Version(7);

impl WlSeat {
    fn send_capabilities(self: &Rc<Self>) {
        self.client.event(Capabilities {
            self_id: self.id,
            capabilities: self.global.capabilities.get(),
        })
    }

    fn send_name(self: &Rc<Self>, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        })
    }

    pub fn keymap_fd(&self, state: &KeyboardState) -> Result<KeymapFd, WlKeyboardError> {
        let fd = match self.client.is_xwayland {
            true => &state.xwayland_map,
            _ => &state.map,
        };
        if self.version >= READ_ONLY_KEYMAP_SINCE {
            return Ok(fd.clone());
        }
        Ok(fd.create_unprotected_fd()?)
    }
}

impl WlSeatRequestHandler for WlSeat {
    type Error = WlSeatError;

    fn get_pointer(&self, req: GetPointer, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let p = Rc::new(WlPointer::new(req.id, slf));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.pointers.set(req.id, p);
        Ok(())
    }

    fn get_keyboard(&self, req: GetKeyboard, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let p = Rc::new(WlKeyboard::new(req.id, slf));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.keyboards.set(req.id, p.clone());
        if let Some(surface) = self.global.keyboard_node.get().node_into_surface() {
            if surface.client.id == self.client.id {
                p.enter(
                    self.client.next_serial(),
                    surface.id,
                    &self.global.seat_kb_state.get().borrow().kb_state,
                );
            }
        }
        if self.version >= REPEAT_INFO_SINCE {
            let (rate, delay) = self.global.repeat_rate.get();
            p.send_repeat_info(rate, delay);
        }
        Ok(())
    }

    fn get_touch(&self, req: GetTouch, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let p = Rc::new(WlTouch::new(req.id, slf));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.touches.set(req.id, p);
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Entry::Occupied(mut hm) = bindings.entry(self.client.id) {
                hm.get_mut().remove(&self.id);
                if hm.get().is_empty() {
                    hm.remove();
                }
            }
        }
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlSeat;
    version = self.version;
}

impl Object for WlSeat {
    fn break_loops(&self) {
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Entry::Occupied(mut hm) = bindings.entry(self.client.id) {
                hm.get_mut().remove(&self.id);
                if hm.get().is_empty() {
                    hm.remove();
                }
            }
        }
        self.pointers.clear();
        self.relative_pointers.clear();
        self.keyboards.clear();
        self.touches.clear();
    }
}

dedicated_add_obj!(WlSeat, WlSeatId, seats);

#[derive(Debug, Error)]
pub enum WlSeatError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    IpcError(#[from] IpcError),
    #[error(transparent)]
    WlKeyboardError(Box<WlKeyboardError>),
    #[error("Data source has a toplevel attached")]
    OfferHasDrag,
}
efrom!(WlSeatError, ClientError);
efrom!(WlSeatError, WlKeyboardError);

pub fn collect_kb_foci2(node: Rc<dyn Node>, seats: &mut SmallVec<[Rc<WlSeatGlobal>; 3]>) {
    node.node_visit(&mut generic_node_visitor(|node| {
        node.node_seat_state().for_each_kb_focus(|s| seats.push(s));
    }));
}

pub fn collect_kb_foci(node: Rc<dyn Node>) -> SmallVec<[Rc<WlSeatGlobal>; 3]> {
    let mut res = SmallVec::new();
    collect_kb_foci2(node, &mut res);
    res
}

impl DeviceHandlerData {
    pub fn set_seat(&self, seat: Option<Rc<WlSeatGlobal>>) {
        if let Some(new) = &seat {
            if let Some(old) = self.seat.get() {
                if old.id() == new.id() {
                    return;
                }
            }
        } else {
            if self.seat.is_none() {
                return;
            }
        }
        self.destroy_physical_keyboard_state();
        let old = self.seat.set(seat.clone());
        if let Some(old) = old {
            if let Some(info) = &self.tablet_init {
                old.tablet_remove_tablet(info.id);
            }
            if let Some(info) = &self.tablet_pad_init {
                old.tablet_remove_tablet_pad(info.id);
            }
            if self.is_touch {
                old.num_touch_devices.fetch_sub(1);
                old.update_capabilities();
            }
        }
        if let Some(seat) = &seat {
            if let Some(info) = &self.tablet_init {
                seat.tablet_add_tablet(self.device.id(), info);
            }
            if let Some(info) = &self.tablet_pad_init {
                seat.tablet_add_tablet_pad(self.device.id(), info);
            }
            if self.is_touch {
                seat.num_touch_devices.fetch_add(1);
                seat.update_capabilities();
            }
        }
    }

    fn destroy_physical_keyboard_state(&self) {
        if let Some(seat) = self.seat.get() {
            seat.destroy_physical_keyboard(self.keyboard_id);
        };
    }

    pub fn set_keymap(&self, keymap: Option<Rc<KbvmMap>>) {
        self.destroy_physical_keyboard_state();
        self.keymap.set(keymap);
    }

    pub fn set_output(&self, output: Option<&WlOutputGlobal>) {
        match output {
            None => {
                log::info!("Removing output mapping of {}", self.device.name());
                self.output.take();
            }
            Some(o) => {
                log::info!("Mapping {} to {}", self.device.name(), o.connector.name);
                self.output.set(Some(o.opt.clone()));
            }
        }
    }

    pub fn get_rect(&self, state: &State) -> Rect {
        if let Some(output) = self.output.get() {
            if let Some(output) = output.get() {
                return output.pos.get();
            }
        }
        state.root.extents.get()
    }
}
