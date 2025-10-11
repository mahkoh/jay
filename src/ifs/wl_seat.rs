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
pub mod wp_pointer_warp_v1;
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
        backend::{KeyState, Leds},
        client::{Client, ClientError, ClientId},
        cursor_user::{CursorUser, CursorUserGroup, CursorUserOwner},
        ei::ei_ifs::ei_seat::EiSeat,
        fixed::Fixed,
        globals::{Global, GlobalName},
        ifs::{
            ext_idle_notification_v1::ExtIdleNotificationV1,
            ipc::{
                self, DynDataSource, IpcError, IpcLocation,
                data_control::{DataControlDeviceId, DynDataControlDevice},
                offer_source_to_regular_client,
                wl_data_device::{ClipboardIpc, WlDataDevice},
                wl_data_source::WlDataSource,
                x_data_device::{XClipboardIpc, XIpcDevice, XIpcDeviceId, XPrimarySelectionIpc},
                zwp_primary_selection_device_v1::{
                    PrimarySelectionIpc, ZwpPrimarySelectionDeviceV1,
                },
                zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
            },
            wl_output::WlOutputGlobal,
            wl_seat::{
                event_handling::FocusHistoryData,
                gesture_owner::GestureOwnerHolder,
                kb_owner::KbOwnerHolder,
                pointer_owner::PointerOwnerHolder,
                tablet::TabletSeatData,
                text_input::{
                    zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
                    zwp_input_method_v2::ZwpInputMethodV2, zwp_text_input_v3::ZwpTextInputV3,
                },
                touch_owner::TouchOwnerHolder,
                wl_keyboard::{REPEAT_INFO_SINCE, WlKeyboard, WlKeyboardError},
                wl_pointer::WlPointer,
                wl_touch::WlTouch,
                zwp_pointer_constraints_v1::{SeatConstraint, SeatConstraintStatus},
                zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1,
                zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
                zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
                zwp_relative_pointer_v1::ZwpRelativePointerV1,
            },
            wl_surface::{
                WlSurface,
                dnd_icon::DndIcon,
                tray::{DynTrayItem, TrayItemId},
                xdg_surface::xdg_popup::XdgPopup,
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            },
            xdg_toplevel_drag_v1::XdgToplevelDragV1,
        },
        kbvm::{KbvmMap, KbvmMapId, KbvmState, PhysicalKeyboardState},
        keyboard::{DynKeyboardState, KeyboardState, KeyboardStateId, KeymapFd, LedsListener},
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        state::{DeviceHandlerData, State},
        tree::{
            ContainerNode, ContainerSplit, Direction, FoundNode, Node, NodeLayer, NodeLayerLink,
            NodeLocation, OutputNode, StackedNode, ToplevelNode, WorkspaceNode,
            generic_node_visitor, toplevel_create_split, toplevel_parent_container,
            toplevel_set_floating, toplevel_set_workspace,
        },
        utils::{
            asyncevent::AsyncEvent,
            bindings::PerClientBindings,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            event_listener::{EventListener, EventSource},
            linkedlist::{LinkedList, LinkedNode, NodeRef},
            numcell::NumCell,
            on_drop::OnDrop,
            rc_eq::{rc_eq, rc_weak_eq},
            smallmap::SmallMap,
        },
        wire::{
            ExtIdleNotificationV1Id, WlDataDeviceId, WlKeyboardId, WlPointerId, WlSeatId,
            WlTouchId, XdgPopupId, ZwpPrimarySelectionDeviceV1Id, ZwpRelativePointerV1Id,
            ZwpTextInputV3Id, wl_seat::*,
        },
        wire_ei::EiSeatId,
    },
    ahash::AHashMap,
    jay_config::keyboard::syms::{KeySym, SYM_Escape},
    kbvm::Keycode,
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
    revert_key: Cell<KeySym>,
    last_focus_location: Cell<Option<NodeLocation>>,
    focus_history: LinkedList<FocusHistoryData>,
    focus_history_rotate: NumCell<u64>,
    focus_history_visible_only: Cell<bool>,
    focus_history_same_workspace: Cell<bool>,
    mark_mode: Cell<Option<MarkMode>>,
    marks: CopyHashMap<Keycode, Rc<dyn Node>>,
    modifiers_listener: EventListener<dyn LedsListener>,
    modifiers_forward: EventSource<dyn LedsListener>,
}

