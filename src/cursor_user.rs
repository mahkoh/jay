use {
    crate::{
        cursor::{Cursor, KnownCursor, DEFAULT_CURSOR_SIZE},
        fixed::Fixed,
        rect::Rect,
        state::State,
        tree::OutputNode,
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt, rc_eq::rc_eq, transform_ext::TransformExt,
        },
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
};

linear_ids!(CursorUserGroupIds, CursorUserGroupId, u64);
linear_ids!(CursorUserIds, CursorUserId, u64);

pub trait CursorUserOwner {
    fn output_changed(&self, output: &Rc<OutputNode>);
}

pub struct CursorUserGroup {
    pub id: CursorUserGroupId,
    state: Rc<State>,
    active_id: Cell<Option<CursorUserId>>,
    active: CloneCell<Option<Rc<CursorUser>>>,
    users: CopyHashMap<CursorUserId, Rc<CursorUser>>,
    hardware_cursor: Cell<bool>,
    size: Cell<u32>,
    latest_output: CloneCell<Rc<OutputNode>>,
}

pub struct CursorUser {
    pub id: CursorUserId,
    group: Rc<CursorUserGroup>,
    desired_known_cursor: Cell<Option<KnownCursor>>,
    cursor: CloneCell<Option<Rc<dyn Cursor>>>,
    output: CloneCell<Rc<OutputNode>>,
    output_pos: Cell<Rect>,
    pos: Cell<(Fixed, Fixed)>,
    owner: CloneCell<Option<Rc<dyn CursorUserOwner>>>,
}

impl CursorUserGroup {
    pub fn create(state: &Rc<State>) -> Rc<Self> {
        let output = state
            .root
            .outputs
            .lock()
            .values()
            .next()
            .cloned()
            .or_else(|| state.dummy_output.get())
            .unwrap();
        let hardware_cursor = state.cursor_user_group_hardware_cursor.is_none();
        let group = Rc::new(Self {
            id: state.cursor_user_group_ids.next(),
            state: state.clone(),
            active_id: Default::default(),
            active: Default::default(),
            users: Default::default(),
            hardware_cursor: Cell::new(hardware_cursor),
            size: Cell::new(*DEFAULT_CURSOR_SIZE),
            latest_output: CloneCell::new(output),
        });
        state.add_cursor_size(*DEFAULT_CURSOR_SIZE);
        state.cursor_user_groups.set(group.id, group.clone());
        if hardware_cursor {
            state
                .cursor_user_group_hardware_cursor
                .set(Some(group.clone()));
        }
        group
    }

    pub fn deactivate(&self) {
        if self.hardware_cursor.get() {
            self.remove_hardware_cursor();
        }
        self.active_id.take();
        self.active.take();
        self.state.damage();
    }

    pub fn latest_output(&self) -> Rc<OutputNode> {
        self.latest_output.get()
    }

    fn remove_hardware_cursor(&self) {
        self.state.hardware_tick_cursor.push(None);
        self.state.disable_hardware_cursors();
        self.state.cursor_user_group_hardware_cursor.take();
    }

    pub fn detach(&self) {
        self.deactivate();
        self.latest_output
            .set(self.state.dummy_output.get().unwrap());
        self.state.remove_cursor_size(self.size.get());
        self.state.cursor_user_groups.remove(&self.id);
        for user in self.users.lock().drain_values() {
            user.detach();
        }
    }

    pub fn create_user(self: &Rc<Self>) -> Rc<CursorUser> {
        let output = self.latest_output.get();
        let user = Rc::new(CursorUser {
            id: self.state.cursor_user_ids.next(),
            group: self.clone(),
            desired_known_cursor: Cell::new(None),
            cursor: Default::default(),
            pos: Cell::new(self.output_center(&output)),
            output_pos: Cell::new(output.global.pos.get()),
            output: CloneCell::new(output),
            owner: Default::default(),
        });
        self.users.set(user.id, user.clone());
        user
    }

    pub fn set_visible(&self, visible: bool) {
        if let Some(user) = self.active.get() {
            if let Some(cursor) = user.cursor.get() {
                cursor.set_visible(visible);
            }
        }
    }

    pub fn active(&self) -> Option<Rc<CursorUser>> {
        self.active.get()
    }

    pub fn render_ctx_changed(&self) {
        for user in self.users.lock().values() {
            if let Some(cursor) = user.desired_known_cursor.get() {
                user.set_known(cursor);
            }
        }
    }

    pub fn reload_known_cursor(&self) {
        for user in self.users.lock().values() {
            user.reload_known_cursor();
        }
    }

    pub fn set_hardware_cursor(self: &Rc<Self>, hardware_cursor: bool) {
        if self.hardware_cursor.replace(hardware_cursor) == hardware_cursor {
            return;
        }
        if hardware_cursor {
            let prev = self
                .state
                .cursor_user_group_hardware_cursor
                .set(Some(self.clone()));
            if let Some(prev) = prev {
                prev.hardware_cursor.set(false);
            }
            match self.active.get() {
                None => self.remove_hardware_cursor(),
                Some(a) => a.update_hardware_cursor(),
            }
        } else {
            self.remove_hardware_cursor();
        }
    }

