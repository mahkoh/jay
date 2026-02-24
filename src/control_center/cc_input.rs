use {
    crate::{
        backend::{InputDeviceCapability, InputDeviceId},
        control_center::{
            ControlCenterInner, GridExt, PaneState, bool, bool_ui, combo_box, combo_box_ui,
            drag_value, drag_value_ui, grid, grid_label, grid_label_ui, label, text_edit, tip,
        },
        ifs::{
            wl_output::WlOutputGlobal,
            wl_seat::{SeatId, WlSeatGlobal},
        },
        kbvm::KbvmMap,
        state::{DeviceHandlerData, State},
        utils::errorfmt::ErrorFmt,
    },
    ahash::AHashMap,
    egui::{
        CollapsingHeader, ComboBox, DragValue, Event, Grid, Id, TextBuffer, TextFormat, Ui,
        UiBuilder, ViewportCommand, Widget, emath::Numeric, text::LayoutJob,
    },
    egui_material_icons::icons::ICON_PENDING,
    isnt::std_1::string::IsntStringExt,
    jay_config::keyboard::syms::KeySym,
    kbvm::Keysym,
    linearize::LinearizeExt,
    rand::random,
    std::{mem, rc::Rc},
};

pub struct InputPane {
    state: Rc<State>,
    paste_requested: Option<Id>,
    keymaps: AHashMap<Key, KeymapState>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Key {
    Seat(SeatId),
    Dev(InputDeviceId),
}

struct KeymapState {
    seed: u64,
    rules_default: bool,
    rules: String,
    model_default: bool,
    model: String,
    layouts: String,
    variants: String,
    options: String,
    backup: Option<Rc<KbvmMap>>,
    pointer_revert_key: Keysym,
    pointer_revert_key_str: Option<String>,
    unknown_pointer_revert_key: bool,
}

impl Default for KeymapState {
    fn default() -> Self {
        Self {
            seed: random(),
            rules_default: true,
            rules: Default::default(),
            model_default: true,
            model: Default::default(),
            layouts: Default::default(),
            variants: Default::default(),
            options: Default::default(),
            backup: Default::default(),
            pointer_revert_key: Default::default(),
            pointer_revert_key_str: None,
            unknown_pointer_revert_key: false,
        }
    }
}

impl ControlCenterInner {
    pub fn create_input_pane(self: &Rc<Self>) -> InputPane {
        InputPane {
            state: self.state.clone(),
            paste_requested: Default::default(),
            keymaps: Default::default(),
        }
    }
}

impl InputPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Input");
    }

    pub fn show(&mut self, ps: &mut PaneState, ui: &mut Ui) {
        let state = self.state.clone();
        let seats = state.globals.seats.lock();
        let mut seats: Vec<_> = seats.values().collect();
        seats.sort_by_key(|d| d.seat_name());
        for seat in &seats {
            self.show_seat(ps, ui, seat);
        }
        let outputs = state.globals.outputs.lock();
        let mut outputs: Vec<_> = outputs.values().collect();
        outputs.sort_by_key(|o| &o.connector.name);
        let dev = &*state.input_device_handlers.borrow();
        let mut dev: Vec<_> = dev.values().collect();
        dev.sort_by_key(|d| d.data.device.name());
        for dev in dev {
            self.show_device(ps, ui, &dev.data, &seats, &outputs);
        }
    }

    fn show_seat(&mut self, ps: &mut PaneState, ui: &mut Ui, seat: &Rc<WlSeatGlobal>) {
        let mut layout_job = LayoutJob::default();
        layout_job.append(
            "Seat",
            0.0,
            TextFormat {
                color: ui.style().visuals.widgets.inactive.text_color(),
                ..Default::default()
            },
        );
        layout_job.append(
            seat.seat_name(),
            10.0,
            TextFormat {
                color: ui.style().visuals.widgets.active.text_color(),
                ..Default::default()
            },
        );
        let ks = self.keymaps.entry(Key::Seat(seat.id())).or_default();
        CollapsingHeader::new(layout_job)
            .id_salt(("seat", seat.id()))
            .show(ui, |ui| {
                grid(ui, ("seat", seat.id()), |ui| {
                    let mut dv = |name: &str, get: &dyn Fn(&mut (i32, i32)) -> &mut i32| {
                        let ui = &mut *ui.row();
                        grid_label(ui, name);
                        let mut v = seat.get_rate();
                        let old = v;
                        ui.horizontal(|ui| {
                            let v = get(&mut v);
                            DragValue::new(v).range(0..=i32::MAX).ui(ui);
                            if ui.button("-20").clicked() {
                                *v = v.saturating_sub(20).max(0)
                            }
                            if ui.button("+20").clicked() {
                                *v = v.saturating_add(20);
                            }
                        });
                        if v != old {
                            seat.set_rate(v.0, v.1);
                        }
                    };
                    dv("Repeat Rate", &|v| &mut v.0);
                    dv("Repeat Delay", &|v| &mut v.1);
                    drag_value(
                        ui,
                        "Cursor Size",
                        seat.cursor_group().cursor_size(),
                        0..=u32::MAX,
                        1.0,
                        |v| seat.cursor_group().set_cursor_size(v),
                    );
                    bool_ui(
                        ui,
                        "Simple IM",
                        |ui| {
                            tip(ui, |ui| {
                                ui.label("A simple input method based on Xcompose files.");
                                ui.label(concat!(
                                    "If you're not using another input method, you should ",
                                    "leave this enabled as it will work for sandboxed ",
                                    "applications, which regular Xcompose will not.",
                                ));
                                ui.label(concat!(
                                    "The `enable-unicode-input` action can be used to input ",
                                    "characters by their unicode value.",
                                ));
                            });
                        },
                        seat.simple_im_enabled(),
                        |b| seat.set_simple_im_enabled(b),
                    );
                    bool_ui(
                        ui,
                        "Hardware Cursor",
                        |ui| {
                            tip(ui, |ui| {
                                ui.label(
                                    "Allow this seat to use the hardware cursor, if available.",
                                );
                                ui.label("Only one seat can use the hardware cursor at a time.");
                            });
                        },
                        seat.cursor_group().hardware_cursor(),
                        |b| seat.cursor_group().set_hardware_cursor(b),
                    );
                    {
                        let ui = &mut *ui.row();
                        let v = seat.pointer_revert_key();
                        let v = Keysym(v.0);
                        if mem::replace(&mut ks.pointer_revert_key, v) != v {
                            ks.pointer_revert_key_str = None;
                        }
                        let name = ks
                            .pointer_revert_key_str
                            .get_or_insert_with(|| v.name().unwrap_or_default().to_string());
                        grid_label_ui(ui, |ui| {
                            ui.label("Pointer Revert Key");
                            tip(ui, |ui| {
                                ui.label(concat!(
                                    "Pressing this key reverts the pointer to the default state, ",
                                    "breaking grabs, drags, etc.",
                                ));
                                ui.label(
                                    "Setting this to `NoSymbol` effectively disables this feature.",
                                );
                            });
                        });
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(name).changed() {
                                let v = Keysym::from_str(name);
                                ks.unknown_pointer_revert_key = v.is_none();
                                if let Some(v) = v {
                                    ks.pointer_revert_key = v;
                                    seat.set_pointer_revert_key(KeySym(v.0));
                                }
                            }
                            if ks.unknown_pointer_revert_key {
                                ui.label("Error: Unknown key");
                            }
                        });
                    }
                    bool(ui, "Focus Follows Mouse", seat.focus_follows_mouse(), |v| {
                        seat.set_focus_follows_mouse(v);
                    });
                    combo_box_ui(
                        ui,
                        "Fallback Output Mode",
                        |ui| {
                            tip(ui, |ui| {
                                ui.label(concat!(
                                    "This determines the output to use in operations where no ",
                                    "output is explicitly specified.",
                                ));
                                ui.label(concat!(
                                    "For example, when a new window is opened, this determines ",
                                    "where the window will be opened.",
                                ));
                                ui.label("`Cursor` refers to the output that contains the cursor.");
                                ui.label(
                                    "`Focus` refers to the output that has the keyboard focus.",
                                );
                            });
                        },
                        seat.fallback_output_mode(),
                        |v| seat.set_fallback_output_mode(v),
                    );
                });
                ui.label("Focus History");
                ui.indent("focus-history", |ui| {
                    let mut v = seat.focus_history_visible();
                    if ui.checkbox(&mut v, "Only Visible").changed() {
                        seat.focus_history_set_visible(v);
                    }
                    let mut v = seat.focus_history_same_workspace();
                    if ui.checkbox(&mut v, "Same Workspace").changed() {
                        seat.focus_history_set_same_workspace(v);
                    }
                });
                if ui.button("Reload Simple IM").clicked() {
                    seat.reload_simple_im();
                }
                show_keymap(
                    &self.state,
                    ps,
                    &mut self.paste_requested,
                    ks,
                    ui,
                    Some(&seat.keymap()),
                    |m| seat.set_seat_keymap(m),
                );
            });
    }

    fn show_device(
        &mut self,
        ps: &mut PaneState,
        ui: &mut Ui,
        dev: &Rc<DeviceHandlerData>,
        seats: &[&Rc<WlSeatGlobal>],
        outputs: &[&Rc<WlOutputGlobal>],
    ) {
        let mut layout_job = LayoutJob::default();
        layout_job.append(
            "Device",
            0.0,
            TextFormat {
                color: ui.style().visuals.widgets.inactive.text_color(),
                ..Default::default()
            },
        );
        layout_job.append(
            &dev.device.name(),
            10.0,
            TextFormat {
                color: ui.style().visuals.widgets.active.text_color(),
                ..Default::default()
            },
        );
        let dev_id = dev.device.id();
        CollapsingHeader::new(layout_job)
            .id_salt(("device", dev_id))
            .show(ui, |ui| {
                grid(ui, ("device", dev_id), |ui| {
                    {
                        let old = dev.seat.get();
                        let ui = &mut *ui.row();
                        grid_label(ui, "Seat");
                        let mut seat = old.as_ref();
                        ui.horizontal(|ui| {
                            let mut cb = ComboBox::from_id_salt("seat");
                            if let Some(seat) = seat {
                                cb = cb.selected_text(seat.seat_name());
                            }
                            cb.show_ui(ui, |ui| {
                                for s in seats {
                                    ui.selectable_value(&mut seat, Some(s), s.seat_name());
                                }
                            });
                            if ui.button("Detach").clicked() {
                                seat = None;
                            }
                        });
                        if seat != old.as_ref() {
                            dev.set_seat(&self.state, seat.cloned());
                        }
                    }
                    macro_rules! string {
                        ($field:ident, $name:expr) => {
                            if let Some(v) = &dev.$field {
                                label(ui, $name, v);
                            }
                        };
                    }
                    string!(syspath, "Syspath");
                    string!(devnode, "Devnode");
                    {
                        let ui = &mut *ui.row();
                        grid_label(ui, "Capabilities");
                        let mut s = String::new();
                        for cap in InputDeviceCapability::variants() {
                            if dev.device.has_capability(cap) {
                                if s.is_not_empty() {
                                    s.push_str(" | ");
                                }
                                s.push_str(cap.text());
                            }
                        }
                        ui.label(s);
                    }
                    if let Some(old) = dev.device.natural_scrolling_enabled() {
                        bool(ui, "Natural Scrolling", old, |v| {
                            dev.set_natural_scrolling_enabled(&self.state, v)
                        });
                    }
                    if dev.device.has_capability(InputDeviceCapability::Pointer) {
                        drag_value_ui(
                            ui,
                            "Scroll Distance (px)",
                            |ui| {
                                tip(ui, |ui| {
                                    ui.label(concat!(
                                        "This only applies to applications that use the ",
                                        "legacy px scrolling events.",
                                    ));
                                });
                            },
                            dev.px_per_scroll_wheel.get(),
                            -f64::INFINITY..=f64::INFINITY,
                            0.1,
                            |v| dev.set_px_per_scroll_wheel(&self.state, v),
                        );
                    }
                    if let Some(old) = dev.device.accel_profile() {
                        combo_box(ui, "Accel Profile", old, |v| {
                            dev.set_accel_profile(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.accel_speed() {
                        drag_value(ui, "Accel Speed", old, 0.0..=1.0, 0.01, |v| {
                            dev.set_accel_speed(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.click_method() {
                        combo_box(ui, "Click Method", old, |v| {
                            dev.set_click_method(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.tap_enabled() {
                        bool(ui, "Tap Enabled", old, |v| {
                            dev.set_tap_enabled(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.drag_enabled() {
                        bool(ui, "Tap Drag Enabled", old, |v| {
                            dev.set_drag_enabled(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.drag_lock_enabled() {
                        bool(ui, "Tap Drag Lock Enabled", old, |v| {
                            dev.set_drag_lock_enabled(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.left_handed() {
                        bool(ui, "Left Handed", old, |v| {
                            dev.set_left_handed(&self.state, v)
                        });
                    }
                    if let Some(old) = dev.device.middle_button_emulation_enabled() {
                        bool(ui, "Middle Button Emulation", old, |v| {
                            dev.set_middle_button_emulation_enabled(&self.state, v)
                        });
                    }
                    {
                        let ui = &mut *ui.row();
                        grid_label_ui(ui, |ui| {
                            ui.label("Output");
                            tip(ui, |ui| {
                                ui.label("This applies to touch and tablet input.");
                            });
                        });
                        ui.horizontal(|ui| {
                            let old = dev.output.get().and_then(|v| v.global.get());
                            let old = old.as_ref();
                            let mut v = old;
                            let mut cb = ComboBox::from_id_salt("output");
                            if let Some(v) = v {
                                cb = cb.selected_text(&*v.connector.name);
                            }
                            cb.show_ui(ui, |ui| {
                                for &output in outputs {
                                    ui.selectable_value(
                                        &mut v,
                                        Some(output),
                                        &*output.connector.name,
                                    );
                                }
                            });
                            if v != old {
                                dev.set_output(&self.state, v.map(|v| &**v));
                            }
                            if ui.button("Detach").clicked() {
                                dev.set_output(&self.state, None);
                            }
                        });
                    }
                    matrix_ui(
                        ui,
                        "Transform Matrix",
                        |ui| {
                            tip(ui, |ui| {
                                ui.label("This matrix is applied to relative pointer movements.");
                            });
                        },
                        dev.device
                            .has_capability(InputDeviceCapability::Pointer)
                            .then(|| {
                                dev.device
                                    .transform_matrix()
                                    .unwrap_or([[1.0, 0.0], [0.0, 1.0]])
                            }),
                        |v| dev.set_transform_matrix(&self.state, v),
                    );
                    matrix(
                        ui,
                        "Calibration Matrix",
                        dev.device.calibration_matrix(),
                        |v| dev.set_calibration_matrix(&self.state, v),
                    );
                });
                if dev.device.has_capability(InputDeviceCapability::Keyboard) {
                    ui.collapsing("Device Keymap", |ui| {
                        let ks = self.keymaps.entry(Key::Dev(dev_id)).or_default();
                        let map = dev.keymap.get();
                        ui.add_enabled_ui(map.is_some(), |ui| {
                            if ui.button("Use Seat Keymap").clicked() {
                                ks.backup(map.as_ref());
                                dev.set_keymap(&self.state, None);
                            }
                        });
                        show_keymap(
                            &self.state,
                            ps,
                            &mut self.paste_requested,
                            ks,
                            ui,
                            map.as_ref(),
                            |m| {
                                dev.set_keymap(&self.state, Some(m.clone()));
                            },
                        );
                    });
                }
            });
    }
}

impl KeymapState {
    fn backup(&mut self, map: Option<&Rc<KbvmMap>>) {
        if self.backup.is_none()
            && let Some(map) = map
        {
            self.backup = Some(map.clone());
        }
    }
}

fn show_keymap(
    state: &State,
    ps: &mut PaneState,
    paste_requested: &mut Option<Id>,
    ks: &mut KeymapState,
    ui: &mut Ui,
    map: Option<&Rc<KbvmMap>>,
    set_map: impl Fn(&Rc<KbvmMap>),
) {
    ui.scope_builder(UiBuilder::new().id(("keymap-settings", ks.seed)), |ui| {
        ui.add_enabled_ui(map.is_some(), |ui| {
            if ui.button("Copy Keymap").clicked()
                && let Some(map) = map
            {
                ui.ctx().copy_text(map.map_text.clone());
            }
        });
        let backup = |ks: &mut KeymapState| {
            ks.backup(map);
        };
        if ui.button("Load Default Keymap").clicked() {
            backup(ks);
            set_map(&state.default_keymap);
        }
        ui.horizontal(|ui| {
            ui.add_enabled_ui(map.is_some(), |ui| {
                if ui.button("Backup Keymap").clicked() {
                    ks.backup = None;
                    backup(ks);
                }
            });
            if let Some(backup) = &ks.backup
                && ui.button("Restore Keymap").clicked()
            {
                set_map(backup);
                ks.backup = None;
            }
        });
        let mut label = "Load Keymap from Clipboard".to_string();
        if *paste_requested == Some(ui.id()) {
            label.push_str(" ");
            label.push_str(ICON_PENDING);
        }
        let button = ui.button(label);
        if button.clicked() {
            *paste_requested = Some(ui.id());
            button.request_focus();
            ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
        } else if *paste_requested == Some(ui.id()) && button.has_focus() {
            ui.input(|e| {
                let map = e
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Paste(s) => Some(s),
                        _ => None,
                    })
                    .next();
                let Some(map) = map else {
                    return;
                };
                *paste_requested = None;
                let map = match state.kb_ctx.parse_keymap(map.as_bytes()) {
                    Ok(m) => m,
                    Err(e) => {
                        let error = format!("Could not parse keymap: {}", ErrorFmt(e));
                        ps.errors.push(error);
                        return;
                    }
                };
                backup(ks);
                set_map(&map);
            });
        } else if *paste_requested == Some(ui.id()) {
            *paste_requested = None;
        }
        ui.collapsing("Create Keymap from Names", |ui| {
            grid(ui, ("keymap-from-names", ui.id()), |ui| {
                let defaulted =
                    |ui: &mut Ui, name: &str, default: &mut bool, text: &mut dyn TextBuffer| {
                        let ui = &mut *ui.row();
                        grid_label(ui, name);
                        ui.add_enabled_ui(!*default, |ui| {
                            text_edit(ui, text);
                        });
                        ui.checkbox(default, "Default");
                    };
                let required = |ui: &mut Ui, name, text| {
                    let ui = &mut *ui.row();
                    grid_label(ui, name);
                    text_edit(ui, text);
                };
                defaulted(ui, "Rules", &mut ks.rules_default, &mut ks.rules);
                defaulted(ui, "Model", &mut ks.model_default, &mut ks.model);
                required(ui, "Layouts", &mut ks.layouts);
                required(ui, "Variants", &mut ks.variants);
                required(ui, "Options", &mut ks.options);
            });
            if ui.button("Load").clicked() {
                'set_map: {
                    let map = state.kb_ctx.keymap_from_rmlvo(
                        (!ks.rules_default).then_some(&ks.rules),
                        (!ks.model_default).then_some(&ks.model),
                        Some(&ks.layouts),
                        Some(&ks.variants),
                        Some(&ks.options),
                    );
                    let map = match map {
                        Ok(map) => map,
                        Err(e) => {
                            let error = format!("Could not parse keymap: {}", ErrorFmt(e));
                            ps.errors.push(error);
                            break 'set_map;
                        }
                    };
                    backup(ks);
                    set_map(&map);
                }
            }
        });
    });
}

fn matrix<T, const W: usize>(
    ui: &mut Ui,
    name: &str,
    old: Option<[[T; W]; 2]>,
    set: impl FnOnce([[T; W]; 2]),
) where
    T: Numeric,
{
    matrix_ui(ui, name, |_| (), old, set);
}

fn matrix_ui<R, T, const W: usize>(
    ui: &mut Ui,
    name: &str,
    label: impl FnOnce(&mut Ui) -> R,
    old: Option<[[T; W]; 2]>,
    set: impl FnOnce([[T; W]; 2]),
) where
    T: Numeric,
{
    if let Some(mut m) = old {
        let old = m;
        let ui = &mut *ui.row();
        grid_label_ui(ui, |ui| {
            ui.label(name);
            label(ui);
        });
        Grid::new(name).show(ui, |ui| {
            for row in &mut m {
                for cell in row {
                    DragValue::new(cell).speed(0.01).ui(ui);
                }
                ui.end_row();
            }
        });
        if old != m {
            set(m);
        }
    }
}