#[derive(Copy, Clone)]
enum MarkMode {
    Mark,
    Jump,
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
        let slf = Rc::new_cyclic(|slf: &Weak<WlSeatGlobal>| Self {
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
            revert_key: Cell::new(SYM_Escape),
            last_focus_location: Default::default(),
            focus_history: Default::default(),
            focus_history_rotate: Default::default(),
            focus_history_visible_only: Cell::new(false),
            focus_history_same_workspace: Cell::new(false),
            mark_mode: Default::default(),
            marks: Default::default(),
            modifiers_listener: EventListener::new(slf.clone()),
            modifiers_forward: Default::default(),
        });
        slf.pointer_cursor.set_owner(slf.clone());
        slf.modifiers_listener
            .attach(&seat_kb_state.borrow().kb_state.leds_changed);
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

    pub fn get_keyboard_node(&self) -> Rc<dyn Node> {
        self.keyboard_node.get()
    }

    pub fn get_keyboard_output(&self) -> Option<Rc<OutputNode>> {
        self.keyboard_node.get().node_output()
    }

    pub fn set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        toplevel_set_workspace(&self.state, tl, ws);
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
            constraint.deactivate(true);
            if constraint.status.get() == SeatConstraintStatus::Inactive {
                constraint
                    .status
                    .set(SeatConstraintStatus::ActivatableOnFocus);
            }
        }
    }

    fn maybe_constrain_pointer_node(&self) {
        if let Some(pn) = self.pointer_node()
            && let Some(surface) = pn.node_into_surface()
        {
            let (mut x, mut y) = self.pointer_cursor.position();
            let (sx, sy) = surface.buffer_abs_pos.get().position();
            x -= Fixed::from_int(sx);
            y -= Fixed::from_int(sy);
            self.maybe_constrain(&surface, x, y);
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
        candidate.position_hint.take();
        if let Some(owner) = candidate.owner.get() {
            owner.send_enabled();
        }
        self.constraint.set(Some(candidate));
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            tl.tl_set_fullscreen(fullscreen, None);
        }
    }

    pub fn get_fullscreen(&self) -> bool {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            return tl.tl_data().is_fullscreen.get();
        }
        false
    }

    pub fn set_seat_keymap(self: &Rc<Self>, keymap: &Rc<KbvmMap>) {
        self.seat_kb_map.set(keymap.clone());
        let new = self.get_kb_state(keymap);
        let old = self.seat_kb_state.set(new.clone());
        if rc_eq(&old, &new) {
            return;
        }
        let mut to_destroy = vec![];
        for (id, s) in self.kb_devices.lock().iter() {
            if !s.has_custom_map.get() {
                to_destroy.push(*id);
            }
        }
        for dev in to_destroy {
            self.destroy_physical_keyboard(dev);
        }
        {
            let new = &*new.borrow();
            self.modifiers_listener.attach(&new.kb_state.leds_changed);
            self.dispatch_seat_leds_listeners(new.kb_state.leds);
        }
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
        if let Some(weak) = self.kb_states.get(&keymap.id)
            && let Some(state) = weak.upgrade()
        {
            return state;
        }
        self.kb_states
            .lock()
            .retain(|_, state| state.strong_count() > 0);
        let s = keymap.state(self.state.keyboard_state_ids.next());
        let s = Rc::new(RefCell::new(s));
        self.kb_states.set(keymap.id, Rc::downgrade(&s));
        s
    }

    fn attach_modifiers_listener(
        &self,
        id: PhysicalKeyboardId,
        listener: &EventListener<dyn LedsListener>,
        map: Option<&Rc<KbvmMap>>,
    ) {
        let _ = self.get_physical_keyboard(id, map);
        let state = match map {
            None => {
                listener.attach(&self.modifiers_forward);
                self.seat_kb_state.get()
            }
            Some(m) => {
                let state = self.get_kb_state(m);
                listener.attach(&state.borrow().kb_state.leds_changed);
                state
            }
        };
        if let Some(l) = listener.get() {
            l.leds(state.borrow().kb_state.leds);
        }
    }

    fn dispatch_seat_leds_listeners(&self, leds: Leds) {
        for listener in self.modifiers_forward.iter() {
            listener.leds(leds);
        }
    }

    pub fn prepare_for_lock(self: &Rc<Self>) {
        self.pointer_owner.revert_to_default(self);
        self.kb_owner.ungrab(self);
    }

    pub fn kb_parent_container(&self) -> Option<Rc<ContainerNode>> {
        if let Some(tl) = self.keyboard_node.get().node_toplevel() {
            return toplevel_parent_container(&*tl);
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
        if let Some(tl) = self.keyboard_node.get().node_toplevel()
            && let Some(parent) = tl.tl_data().parent.get()
            && let Some(container) = parent.node_into_container()
        {
            let node = if mono { Some(tl.deref()) } else { None };
            container.set_mono(node);
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
        toplevel_create_split(&self.state, tl, axis);
    }

    pub fn focus_parent(self: &Rc<Self>) {
        if let Some(tl) = self.keyboard_node.get().node_toplevel()
            && let Some(parent) = tl.tl_data().parent.get()
            && let Some(tl) = parent.node_toplevel()
        {
            self.focus_node(tl);
        }
    }

    pub fn get_floating(self: &Rc<Self>) -> Option<bool> {
        match self.keyboard_node.get().node_toplevel() {
            Some(tl) => Some(tl.tl_data().parent_is_float.get()),
            _ => None,
        }
    }

    pub fn set_floating(self: &Rc<Self>, floating: bool) {
        let tl = match self.keyboard_node.get().node_toplevel() {
            Some(tl) => tl,
            _ => return,
        };
        toplevel_set_floating(&self.state, tl, floating);
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
        if let Some(tl) = kb_node.node_toplevel()
            && let Some(parent) = tl.tl_data().parent.get()
            && let Some(c) = parent.node_into_container()
        {
            c.move_child(tl, direction);
        }
    }

    pub fn get_last_focus_on_workspace(&self, ws: &WorkspaceNode) -> Option<Rc<dyn Node>> {
        let mut node = self.focus_history.last()?;
        loop {
            if let Some(node) = node.node.upgrade()
                && let Some(NodeLocation::Workspace(_, new)) = node.node_location()
                && new == ws.id
            {
                return Some(node);
            }
            node = node.prev()?;
        }
    }

    fn get_focus_history(
        &self,
        next: impl Fn(&NodeRef<FocusHistoryData>) -> Option<NodeRef<FocusHistoryData>>,
        first: impl FnOnce(&LinkedList<FocusHistoryData>) -> Option<NodeRef<FocusHistoryData>>,
    ) -> Option<(Rc<dyn Node>, bool)> {
        let original = self.keyboard_node.get();
        let mut output = None;
        let mut workspace = None;
        if let Some(old) = original.node_location() {
            match old {
                NodeLocation::Workspace(o, w) => {
                    workspace = Some(w);
                    output = Some(o);
                }
                NodeLocation::Output(o) => {
                    output = Some(o);
                }
            }
        }
        if (output.is_none() || workspace.is_none())
            && let Some(old) = self.last_focus_location.get()
        {
            match old {
                NodeLocation::Workspace(o, w) => {
                    workspace = workspace.or(Some(w));
                    output = output.or(Some(o));
                }
                NodeLocation::Output(o) => {
                    output = output.or(Some(o));
                }
            }
        }
        if workspace.is_none()
            && let Some(output) = original.node_output()
            && let Some(ws) = output.workspace.get()
        {
            workspace = Some(ws.id);
        }
        let matches = |node: &FocusHistoryData| {
            let visible = node.visible.get();
            if self.focus_history_visible_only.get() && !visible {
                return None;
            }
            let node = node.node.upgrade()?;
            if self.focus_history_same_workspace.get() {
                let new = node.node_location()?;
                let o = match new {
                    NodeLocation::Workspace(o, w) => {
                        if workspace != Some(w) {
                            return None;
                        }
                        o
                    }
                    NodeLocation::Output(o) => o,
                };
                if output != Some(o) {
                    return None;
                }
            }
            Some((node, visible))
        };
        let node = original.node_seat_state().get_focus_history(self);
        if let Some(mut node) = node {
            loop {
                node = match next(&node) {
                    Some(n) => n,
                    _ => break,
                };
                if let Some(matches) = matches(&node) {
                    return Some(matches);
                }
            }
        }
        let mut node = first(&self.focus_history)?;
        loop {
            if rc_weak_eq(&original, &node.node) {
                return None;
            }
            if let Some(matches) = matches(&node) {
                return Some(matches);
            }
            node = next(&node)?;
        }
    }

    fn focus_history(
        self: &Rc<Self>,
        next: impl Fn(&NodeRef<FocusHistoryData>) -> Option<NodeRef<FocusHistoryData>>,
        first: impl FnOnce(&LinkedList<FocusHistoryData>) -> Option<NodeRef<FocusHistoryData>>,
    ) {
        let Some((node, visible)) = self.get_focus_history(next, first) else {
            return;
        };
        self.focus_history_rotate.fetch_add(1);
        let _reset = OnDrop(|| {
            self.focus_history_rotate.fetch_sub(1);
        });
        if !visible {
            node.clone().node_make_visible();
            if !node.node_visible() {
                return;
            }
        }
        self.focus_node(node);
    }

    pub fn focus_prev(self: &Rc<Self>) {
        self.focus_history(|s| s.prev(), |l| l.last());
    }

    pub fn focus_next(self: &Rc<Self>) {
        self.focus_history(|s| s.next(), |l| l.first());
    }

    pub fn focus_history_set_visible(&self, visible: bool) {
        self.focus_history_visible_only.set(visible);
    }

    pub fn focus_history_set_same_workspace(&self, same_workspace: bool) {
        self.focus_history_same_workspace.set(same_workspace);
    }

    fn focus_layer_rel<LI, SI>(
        self: &Rc<Self>,
        next_layer: impl Fn(NodeLayer) -> NodeLayer,
        layer_node_next: impl Fn(
            &NodeRef<Rc<ZwlrLayerSurfaceV1>>,
        ) -> Option<NodeRef<Rc<ZwlrLayerSurfaceV1>>>,
        stacked_node_next: impl Fn(
            &NodeRef<Rc<dyn StackedNode>>,
        ) -> Option<NodeRef<Rc<dyn StackedNode>>>,
        layer_list_iter: impl Fn(&LinkedList<Rc<ZwlrLayerSurfaceV1>>) -> LI,
        stacked_list_iter: impl Fn(&LinkedList<Rc<dyn StackedNode>>) -> SI,
    ) where
        LI: Iterator<Item = NodeRef<Rc<ZwlrLayerSurfaceV1>>>,
        SI: Iterator<Item = NodeRef<Rc<dyn StackedNode>>>,
    {
        fn node_viable(n: &(impl Node + ?Sized)) -> bool {
            n.node_visible() && n.node_accepts_focus()
        }

        let current = self.keyboard_node.get();
        let Some(output) = current.node_output() else {
            return;
        };
        let current_layer = current.node_layer();
        match &current_layer {
            NodeLayerLink::Layer0(l)
            | NodeLayerLink::Layer1(l)
            | NodeLayerLink::Layer2(l)
            | NodeLayerLink::Layer3(l) => {
                if let Some(n) = layer_node_next(l)
                    && node_viable(&**n)
                {
                    n.deref()
                        .clone()
                        .node_do_focus(self, Direction::Unspecified);
                    return;
                }
            }
            NodeLayerLink::Stacked(l) | NodeLayerLink::StackedAboveLayers(l) => {
                if let Some(n) = stacked_node_next(l)
                    && node_viable(&**n)
                    && n.node_output().map(|o| o.id) == Some(output.id)
                {
                    n.deref()
                        .clone()
                        .node_do_focus(self, Direction::Unspecified);
                    return;
                }
            }
            NodeLayerLink::Display => {}
            NodeLayerLink::Output => {}
            NodeLayerLink::Workspace => {}
            NodeLayerLink::Tiled => {}
            NodeLayerLink::Fullscreen => {}
            NodeLayerLink::Lock => {}
            NodeLayerLink::InputMethod => {}
        }
        let handle_layer_shell = |l: &LinkedList<Rc<ZwlrLayerSurfaceV1>>| {
            for n in layer_list_iter(l) {
                if node_viable(&**n) {
                    return Some(n.deref().clone() as Rc<dyn Node>);
                }
            }
            None
        };
        let handle_stacked = |l: &LinkedList<Rc<dyn StackedNode>>| {
            for n in stacked_list_iter(l) {
                if node_viable(&**n) && n.node_output().map(|o| o.id) == Some(output.id) {
                    return Some(n.deref().clone() as Rc<dyn Node>);
                }
            }
            None
        };
        let ws = output.workspace.get();
        let first = next_layer(current_layer.layer());
        let mut layer = first;
        loop {
            let node = match layer {
                NodeLayer::Display => None,
                NodeLayer::Layer0 => handle_layer_shell(&output.layers[0]),
                NodeLayer::Layer1 => handle_layer_shell(&output.layers[1]),
                NodeLayer::Output => None,
                NodeLayer::Workspace => None,
                NodeLayer::Tiled => ws
                    .as_ref()
                    .and_then(|w| w.container.get())
                    .map(|n| n as Rc<dyn Node>),
                NodeLayer::Fullscreen => ws
                    .as_ref()
                    .and_then(|w| w.fullscreen.get())
                    .map(|n| n as Rc<dyn Node>),
                NodeLayer::Stacked => handle_stacked(&self.state.root.stacked),
                NodeLayer::Layer2 => handle_layer_shell(&output.layers[2]),
                NodeLayer::Layer3 => handle_layer_shell(&output.layers[3]),
                NodeLayer::StackedAboveLayers => {
                    handle_stacked(&self.state.root.stacked_above_layers)
                }
                NodeLayer::Lock => None,
                NodeLayer::InputMethod => None,
            };
            if let Some(n) = node {
                if node_viable(&*n) {
                    n.node_do_focus(self, Direction::Unspecified);
                    return;
                }
            }
            layer = next_layer(layer);
            if layer == first {
                return;
            }
        }
    }

    pub fn focus_layer_below(self: &Rc<Self>) {
        self.focus_layer_rel(
            |l| l.prev(),
            |n| n.prev(),
            |n| n.prev(),
            |l| l.rev_iter(),
            |l| l.rev_iter(),
        );
    }

    pub fn focus_layer_above(self: &Rc<Self>) {
        self.focus_layer_rel(
            |l| l.next(),
            |n| n.next(),
            |n| n.next(),
            |l| l.iter(),
            |l| l.iter(),
        );
    }

    pub fn focus_tiles(self: &Rc<Self>) {
        let current = self.keyboard_node.get();
        if matches!(
            current.node_layer().layer(),
            NodeLayer::Tiled | NodeLayer::Fullscreen,
        ) {
            return;
        }
        let Some(output) = current.node_output() else {
            return;
        };
        let Some(ws) = output.workspace.get() else {
            return;
        };
        let node = match ws.fullscreen.get() {
            Some(fs) => fs as Rc<dyn Node>,
            _ => match ws.container.get() {
                Some(c) => c,
                _ => return,
            },
        };
        if node.node_visible() && node.node_accepts_focus() {
            node.node_do_focus(self, Direction::Unspecified);
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
        if let (Some(new), Some(old)) = (&src, &field.get())
            && new.source_data().id == old.source_data().id
        {
            return Ok(());
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
            let output = self.pointer_cursor.output();
            icon.surface()
                .set_output(&output, NodeLocation::Output(output.id));
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
        if let Some(selection) = &selection
            && selection.toplevel_drag.is_some()
        {
            return Err(WlSeatError::OfferHasDrag);
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
        if let Some(serial) = serial
            && serial < self.primary_selection_serial.get()
        {
            return false;
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
        self.marks.clear();
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
        if let Some(tl_drag) = self.toplevel_drag()
            && let Some(tl) = tl_drag.toplevel.get()
        {
            tl.tl_set_visible(visible);
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

    pub fn pinned(&self) -> bool {
        let Some(tl) = self.keyboard_node.get().node_toplevel() else {
            return false;
        };
        tl.tl_pinned()
    }

    pub fn set_pinned(&self, pinned: bool) {
        let Some(tl) = self.keyboard_node.get().node_toplevel() else {
            return;
        };
        tl.tl_set_pinned(true, pinned);
    }

    pub fn set_pointer_revert_key(&self, key: KeySym) {
        self.revert_key.set(key);
    }
}

impl CursorUserOwner for WlSeatGlobal {
    fn output_changed(&self, output: &Rc<OutputNode>) {
        if let Some(dnd) = self.pointer_owner.dnd_icon() {
            dnd.surface()
                .set_output(output, NodeLocation::Output(output.id));
        }
        if let Some(drag) = self.pointer_owner.toplevel_drag()
            && let Some(tl) = drag.toplevel.get()
        {
            tl.xdg.set_output(output);
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
            true => &state.map.xwayland_map,
            _ => &state.map.map,
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
        self.pointers.set(req.id, p.clone());
        let surface = self
            .global
            .pointer_node()
            .and_then(|n| n.node_into_surface());
        if let Some(surface) = surface
            && surface.client.id == self.client.id
        {
            let (x, y) = self.global.pointer_cursor.position();
            let (x_int, y_int) = surface
                .buffer_abs_pos
                .get()
                .translate(x.round_down(), y.round_down());
            p.send_enter(
                self.client.next_serial(),
                surface.id,
                x.apply_fract(x_int),
                y.apply_fract(y_int),
            );
        }
        Ok(())
    }

    fn get_keyboard(&self, req: GetKeyboard, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let p = Rc::new(WlKeyboard::new(req.id, slf));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.keyboards.set(req.id, p.clone());
        if let Some(surface) = self.global.keyboard_node.get().node_into_surface()
            && surface.client.id == self.client.id
        {
            p.enter(
                self.client.next_serial(),
                surface.id,
                &self.global.seat_kb_state.get().borrow().kb_state,
            );
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
    fn break_loops(self: Rc<Self>) {
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
            if let Some(old) = self.seat.get()
                && old.id() == new.id()
            {
                return;
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
        self.attach_event_listeners();
    }

    fn destroy_physical_keyboard_state(&self) {
        self.mods_listener.detach();
        if let Some(seat) = self.seat.get() {
            seat.destroy_physical_keyboard(self.keyboard_id);
        };
    }

    fn attach_event_listeners(&self) {
        if self.is_kb
            && let Some(seat) = self.seat.get()
        {
            seat.attach_modifiers_listener(
                self.keyboard_id,
                &self.mods_listener,
                self.keymap.get().as_ref(),
            );
        };
    }

    pub fn set_keymap(&self, keymap: Option<Rc<KbvmMap>>) {
        self.destroy_physical_keyboard_state();
        self.keymap.set(keymap);
        self.attach_event_listeners();
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
        if let Some(output) = self.output.get()
            && let Some(output) = output.get()
        {
            return output.pos.get();
        }
        state.root.extents.get()
    }
}

impl LedsListener for DeviceHandlerData {
    fn leds(&self, leds: Leds) {
        self.device.set_enabled_leds(leds);
    }
}

impl LedsListener for WlSeatGlobal {
    fn leds(&self, leds: Leds) {
        self.dispatch_seat_leds_listeners(leds)
    }
}

pub struct PositionHintRequest {
    seat: Rc<WlSeatGlobal>,
    client_id: ClientId,
    old_pos: (Fixed, Fixed),
    new_pos: (Fixed, Fixed),
}

pub async fn handle_position_hint_requests(state: Rc<State>) {
    loop {
        let req = state.position_hint_requests.pop().await;
        let (x, y) = (req.new_pos.0.round_down(), req.new_pos.1.round_down());
        if state.node_at(x, y).node.node_client_id() != Some(req.client_id) {
            continue;
        }
        let current_pos = req.seat.pointer_cursor.position();
        let (x, y) = (
            req.new_pos.0 + (current_pos.0 - req.old_pos.0),
            req.new_pos.1 + (current_pos.1 - req.old_pos.1),
        );
        req.seat.motion_event_abs(state.now_usec(), x, y, false);
    }
}