    pub fn hardware_cursor(&self) -> bool {
        self.hardware_cursor.get()
    }

    pub fn set_cursor_size(&self, size: u32) {
        let old = self.size.replace(size);
        if size != old {
            self.state.remove_cursor_size(old);
            self.state.add_cursor_size(size);
            self.reload_known_cursor();
        }
    }

    fn output_center(&self, output: &Rc<OutputNode>) -> (Fixed, Fixed) {
        let pos = output.global.pos.get();
        let x = Fixed::from_int((pos.x1() + pos.x2()) / 2);
        let y = Fixed::from_int((pos.y1() + pos.y2()) / 2);
        (x, y)
    }

    pub fn first_output_connected(&self, output: &Rc<OutputNode>) {
        self.latest_output.set(output.clone());
        let (x, y) = self.output_center(output);
        for user in self.users.lock().values() {
            user.set_output(output);
            user.set_position(x, y);
        }
    }

    pub fn output_disconnected(&self, output: &Rc<OutputNode>, next: &Rc<OutputNode>) {
        if self.latest_output.get().id == output.id {
            self.latest_output.set(next.clone());
        }
        let (x, y) = self.output_center(next);
        for user in self.users.lock().values() {
            if user.output.get().id == output.id {
                user.set_output(next);
                user.set_position(x, y);
            }
        }
    }

    pub fn output_pos_changed(&self, output: &Rc<OutputNode>) {
        let (x, y) = self.output_center(output);
        for user in self.users.lock().values() {
            if user.output.get().id == output.id {
                user.output_pos.set(output.global.pos.get());
                user.set_position(x, y);
            }
        }
    }
}

impl CursorUser {
    pub fn set_owner(&self, owner: Rc<dyn CursorUserOwner>) {
        self.owner.set(Some(owner));
    }

    pub fn detach(&self) {
        self.set(None);
        self.owner.take();
        self.group.users.remove(&self.id);
        if self.group.active_id.get() == Some(self.id) {
            self.group.active_id.take();
            self.group.active.take();
            self.group.state.damage();
        }
    }

    pub fn activate(self: &Rc<Self>) {
        if self.group.active_id.replace(Some(self.id)) == Some(self.id) {
            return;
        }
        self.group.latest_output.set(self.output.get());
        self.group.active.set(Some(self.clone()));
        self.update_hardware_cursor();
        self.group.state.damage();
    }

    #[cfg_attr(not(feature = "it"), allow(dead_code))]
    pub fn desired_known_cursor(&self) -> Option<KnownCursor> {
        self.desired_known_cursor.get()
    }

    pub fn set_known(&self, cursor: KnownCursor) {
        self.desired_known_cursor.set(Some(cursor));
        let cursors = match self.group.state.cursors.get() {
            Some(c) => c,
            None => {
                self.set_cursor2(None);
                return;
            }
        };
        let tpl = match cursor {
            KnownCursor::Default => &cursors.default,
            KnownCursor::ContextMenu => &cursors.context_menu,
            KnownCursor::Help => &cursors.help,
            KnownCursor::Pointer => &cursors.pointer,
            KnownCursor::Progress => &cursors.progress,
            KnownCursor::Wait => &cursors.wait,
            KnownCursor::Cell => &cursors.cell,
            KnownCursor::Crosshair => &cursors.crosshair,
            KnownCursor::Text => &cursors.text,
            KnownCursor::VerticalText => &cursors.vertical_text,
            KnownCursor::Alias => &cursors.alias,
            KnownCursor::Copy => &cursors.copy,
            KnownCursor::Move => &cursors.r#move,
            KnownCursor::NoDrop => &cursors.no_drop,
            KnownCursor::NotAllowed => &cursors.not_allowed,
            KnownCursor::Grab => &cursors.grab,
            KnownCursor::Grabbing => &cursors.grabbing,
            KnownCursor::EResize => &cursors.e_resize,
            KnownCursor::NResize => &cursors.n_resize,
            KnownCursor::NeResize => &cursors.ne_resize,
            KnownCursor::NwResize => &cursors.nw_resize,
            KnownCursor::SResize => &cursors.s_resize,
            KnownCursor::SeResize => &cursors.se_resize,
            KnownCursor::SwResize => &cursors.sw_resize,
            KnownCursor::WResize => &cursors.w_resize,
            KnownCursor::EwResize => &cursors.ew_resize,
            KnownCursor::NsResize => &cursors.ns_resize,
            KnownCursor::NeswResize => &cursors.nesw_resize,
            KnownCursor::NwseResize => &cursors.nwse_resize,
            KnownCursor::ColResize => &cursors.col_resize,
            KnownCursor::RowResize => &cursors.row_resize,
            KnownCursor::AllScroll => &cursors.all_scroll,
            KnownCursor::ZoomIn => &cursors.zoom_in,
            KnownCursor::ZoomOut => &cursors.zoom_out,
        };
        self.set_cursor2(Some(tpl.instantiate(self.group.size.get())));
    }

