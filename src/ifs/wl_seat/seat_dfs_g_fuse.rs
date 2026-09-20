/*
dir seat {
    @inherit dfs_global,
    id: reg,
    name: reg,
    capabilities: reg,
    num_touch_devices: reg,
    changes: reg,
    forward: reg,
    focus_follows_mouse: reg,
    mouse_follows_focus: reg,
    warp_mouse_to_focus_scheduled: reg,
    simple_im_enabled: reg,
    focus_history_visible_only: reg,
    focus_history_same_workspace: reg,
    has_constraint: reg,
    num_ei_seats: reg,
    num_idle_notifications: reg,
    num_tray_popups: reg,
    num_marks: reg,
    keyboard_node_serial: reg,
    latest_kb_state_id: reg,
    repeat_rate: reg,
    repeat_delay: reg,
    repeat_key_shortcuts_only: reg,
    repeat_key_start_ns: reg,
    has_repeat_key: reg,
    num_kb_devices: reg,
    num_kb_states: reg,
    has_input_method: reg,
    has_input_method_grab: reg,
    has_text_input: reg,
    pos_time_usec: reg,
    last_input_usec: reg,
    pointer_stack_modified: reg,
    pointer_stack_len: reg,
    has_dropped_dnd: reg,
    has_ui_drag_highlight: reg,
    has_selection: reg,
    selection_serial: reg,
    has_primary_selection: reg,
    primary_selection_serial: reg,
    num_data_control_devices: reg,
    num_x_data_devices: reg,
}
 */
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_seat::seat_dfs_g_fuse::generated::seat;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
pub use seat::View as SeatView;

impl seat::Dir for WlSeatGlobal {
    fn read_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.id.raw().str_fmt(buf, ctx);
    }

    fn read_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.seat_name.as_str().str_fmt(buf, ctx);
    }

    fn read_capabilities(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.capabilities.get().str_fmt(buf, ctx);
    }

    fn read_num_touch_devices(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.num_touch_devices.get().str_fmt(buf, ctx);
    }

    fn read_changes(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.changes.get().str_fmt(buf, ctx);
    }

    fn read_forward(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.forward.get().str_fmt(buf, ctx);
    }

    fn read_focus_follows_mouse(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.focus_follows_mouse.get().str_fmt(buf, ctx);
    }

    fn read_mouse_follows_focus(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.mouse_follows_focus.get().str_fmt(buf, ctx);
    }

    fn read_warp_mouse_to_focus_scheduled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.warp_mouse_to_focus_scheduled.get().str_fmt(buf, ctx);
    }

    fn read_simple_im_enabled(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.simple_im_enabled.get().str_fmt(buf, ctx);
    }

    fn read_focus_history_visible_only(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.focus_history_visible_only.get().str_fmt(buf, ctx);
    }

    fn read_focus_history_same_workspace(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.focus_history_same_workspace.get().str_fmt(buf, ctx);
    }

    fn read_has_constraint(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.constraint.get().is_some().str_fmt(buf, ctx);
    }

    fn read_num_ei_seats(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ei_seats.len().str_fmt(buf, ctx);
    }

    fn read_num_idle_notifications(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.idle_notifications.len().str_fmt(buf, ctx);
    }

    fn read_num_tray_popups(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.tray_popups.len().str_fmt(buf, ctx);
    }

    fn read_num_marks(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.marks.len().str_fmt(buf, ctx);
    }

    fn read_keyboard_node_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.keyboard_node_serial.get().str_fmt(buf, ctx);
    }

    fn read_latest_kb_state_id(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.latest_kb_state_id.get().raw().str_fmt(buf, ctx);
    }

    fn read_repeat_rate(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.repeat_rate.get().0.str_fmt(buf, ctx);
    }

    fn read_repeat_delay(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.repeat_rate.get().1.str_fmt(buf, ctx);
    }

    fn read_repeat_key_shortcuts_only(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.repeat_key_shortcuts_only.get().str_fmt(buf, ctx);
    }

    fn read_repeat_key_start_ns(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.repeat_key_start_ns.get().str_fmt(buf, ctx);
    }

    fn read_has_repeat_key(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.repeat_key.get().is_some().str_fmt(buf, ctx);
    }

    fn read_num_kb_devices(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb_devices.len().str_fmt(buf, ctx);
    }

    fn read_num_kb_states(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.kb_states.len().str_fmt(buf, ctx);
    }

    fn read_has_input_method(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.input_method.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_input_method_grab(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.input_method_grab.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_text_input(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.text_input.get().is_some().str_fmt(buf, ctx);
    }

    fn read_pos_time_usec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pos_time_usec.get().str_fmt(buf, ctx);
    }

    fn read_last_input_usec(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.last_input_usec.get().str_fmt(buf, ctx);
    }

    fn read_pointer_stack_modified(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pointer_stack_modified.get().str_fmt(buf, ctx);
    }

    fn read_pointer_stack_len(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.pointer_stack.borrow().len().str_fmt(buf, ctx);
    }

    fn read_has_dropped_dnd(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.dropped_dnd.borrow().is_some().str_fmt(buf, ctx);
    }

    fn read_has_ui_drag_highlight(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.ui_drag_highlight.get().is_some().str_fmt(buf, ctx);
    }

    fn read_has_selection(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.selection.get().is_some().str_fmt(buf, ctx);
    }

    fn read_selection_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.selection_serial.get().str_fmt(buf, ctx);
    }

    fn read_has_primary_selection(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.primary_selection.get().is_some().str_fmt(buf, ctx);
    }

    fn read_primary_selection_serial(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.primary_selection_serial.get().str_fmt(buf, ctx);
    }

    fn read_num_data_control_devices(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.data_control_devices.len().str_fmt(buf, ctx);
    }

    fn read_num_x_data_devices(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.x_data_devices.len().str_fmt(buf, ctx);
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_d21e0724a24e54ec878257980159d1f9a18a4e1c9cb18f4a253c7a5c7607a146.rs",
));
// FUSE GENERATED STOP