    fn set_output(&self, output: &Rc<OutputNode>) {
        self.output.set(output.clone());
        self.output_pos.set(output.global.pos.get());
        if self.is_active() {
            self.group.latest_output.set(output.clone());
        }
        if let Some(cursor) = self.cursor.get() {
            cursor.set_output(output);
        }
        if let Some(owner) = self.owner.get() {
            owner.output_changed(output);
        }
    }

    pub fn output(&self) -> Rc<OutputNode> {
        self.output.get()
    }

    pub fn get(&self) -> Option<Rc<dyn Cursor>> {
        self.cursor.get()
    }

    pub fn set(&self, cursor: Option<Rc<dyn Cursor>>) {
        self.set_cursor2(cursor);
        self.desired_known_cursor.set(None);
    }

    fn is_active(&self) -> bool {
        self.group.active_id.get() == Some(self.id)
    }

    fn set_cursor2(&self, cursor: Option<Rc<dyn Cursor>>) {
        if let Some(old) = self.cursor.get() {
            if let Some(new) = cursor.as_ref() {
                if rc_eq(&old, new) {
                    self.update_hardware_cursor();
                    return;
                }
            }
            old.handle_unset();
        }
        if let Some(cursor) = cursor.as_ref() {
            cursor.clone().handle_set();
            cursor.set_output(&self.output.get());
        }
        self.cursor.set(cursor.clone());
        self.update_hardware_cursor();
    }

    pub fn position(&self) -> (Fixed, Fixed) {
        self.pos.get()
    }

    pub fn set_position(&self, mut x: Fixed, mut y: Fixed) -> (Fixed, Fixed) {
        let x_int = x.round_down();
        let y_int = y.round_down();
        if !self.output_pos.get().contains(x_int, y_int) {
            let (output, x_tmp, y_tmp) = self.group.state.find_closest_output(x_int, y_int);
            self.set_output(&output);
            x = x.apply_fract(x_tmp);
            y = y.apply_fract(y_tmp);
        }
        self.pos.set((x, y));
        self.update_hardware_cursor_(false);
        (x, y)
    }

    pub fn update_hardware_cursor(&self) {
        self.update_hardware_cursor_(true);
    }

    fn hardware_cursor(&self) -> bool {
        self.is_active() && self.group.hardware_cursor.get()
    }

    fn update_hardware_cursor_(&self, render: bool) {
        if !self.hardware_cursor() {
            return;
        }
        let cursor = self.cursor.get();
        self.group.state.hardware_tick_cursor.push(cursor.clone());
        let cursor = match cursor {
            Some(c) => c,
            _ => {
                self.group.state.disable_hardware_cursors();
                return;
            }
        };
        if render {
            cursor.tick();
        }
        let (x, y) = self.pos.get();
        for output in self.group.state.root.outputs.lock().values() {
            if let Some(hc) = output.hardware_cursor.get() {
                let transform = output.global.persistent.transform.get();
                let render = render | output.hardware_cursor_needs_render.take();
                let scale = output.global.persistent.scale.get();
                let extents = cursor.extents_at_scale(scale);
                let (hc_width, hc_height) = hc.size();
                if render {
                    let (max_width, max_height) = transform.maybe_swap((hc_width, hc_height));
                    if extents.width() > max_width || extents.height() > max_height {
                        hc.set_enabled(false);
                        hc.commit();
                        continue;
                    }
                }
                let opos = output.global.pos.get();
                let (x_rel, y_rel);
                if scale == 1 {
                    x_rel = x.round_down() - opos.x1();
                    y_rel = y.round_down() - opos.y1();
                } else {
                    let scalef = scale.to_f64();
                    x_rel = ((x - Fixed::from_int(opos.x1())).to_f64() * scalef).round() as i32;
                    y_rel = ((y - Fixed::from_int(opos.y1())).to_f64() * scalef).round() as i32;
                }
                let (width, height) = output.global.pixel_size();
                if extents.intersects(&Rect::new_sized(-x_rel, -y_rel, width, height).unwrap()) {
                    if render {
                        let buffer = hc.get_buffer();
                        let res = buffer.render_hardware_cursor(
                            cursor.deref(),
                            &self.group.state,
                            scale,
                            transform,
                        );
                        match res {
                            Ok(sync_file) => {
                                hc.set_sync_file(sync_file);
                                hc.swap_buffer();
                            }
                            Err(e) => {
                                log::error!("Could not render hardware cursor: {}", ErrorFmt(e));
                            }
                        }
                    }
                    hc.set_enabled(true);
                    let mode = output.global.mode.get();
                    let (x_rel, y_rel) =
                        transform.apply_point(mode.width, mode.height, (x_rel, y_rel));
                    let (hot_x, hot_y) =
                        transform.apply_point(hc_width, hc_height, (-extents.x1(), -extents.y1()));
                    hc.set_position(x_rel - hot_x, y_rel - hot_y);
                } else {
                    if render {
                        output.hardware_cursor_needs_render.set(true);
                    }
                    hc.set_enabled(false);
                }
                hc.commit();
            }
        }
    }

    fn reload_known_cursor(&self) {
        if let Some(kc) = self.desired_known_cursor.get() {
            self.set_known(kc);
        }
    }
}
