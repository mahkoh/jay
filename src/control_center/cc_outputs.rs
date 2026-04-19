use {
    crate::{
        backend::{
            BackendColorSpace, BackendEotfs, ConnectorId, Mode,
            transaction::{
                BackendConnectorTransactionError, ConnectorTransaction,
                PreparedConnectorTransaction,
            },
        },
        cmm::cmm_luminance::Luminance,
        compositor::{MAX_EXTENTS, MAX_SCALE, MIN_SCALE},
        control_center::{ControlCenterInner, GridExt, PaneState, grid, grid_label, label, tip},
        egui_adapter::{
            egui_oklch::Color32Ext,
            egui_platform::icons::{ICON_ADD, ICON_REMOVE},
        },
        ifs::{
            head_management::{HeadName, HeadState, ReadOnlyHeadState},
            wl_output::BlendSpace,
        },
        scale::{SCALE_BASE, SCALE_BASEF, Scale},
        state::State,
        tree::{TearingMode, Transform, VrrMode},
        utils::{errorfmt::ErrorFmt, static_text::StaticText},
    },
    ahash::AHashMap,
    egui::{
        Align, Button, Checkbox, CollapsingHeader, Color32, ComboBox, DragValue, EventFilter,
        FontId, Frame, Grid, Id, Key, Layout, PointerButton, Rect, ScrollArea, Sense, Shadow,
        Stroke, StrokeKind, Style, TextFormat, Ui, UiBuilder, Vec2, Widget, WidgetText, emath,
        pos2, text::LayoutJob, vec2,
    },
    egui_tiles::{
        Behavior, Container, Linear, LinearDir, ResizeState, SimplificationOptions, Tile, TileId,
        Tiles, Tree, UiResponse,
    },
    linearize::{Linearize, LinearizeExt},
    rand::random,
    serde::{Deserialize, Serialize},
    std::{
        cell::{Cell, Ref},
        fmt,
        rc::Rc,
    },
    thiserror::Error,
};

pub struct OutputsPane {
    tree: Tree<Pane>,
    root_id: TileId,
    arrangement_id: Option<TileId>,
    inner: OutputsPaneInner,
}

struct OutputsPaneInner {
    state: Rc<State>,
    in_transaction: Cell<bool>,
    heads: AHashMap<HeadName, CompleteHead>,
    ui: UiSettings,
    settings: Settings,
    seed: u64,
}

enum Pane {
    Arrangement,
    Settings,
}

struct CompleteHead {
    id: ConnectorId,
    name: HeadName,
    pretty_name: Rc<String>,
    live_state: ReadOnlyHeadState,
    changed_state: Option<HeadState>,
    z: u64,
    focus: u64,
    drag_pos: Option<(f32, f32)>,
}

struct UiSettings {
    scale: f32,
    origin: Vec2,
    origin_drag: Option<Vec2>,
    next_z: u64,
    focus: u64,
    zoom_to_fit: bool,
    view: View,
}

struct Settings {
    show_guide_lines: bool,
    snap_to_neighbor: bool,
    show_disconnected: bool,
    show_disabled: bool,
    show_arrangement: bool,
    layout: UiLayout,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_guide_lines: false,
            snap_to_neighbor: true,
            show_disconnected: false,
            show_disabled: true,
            show_arrangement: true,
            layout: UiLayout::Auto,
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Linearize)]
enum UiLayout {
    Auto,
    Vertical,
    Horizontal,
}

#[derive(Copy, Clone)]
pub enum View {
    Connectors,
    Settings,
}

#[derive(Error, Debug)]
enum HeadTransactionError {
    #[error("The connector {} has been removed", .0)]
    HeadRemoved(Rc<String>),
    #[error("The display connected to connector {} has changed", .0)]
    MonitorChanged(Rc<String>),
    #[error(transparent)]
    Backend(#[from] BackendConnectorTransactionError),
}

macro_rules! effective {
    ($m:expr, $t:expr) => {
        $t.as_ref().unwrap_or($m)
    };
}

macro_rules! modify {
    ($m:expr, $t:expr) => {
        $t.get_or_insert_with(|| $m.clone())
    };
}

impl ControlCenterInner {
    pub fn create_outputs_pane(self: &Rc<Self>) -> OutputsPane {
        let seed = random();
        let mut tiles = Tiles::default();
        let settings_id = tiles.insert_pane(Pane::Settings);
        let arrangement_id = tiles.insert_pane(Pane::Arrangement);
        let root_id = tiles.insert_container(Linear::new(
            LinearDir::Horizontal,
            vec![arrangement_id, settings_id],
        ));
        let tree = Tree::new(Id::new(("cc_outputs", seed)), root_id, tiles);
        let mut pane = OutputsPane {
            root_id,
            arrangement_id: Some(arrangement_id),
            tree,
            inner: OutputsPaneInner {
                state: self.state.clone(),
                ui: UiSettings {
                    scale: 0.1,
                    origin: Default::default(),
                    origin_drag: None,
                    next_z: 0,
                    focus: 0,
                    zoom_to_fit: true,
                    view: View::Connectors,
                },
                settings: Default::default(),
                in_transaction: Default::default(),
                heads: Default::default(),
                seed,
            },
        };
        pane.inner.reset();
        pane
    }
}

struct B<'a>(&'a mut OutputsPaneInner, &'a mut PaneState);

impl Behavior<Pane> for B<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: TileId, pane: &mut Pane) -> UiResponse {
        Frame::new().inner_margin(5.0).show(ui, |ui| match pane {
            Pane::Arrangement => self.0.show_arrangement(ui),
            Pane::Settings => self.0.show_main_area(self.1, ui),
        });
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, _pane: &Pane) -> WidgetText {
        "".into()
    }

    fn gap_width(&self, _style: &Style) -> f32 {
        5.0
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            prune_empty_tabs: false,
            prune_empty_containers: false,
            prune_single_child_tabs: false,
            prune_single_child_containers: false,
            all_panes_must_have_tabs: false,
            join_nested_linear_containers: false,
        }
    }

    fn resize_stroke(&self, style: &Style, resize_state: ResizeState) -> Stroke {
        match resize_state {
            ResizeState::Idle => style.visuals.widgets.noninteractive.bg_stroke,
            ResizeState::Hovering => style.visuals.widgets.hovered.fg_stroke,
            ResizeState::Dragging => style.visuals.widgets.active.fg_stroke,
        }
    }
}

impl OutputsPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Outputs");
        if self.inner.in_transaction.get() {
            res.push_str(" (*)");
        }
    }

    pub fn show(&mut self, ps: &mut PaneState, ui: &mut Ui) {
        self.inner.add_new_heads();
        if let Some(id) = self.arrangement_id {
            if !self.inner.settings.show_arrangement {
                self.tree.remove_recursively(id);
                self.arrangement_id = None;
            }
        } else {
            if self.inner.settings.show_arrangement {
                let id = self.tree.tiles.insert_pane(Pane::Arrangement);
                self.tree.move_tile_to_container(id, self.root_id, 0, false);
                self.arrangement_id = Some(id);
            }
        }
        let show_vertical = match self.inner.settings.layout {
            UiLayout::Auto => ui.available_width() < 1024.0,
            UiLayout::Vertical => true,
            UiLayout::Horizontal => false,
        };
        if let Some(root) = self.tree.tiles.get_mut(self.root_id)
            && let Tile::Container(root) = root
            && let Container::Linear(root) = root
        {
            root.dir = match show_vertical {
                true => LinearDir::Vertical,
                false => LinearDir::Horizontal,
            };
        }
        self.tree.ui(&mut B(&mut self.inner, ps), ui)
    }
}

impl OutputsPaneInner {
    fn show_main_area(&mut self, ps: &mut PaneState, ui: &mut Ui) {
        ui.scope_builder(
            UiBuilder::new().id(Id::new(("main_area", self.seed))),
            |ui| {
                self.show_settings_bar(ps, ui);
                ScrollArea::vertical().show(ui, |ui| {
                    match self.ui.view {
                        View::Connectors => self.show_connectors(ui),
                        View::Settings => self.show_settings(ui),
                    }
                    ui.allocate_space(ui.available_size());
                });
            },
        );
    }

    fn show_settings_bar(&mut self, ps: &mut PaneState, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Connectors").clicked() {
                self.ui.view = View::Connectors;
                ui.request_repaint();
            }
            if ui.button("Settings").clicked() {
                self.ui.view = View::Settings;
                ui.request_repaint();
            }
            if ui
                .checkbox(&mut self.ui.zoom_to_fit, "Zoom To Fit")
                .changed()
            {
                ui.request_repaint();
            }
            {
                let mut reset = !self.in_transaction.get();
                let reset2 = reset;
                let widget = Checkbox::new(&mut reset, "Reset");
                if reset2 {
                    ui.add_enabled(false, widget);
                } else {
                    if widget.ui(ui).changed() {
                        self.reset();
                    }
                }
            }
            ui.with_layout(Layout::right_to_left(Align::LEFT), |ui| {
                let enabled = self.in_transaction.get();
                ui.add_enabled_ui(enabled, |ui| {
                    if ui.button("Test").clicked() {
                        if let Err(e) = self.test_transaction() {
                            ps.errors.push(ErrorFmt(e).to_string());
                        }
                    }
                });
                let enabled = self.in_transaction.get();
                ui.add_enabled_ui(enabled, |ui| {
                    let button = Button::new("Commit").fill(ui.style().visuals.extreme_bg_color);
                    if ui.add(button).clicked() {
                        match self.commit_transaction() {
                            Ok(_) => self.reset(),
                            Err(e) => {
                                ps.errors.push(ErrorFmt(e).to_string());
                            }
                        }
                    }
                });
                ui.add_space(ui.available_width());
            });
        });
        ui.separator();
    }

    fn show_connectors(&mut self, ui: &mut Ui) {
        let mut heads: Vec<_> = self.heads.values_mut().collect();
        heads.sort_by(|a, b| {
            a.live_state
                .borrow()
                .name
                .cmp(&b.live_state.borrow().name)
                .then_with(|| a.name.cmp(&b.name))
        });
        let mut is_in_transaction = false;
        for head in &mut heads {
            show_connector(&self.state, &self.settings, head, ui);
            if head.changed_state.is_some() {
                is_in_transaction = true;
            }
        }
        self.in_transaction.set(is_in_transaction);
    }

    fn show_settings(&mut self, ui: &mut Ui) {
        let mut changed = false;

        {
            changed |= ui
                .checkbox(&mut self.settings.show_guide_lines, "Show guide lines")
                .changed();
        }

        {
            ui.horizontal(|ui| {
                changed |= ui
                    .checkbox(&mut self.settings.snap_to_neighbor, "Snap to neighbor")
                    .changed();
                tip(ui, |ui| {
                    ui.label("Hold Shift to invert this");
                });
            });
        }

        {
            ui.checkbox(&mut self.settings.show_arrangement, "Show arrangement area");
        }

        {
            let layout_text = |l: UiLayout| match l {
                UiLayout::Auto => "Auto",
                UiLayout::Vertical => "Vertical",
                UiLayout::Horizontal => "Horizontal",
            };
            changed |= ComboBox::new("layout", "Layout")
                .selected_text(layout_text(self.settings.layout))
                .show_ui(ui, |ui| {
                    for l in UiLayout::variants() {
                        ui.selectable_value(&mut self.settings.layout, l, layout_text(l));
                    }
                })
                .response
                .changed();
        }

        {
            changed |= ui
                .checkbox(
                    &mut self.settings.show_disconnected,
                    "Show disconnected heads",
                )
                .changed();
        }

        {
            changed |= ui
                .checkbox(&mut self.settings.show_disabled, "Show disabled heads")
                .changed();
        }

        if changed {
            ui.request_repaint();
        }
    }

    fn show_arrangement(&mut self, ui: &mut Ui) {
        let clip_rect = ui.available_rect_before_wrap();
        let origin = &mut self.ui.origin;
        let ox = origin.x.round();
        let oy = origin.y.round();
        let mut heads = vec![];
        let scale = self.ui.scale;
        struct PreparedHead<'a> {
            name: HeadName,
            m: Ref<'a, HeadState>,
            changed_state: &'a mut Option<HeadState>,
            z: &'a mut u64,
            focus: &'a mut u64,
            drag_pos: &'a mut Option<(f32, f32)>,
            x1: i32,
            y1: i32,
            x2: i32,
            y2: i32,
            rect: Rect,
        }
        for head in self.heads.values_mut() {
            let m = head.live_state.borrow();
            let e = effective!(&*m, head.changed_state);
            if !e.in_compositor_space {
                continue;
            }
            let (x, y) = e.position;
            let (w, h) = e.size;
            let x1 = (x as f32 * scale).round() - ox + clip_rect.min.x;
            let y1 = (y as f32 * scale).round() - oy + clip_rect.min.y;
            let x2 = ((x + w) as f32 * scale).round() - ox + clip_rect.min.x;
            let y2 = ((y + h) as f32 * scale).round() - oy + clip_rect.min.y;
            heads.push(PreparedHead {
                name: head.name,
                m,
                changed_state: &mut head.changed_state,
                z: &mut head.z,
                focus: &mut head.focus,
                drag_pos: &mut head.drag_pos,
                x1: x,
                y1: y,
                x2: x + w,
                y2: y + h,
                rect: Rect {
                    min: pos2(x1, y1),
                    max: pos2(x2, y2),
                },
            });
        }
        if self.ui.zoom_to_fit {
            let mut x_min = i32::MAX;
            let mut x_max = i32::MIN;
            let mut y_min = i32::MAX;
            let mut y_max = i32::MIN;
            for head in &heads {
                x_min = x_min.min(head.x1);
                x_max = x_max.max(head.x2);
                y_min = y_min.min(head.y1);
                y_max = y_max.max(head.y2);
            }
            if x_min > x_max {
                x_min = 0;
                x_max = 0;
            }
            if y_min > y_max {
                y_min = 0;
                y_max = 0;
            }
            x_min -= 100;
            y_min -= 100;
            x_max += 100;
            y_max += 100;
            let dx = x_max - x_min + 1;
            let dy = y_max - y_min + 1;
            let x_scale = clip_rect.width() / dx as f32;
            let y_scale = clip_rect.height() / dy as f32;
            let new_scale = x_scale.min(y_scale);
            let new_ox = x_min as f32 * new_scale;
            let new_oy = y_min as f32 * new_scale;
            if new_scale != scale || new_ox != ox || new_oy != oy {
                self.ui.scale = new_scale;
                origin.x = new_ox;
                origin.y = new_oy;
                ui.request_repaint();
            }
        }
        heads.sort_by_key(|h| *h.z);
        let style = &ui.style().visuals;
        let mut bg_color = style.panel_fill.to_oklch();
        let mut no_capture_bg_color = bg_color;
        let mut disabled_bg_color = bg_color;
        if bg_color.l > 0.5 {
            bg_color.l -= 0.05;
            disabled_bg_color.l -= 0.1;
            no_capture_bg_color.l -= 0.075;
        } else {
            bg_color.l += 0.05;
            disabled_bg_color.l += 0.1;
            no_capture_bg_color.l += 0.075;
        }
        let fg_color_base = style.widgets.noninteractive.text_color().to_oklab();
        let fg_color_1 = fg_color_base * 1.0 / 3.0 + bg_color.to_oklab() * 2.0 / 3.0;
        let fg_color_2 = fg_color_base * 2.0 / 3.0 + bg_color.to_oklab() * 1.0 / 3.0;
        let painter = ui.painter_at(clip_rect);
        painter.rect(
            Rect::EVERYTHING,
            0.0,
            disabled_bg_color,
            Stroke::NONE,
            StrokeKind::Inside,
        );
        {
            let x_min = 0.0;
            let y_min = 0.0;
            let x_max = MAX_EXTENTS as f32;
            let y_max = MAX_EXTENTS as f32;
            let good = Rect {
                min: clip_rect.min + vec2(x_min, y_min) * scale - *origin,
                max: clip_rect.min + vec2(x_max, y_max) * scale - *origin,
            };
            painter.rect(good, 0.0, bg_color, Stroke::NONE, StrokeKind::Inside);
        }
        painter.hline(
            clip_rect.left()..=clip_rect.right(),
            clip_rect.min.y - origin.y,
            (1.0, fg_color_1),
        );
        painter.vline(
            clip_rect.min.x - origin.x,
            clip_rect.top()..=clip_rect.bottom(),
            (1.0, fg_color_1),
        );
        if self.settings.show_guide_lines {
            for head in &heads {
                let rect = head.rect;
                painter.hline(
                    clip_rect.left()..=clip_rect.right(),
                    rect.top(),
                    (1.0, fg_color_2),
                );
                painter.hline(
                    clip_rect.left()..=clip_rect.right(),
                    rect.bottom(),
                    (1.0, fg_color_2),
                );
                painter.vline(
                    rect.left(),
                    clip_rect.top()..=clip_rect.bottom(),
                    (1.0, fg_color_2),
                );
                painter.vline(
                    rect.right(),
                    clip_rect.top()..=clip_rect.bottom(),
                    (1.0, fg_color_2),
                );
            }
        }
        for head in &mut heads {
            let rect = head.rect;
            let mut color = fg_color_2;
            if *head.focus == self.ui.focus {
                let shape = Shadow {
                    offset: [0, 0],
                    blur: (512.0 * scale).sqrt() as u8,
                    spread: (255.0 * scale).sqrt() as u8,
                    color: Color32::from_black_alpha(200),
                }
                .as_shape(rect, 0.0);
                painter.add(shape);
                color = fg_color_base;
            }
            painter.hline(rect.left()..=rect.right() + 1.0, rect.top(), (1.0, color));
            painter.hline(
                rect.left()..=rect.right() + 1.0,
                rect.bottom(),
                (1.0, color),
            );
            painter.vline(rect.left(), rect.top()..=rect.bottom() + 1.0, (1.0, color));
            painter.vline(rect.right(), rect.top()..=rect.bottom() + 1.0, (1.0, color));
            let content_rect = Rect {
                min: pos2(rect.min.x + 1.0, rect.min.y + 1.0),
                max: pos2(rect.max.x, rect.max.y),
            };
            let painter = painter.with_clip_rect(content_rect);
            painter.rect(
                content_rect,
                0.0,
                no_capture_bg_color,
                Stroke::NONE,
                StrokeKind::Inside,
            );
            let galley =
                painter.layout_no_wrap(head.m.name.to_string(), FontId::default(), Color32::WHITE);
            let rect = Rect::from_min_size(content_rect.min, galley.rect.size() + vec2(2.0, 2.0));
            painter.rect(rect, 0.0, Color32::BLUE, Stroke::NONE, StrokeKind::Inside);
            painter.galley(rect.min + vec2(1.0, 1.0), galley, Color32::WHITE);
        }
        ui.allocate_space(ui.available_size());
        macro_rules! interacted {
            () => {{
                self.ui.zoom_to_fit = false;
            }};
        }
        let response = ui.allocate_rect(clip_rect, Sense::all());
        if response.has_focus() {
            let mut dx = 0;
            let mut dy = 0;
            ui.input(|i| {
                if i.key_pressed(Key::ArrowUp) {
                    dy -= 1;
                }
                if i.key_pressed(Key::ArrowDown) {
                    dy += 1;
                }
                if i.key_pressed(Key::ArrowLeft) {
                    dx -= 1;
                }
                if i.key_pressed(Key::ArrowRight) {
                    dx += 1;
                }
            });
            if dx != 0 || dy != 0 {
                interacted!();
                for head in &mut heads {
                    if *head.focus == self.ui.focus {
                        let x = (head.x1 + dx).clamp(0, MAX_EXTENTS);
                        let y = (head.y1 + dy).clamp(0, MAX_EXTENTS);
                        let pos = (x, y);
                        if effective!(&*head.m, head.changed_state).position != pos {
                            modify!(&*head.m, head.changed_state).position = pos;
                            ui.request_repaint();
                        }
                    }
                }
            }
        }
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let mut new = scale;
            if scroll.y != 0.0 {
                interacted!();
            }
            if scroll.y < 0.0 {
                new /= 1.0 - scroll.y / 1000.0;
            } else {
                new *= 1.0 + scroll.y / 1000.0;
            }
            new = new.max(0.01);
            if new != scale {
                self.ui.scale = new;
                ui.request_repaint();
                let relative_pos = pos - clip_rect.min;
                let real_pos = (relative_pos + *origin) / scale;
                *origin = real_pos * new - relative_pos;
            }
        }
        if ui.input(|i| i.pointer.button_pressed(PointerButton::Primary)) {
            self.ui.focus += 1;
            if let Some(pos) = response.hover_pos() {
                interacted!();
                response.request_focus();
                for head in heads.iter_mut().rev() {
                    if head.rect.contains(pos) {
                        *head.z = self.ui.next_z;
                        self.ui.next_z += 1;
                        *head.focus = self.ui.focus;
                        ui.request_repaint();
                        break;
                    }
                }
            }
        }
        if response.clicked_elsewhere() {
            self.ui.focus += 1;
        }
        if response.drag_started_by(PointerButton::Middle)
            || response.drag_started_by(PointerButton::Secondary)
        {
            interacted!();
            self.ui.origin_drag = Some(self.ui.origin);
        } else if response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = response.hover_pos()
        {
            interacted!();
            for head in heads.iter_mut().rev() {
                if head.rect.contains(pos) {
                    *head.drag_pos = Some((head.x1 as f32, head.y1 as f32));
                    break;
                }
            }
        }
        let drag_delta = response.drag_delta();
        if drag_delta.x != 0.0 || drag_delta.y != 0.0 {
            if let Some(origin_drag) = &mut self.ui.origin_drag {
                *origin_drag -= drag_delta;
                self.ui.origin = *origin_drag;
                ui.request_repaint();
            }
            let snap = self.settings.snap_to_neighbor ^ ui.input(|i| i.modifiers.shift);
            let mut head_positions = vec![];
            struct HeadPosition {
                name: HeadName,
                x1: i32,
                y1: i32,
                x2: i32,
                y2: i32,
            }
            if snap {
                for head in &heads {
                    let PreparedHead {
                        name,
                        x1,
                        y1,
                        x2,
                        y2,
                        ..
                    } = *head;
                    head_positions.push(HeadPosition {
                        name,
                        x1,
                        y1,
                        x2,
                        y2,
                    });
                }
            }
            for head in &mut heads {
                if let Some((mut x, mut y)) = *head.drag_pos {
                    x += drag_delta.x / scale;
                    y += drag_delta.y / scale;
                    let mut x_int = if x < 0.0 { x.ceil() } else { x.floor() } as i32;
                    let mut y_int = if y < 0.0 { y.ceil() } else { y.floor() } as i32;
                    if snap {
                        for other in &head_positions {
                            if head.name == other.name {
                                continue;
                            }
                            macro_rules! snap {
                                ($int:ident, $one:ident, $two:ident) => {
                                    if $int.abs() as f32 * scale <= 10.0 {
                                        $int = 0;
                                    } else if ($int - other.$one).abs() as f32 * scale <= 10.0 {
                                        $int = other.$one;
                                    } else if ($int - other.$two).abs() as f32 * scale <= 10.0 {
                                        $int = other.$two;
                                    } else if ($int + head.$two - head.$one - other.$one).abs()
                                        as f32
                                        * scale
                                        <= 10.0
                                    {
                                        $int = other.$one + head.$one - head.$two;
                                    } else if ($int + head.$two - head.$one - other.$two).abs()
                                        as f32
                                        * scale
                                        <= 10.0
                                    {
                                        $int = other.$two + head.$one - head.$two;
                                    }
                                };
                            }
                            snap!(x_int, x1, x2);
                            snap!(y_int, y1, y2);
                        }
                    }
                    x_int = x_int.clamp(0, MAX_EXTENTS);
                    y_int = y_int.clamp(0, MAX_EXTENTS);
                    let pos = (x_int, y_int);
                    if effective!(&*head.m, head.changed_state).position != pos {
                        modify!(&*head.m, head.changed_state).position = pos;
                        ui.request_repaint();
                    }
                    *head.drag_pos = Some((x, y));
                }
            }
        }
        if response.drag_stopped() {
            self.ui.origin_drag = None;
            for head in heads.iter_mut().rev() {
                *head.drag_pos = None;
            }
        }
        ui.memory_mut(|mem| {
            mem.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: false,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: false,
                },
            )
        });
    }

    fn prepare_transaction(&self) -> Result<PreparedConnectorTransaction, HeadTransactionError> {
        let mut tran = ConnectorTransaction::new(&self.state);
        for head in self.heads.values() {
            let Some(desired) = &head.changed_state else {
                continue;
            };
            let Some(connector) = self.state.connectors.get(&head.id) else {
                return Err(HeadTransactionError::HeadRemoved(head.pretty_name.clone()));
            };
            if head.live_state.borrow().monitor_info != desired.monitor_info {
                return Err(HeadTransactionError::MonitorChanged(
                    head.pretty_name.clone(),
                ));
            }
            let old = connector.state.borrow().clone();
            let mut new = old.clone();
            new.enabled = desired.connector_enabled;
            new.mode = desired.mode;
            new.non_desktop_override = desired.override_non_desktop;
            new.format = desired.format;
            new.color_space = desired.color_space;
            new.eotf = desired.eotf;
            if old == new {
                continue;
            }
            tran.add(&connector.connector, new)?;
        }
        Ok(tran.prepare()?)
    }

    fn commit_transaction(&self) -> Result<(), HeadTransactionError> {
        self.prepare_transaction()?.apply()?.commit();
        for head in self.heads.values() {
            let Some(desired) = &head.changed_state else {
                continue;
            };
            desired.flush_persistent_state(&self.state);
            if let Some(output) = self.state.outputs.get(&head.id)
                && let Some(node) = &output.node
            {
                node.set_position(desired.position.0, desired.position.1);
                node.set_preferred_scale(desired.scale);
                node.update_transform(desired.transform);
                node.set_vrr_mode(&desired.vrr_mode);
                node.set_tearing_mode(&desired.tearing_mode);
                node.set_brightness(desired.brightness);
                node.set_blend_space(desired.blend_space);
                node.set_use_native_gamut(desired.use_native_gamut);
                node.schedule
                    .set_cursor_hz(&self.state, desired.vrr_cursor_hz.unwrap_or(f64::INFINITY));
            } else if let Some(mi) = &desired.monitor_info {
                let pos = &self.state.persistent_output_states;
                let pos = pos.lock().entry(mi.output_id.clone()).or_default().clone();
                pos.pos.set(desired.position);
                pos.scale.set(desired.scale);
                pos.transform.set(desired.transform);
                pos.vrr_mode.set(desired.vrr_mode);
                pos.tearing_mode.set(desired.tearing_mode);
                pos.brightness.set(desired.brightness);
                pos.blend_space.set(desired.blend_space);
                pos.use_native_gamut.set(desired.use_native_gamut);
                pos.vrr_cursor_hz.set(desired.vrr_cursor_hz);
            }
        }
        Ok(())
    }

    fn test_transaction(&self) -> Result<(), HeadTransactionError> {
        self.prepare_transaction()?;
        Ok(())
    }

    fn reset(&mut self) {
        self.in_transaction.set(false);
        let mut to_remove = vec![];
        for head in self.heads.values_mut() {
            if self.state.connectors.contains(&head.id) {
                head.changed_state = None;
            } else {
                to_remove.push(head.name);
            }
        }
        for name in to_remove {
            self.heads.remove(&name);
        }
    }

    fn add_new_heads(&mut self) {
        for connector in self.state.connectors.lock().values() {
            let mgr = &connector.head_manager;
            self.heads.entry(mgr.name).or_insert_with(|| CompleteHead {
                id: connector.id,
                name: mgr.name,
                pretty_name: connector.name.clone(),
                live_state: mgr.state(),
                changed_state: None,
                z: 0,
                focus: 0,
                drag_pos: None,
            });
        }
    }
}

fn show_connector(state: &State, settings: &Settings, head: &mut CompleteHead, ui: &mut Ui) {
    let m = &*head.live_state.borrow();
    let t = &mut head.changed_state;
    if t.is_none() {
        if !m.connector_enabled && !settings.show_disabled {
            return;
        }
        if !m.connected && !settings.show_disconnected {
            return;
        }
    }
    let mut layout_job = LayoutJob::default();
    layout_job.append(
        "Connector",
        0.0,
        TextFormat {
            color: ui.style().visuals.widgets.inactive.text_color(),
            ..Default::default()
        },
    );
    layout_job.append(
        &m.name,
        10.0,
        TextFormat {
            color: ui.style().visuals.widgets.active.text_color(),
            ..Default::default()
        },
    );
    let mut name = String::new();
    if let Some(v) = &m.monitor_info {
        name.push_str(&v.output_id.manufacturer);
        name.push_str(" - ");
        name.push_str(&v.output_id.model);
    }
    layout_job.append(
        &name,
        10.0,
        TextFormat {
            color: ui.style().visuals.widgets.inactive.text_color(),
            ..Default::default()
        },
    );
    CollapsingHeader::new(layout_job)
        .id_salt(("connector", head.name))
        .show(ui, |ui| {
            grid(ui, ("settings", head.name), |ui| {
                let mut diff = false;
                show_serial_number(ui, m);
                diff |= show_enablement(state, ui, m, t);
                diff |= show_position(ui, m, t);
                diff |= show_scale(ui, m, t);
                diff |= show_mode(ui, m, t);
                diff |= show_size(ui, m, t);
                diff |= show_transform(ui, m, t);
                diff |= show_brightness(ui, m, t);
                diff |= show_color_space(ui, m, t);
                diff |= show_eotf(ui, m, t);
                diff |= show_format(ui, m, t);
                diff |= show_tearing(ui, m, t);
                diff |= show_vrr(ui, m, t);
                diff |= show_non_desktop(state, ui, m, t);
                diff |= show_blend_space(ui, m, t);
                diff |= show_use_native_gamut(ui, m, t);
                show_native_gamut(ui, m);
                diff |= show_cursor_hz(ui, m, t);
                show_flip_margin(state, ui, m, t, head.id);
                if diff {
                    let ui = &mut *ui.row();
                    ui.label("");
                    ui.label("");
                    ui.label("^ current");
                }
            });
        });
}

fn show_serial_number(ui: &mut Ui, m: &HeadState) {
    if let Some(info) = &m.monitor_info {
        let ui = &mut *ui.row();
        grid_label(ui, "Serial Number");
        ui.label(&info.output_id.serial_number);
    }
}

fn show_enablement(state: &State, ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    let ui = &mut *ui.row();
    grid_label(ui, "Enabled");
    let mut v = effective!(m, t).connector_enabled;
    let changed = Checkbox::without_text(&mut v).ui(ui).changed();
    if changed {
        let t = modify!(m, t);
        t.connector_enabled = v;
        t.update_in_compositor_space(state, m.wl_output);
    }
    let diff = v != m.connector_enabled;
    if diff {
        ui.label(match m.connector_enabled {
            true => "enabled",
            false => "disabled",
        });
    }
    diff
}

fn show_position(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Position");
    let (mut x, mut y) = effective!(m, t).position;
    ui.horizontal(|ui| {
        let value = |ui: &mut Ui, v, min, max| {
            let res = DragValue::new(v).range(min..=max).speed(1.0).ui(ui);
            res.changed()
        };
        let mut changed = false;
        changed |= value(ui, &mut x, 0, MAX_EXTENTS);
        ui.label("x");
        changed |= value(ui, &mut y, 0, MAX_EXTENTS);
        if changed {
            modify!(m, t).position = (x, y);
        }
    });
    let diff = m.position != (x, y);
    if diff {
        ui.label(format!("{} x {}", m.position.0, m.position.1));
    }
    diff
}

fn show_scale(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Scale");
    let mut v = effective!(m, t).scale;
    let old = v;
    ui.horizontal(|ui| {
        let mut s = v.to_f64();
        let res = DragValue::new(&mut s)
            .range(MIN_SCALE.to_f64()..=MAX_SCALE.to_f64())
            .speed(1.0 / SCALE_BASEF)
            .fixed_decimals(5)
            .ui(ui);
        if res.changed() {
            v = Scale::from_f64(s);
        }
        if ui.button(ICON_REMOVE).clicked() {
            v = Scale::from_wl(v.to_wl().saturating_sub(SCALE_BASE)).clamp(MIN_SCALE, MAX_SCALE);
        }
        if ui.button(ICON_ADD).clicked() {
            v = Scale::from_wl(v.to_wl().saturating_add(SCALE_BASE)).clamp(MIN_SCALE, MAX_SCALE);
        }
    });
    if old != v {
        let t = modify!(m, t);
        t.scale = v;
        t.update_size();
    }
    let diff = m.scale != v;
    if diff {
        ui.label(format!("{}", m.scale.to_f64()));
    }
    diff
}

fn show_mode(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    let mut mode = effective!(m, t).mode;
    let old = mode;
    grid_label(ui, "Mode");
    let mode_text = |mode: Mode| {
        format!(
            "{}x{}@{}",
            mode.width,
            mode.height,
            mode.refresh_rate_millihz as f64 / 1000.0
        )
    };
    if let Some(monitor_info) = &m.monitor_info
        && let Some(modes) = &monitor_info.modes
        && modes.len() > 1
    {
        ComboBox::from_id_salt("modes")
            .selected_text(mode_text(mode))
            .show_ui(ui, |ui| {
                for v in modes {
                    ui.selectable_value(&mut mode, *v, mode_text(*v));
                }
            });
    } else if let Some(monitor_info) = &m.monitor_info
        && monitor_info.modes.is_none()
    {
        ui.horizontal(|ui| {
            fn value<T: emath::Numeric>(ui: &mut Ui, v: &mut T, min: T, max: T) -> bool {
                let res = DragValue::new(v).range(min..=max).speed(1.0).ui(ui);
                res.changed()
            }
            value(ui, &mut mode.width, 1, u16::MAX as i32);
            ui.label("x");
            value(ui, &mut mode.height, 1, u16::MAX as i32);
            ui.label("@");
            let mut hz = mode.refresh_rate_millihz as f64 / 1_000.0;
            if value(ui, &mut hz, 0.0, 1_000_000.0) {
                mode.refresh_rate_millihz = (hz * 1_000.0).round() as u32;
            }
        });
    } else {
        ui.label(mode_text(mode));
    }
    if old != mode {
        let t = modify!(m, t);
        t.mode = mode;
        t.update_size();
    }
    let mut diff = false;
    if m.mode != mode {
        diff = true;
        ui.label(mode_text(m.mode));
    }
    diff
}

fn show_size(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if let Some(info) = &m.monitor_info {
        let ui = &mut *ui.row();
        grid_label(ui, "Physical Size (mm)");
        ui.label(format!("{} x {}", info.width_mm, info.height_mm));
    }
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Size");
    let (w, h) = effective!(m, t).size;
    ui.label(format!("{w} x {h}"));
    let diff = m.size != (w, h);
    if diff {
        ui.label(format!("{} x {}", m.size.0, m.size.1));
    }
    diff
}

fn show_transform(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Transform");
    let mut v = effective!(m, t).transform;
    let mut changed = false;
    ComboBox::from_id_salt("transform")
        .selected_text(v.text())
        .show_ui(ui, |ui| {
            let transforms = [
                Transform::None,
                Transform::Rotate90,
                Transform::Rotate180,
                Transform::Rotate270,
                Transform::Flip,
                Transform::FlipRotate90,
                Transform::FlipRotate180,
                Transform::FlipRotate270,
            ];
            for s in transforms {
                changed |= ui.selectable_value(&mut v, s, s.text()).changed();
            }
        });
    if changed {
        let t = modify!(m, t);
        t.transform = v;
        t.update_size();
    }
    let diff = m.transform != v;
    if diff {
        ui.label(m.transform.text());
    }
    diff
}

fn show_brightness(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let old_custom_brightness = effective!(m, t).brightness.is_some();
    let mut custom_brightness = old_custom_brightness;
    let mut changed = false;
    grid_label(ui, "Custom Brightness");
    Checkbox::without_text(&mut custom_brightness).ui(ui);
    changed |= old_custom_brightness != custom_brightness;
    let diff1 = m.brightness.is_some() != custom_brightness;
    if diff1 {
        ui.label(match m.brightness.is_some() {
            true => "enabled",
            false => "disabled",
        });
    }
    ui.end_row();

    if !custom_brightness {
        if changed {
            modify!(m, t).brightness = None;
        }
        return diff1;
    }

    grid_label(ui, "Brightness");
    ui.vertical(|ui| {
        let effective = effective!(m, t);
        let default_brightness = match effective.eotf {
            BackendEotfs::Default => effective
                .monitor_info
                .as_ref()
                .and_then(|m| m.luminance.as_ref())
                .map(|l| l.max)
                .unwrap_or(Luminance::SRGB.white.0),
            BackendEotfs::Pq => Luminance::ST2084_PQ.white.0,
        };
        let mut brightness = effective.brightness.unwrap_or(default_brightness);
        changed |= DragValue::new(&mut brightness)
            .range(0.0..=1000.0)
            .ui(ui)
            .changed();
        ui.label(format!("reference: {default_brightness})"));
        if changed {
            modify!(m, t).brightness = Some(brightness);
        }
    });
    let mut diff2 = false;
    if let Some(t) = t
        && m.brightness != t.brightness
    {
        diff2 = true;
        ui.label(format!(
            "{}",
            fmt::from_fn(|f| match m.brightness {
                None => f.write_str("disabled"),
                Some(v) => write!(f, "{}", v),
            })
        ));
    }
    ui.end_row();
    diff1 || diff2
}

fn show_color_space(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Colorimetry");
    let mut v = effective!(m, t).color_space;
    ui.horizontal(|ui| {
        if let Some(monitor_info) = &effective!(m, t).monitor_info {
            if monitor_info.color_spaces.is_empty() {
                ui.label(v.name());
            } else {
                let mut changed = false;
                ComboBox::from_id_salt("colorimetry")
                    .selected_text(v.name())
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut v,
                                BackendColorSpace::Default,
                                BackendColorSpace::Default.name(),
                            )
                            .changed();
                        for &s in &monitor_info.color_spaces {
                            changed |= ui.selectable_value(&mut v, s, s.name()).changed();
                        }
                    });
                if changed {
                    modify!(m, t).color_space = v;
                }
            }
        }
    });
    let diff = m.color_space != v;
    if diff {
        ui.label(m.color_space.name());
    }
    diff
}

fn show_eotf(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "EOTF");
    let mut v = effective!(m, t).eotf;
    ui.horizontal(|ui| {
        if let Some(monitor_info) = &effective!(m, t).monitor_info {
            if monitor_info.eotfs.is_empty() {
                ui.label(v.name());
            } else {
                let mut changed = false;
                ComboBox::from_id_salt("eotf")
                    .selected_text(v.name())
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut v,
                                BackendEotfs::Default,
                                BackendEotfs::Default.name(),
                            )
                            .changed();
                        for &s in &monitor_info.eotfs {
                            changed |= ui.selectable_value(&mut v, s, s.name()).changed();
                        }
                    });
                if changed {
                    modify!(m, t).eotf = v;
                }
            }
        }
    });
    let diff = m.eotf != v;
    if diff {
        ui.label(m.eotf.name());
    }
    diff
}

fn show_format(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Format");
    let mut v = effective!(m, t).format;
    ui.horizontal(|ui| {
        if m.supported_formats.len() < 2 {
            ui.label(v.name);
        } else {
            let mut changed = false;
            ComboBox::from_id_salt("format")
                .selected_text(v.name)
                .show_ui(ui, |ui| {
                    for &s in &*m.supported_formats {
                        changed |= ui.selectable_value(&mut v, s, s.name).changed();
                    }
                });
            if changed {
                modify!(m, t).format = v;
            }
        }
    });
    let diff = m.format != v;
    if diff {
        ui.label(m.format.name);
    }
    diff
}

fn show_tearing(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let render_settings = |ui: &mut Ui, old: TearingMode| {
        #[derive(Copy, Clone, PartialEq, Linearize)]
        enum Mode {
            Never,
            Always,
            Fullscreen,
        }
        fn name(mode: Mode) -> &'static str {
            match mode {
                Mode::Never => "Never",
                Mode::Always => "Always",
                Mode::Fullscreen => "Fullscreen",
            }
        }
        let mut mode = match old {
            TearingMode::Never => Mode::Never,
            TearingMode::Always => Mode::Always,
            TearingMode::Fullscreen { .. } => Mode::Fullscreen,
        };
        let old_mode = mode;
        let mut surface = None;
        ui.vertical(|ui| {
            ComboBox::from_id_salt("tearing mode")
                .selected_text(name(mode))
                .show_ui(ui, |ui| {
                    for s in Mode::variants() {
                        ui.selectable_value(&mut mode, s, name(s));
                    }
                });
            if mode == Mode::Fullscreen {
                if old_mode != mode {
                    surface = Some(Default::default());
                }
                if let TearingMode::Fullscreen { surface: s } = old {
                    surface = s;
                }
                let mut limit_windows = surface.is_some();
                ui.checkbox(&mut limit_windows, "Limit Windows");
                if !limit_windows {
                    surface = None;
                } else {
                    ui.indent("limit windows", |ui| {
                        let surface = surface.get_or_insert_default();
                        ui.checkbox(&mut surface.tearing_requested, "Requests Tearing");
                    });
                }
            }
        });
        match mode {
            Mode::Never => TearingMode::Never,
            Mode::Always => TearingMode::Always,
            Mode::Fullscreen => TearingMode::Fullscreen { surface },
        }
    };
    let ui = &mut *ui.row();
    grid_label(ui, "Tearing");
    let old = effective!(m, t).tearing_mode;
    let v = render_settings(ui, old);
    if v != old {
        modify!(m, t).tearing_mode = v;
    }
    let diff = v != m.tearing_mode;
    if diff {
        ui.add_enabled_ui(false, |ui| {
            render_settings(ui, m.tearing_mode);
        });
    }
    diff
}

fn show_vrr(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    if let Some(info) = &m.monitor_info
        && !info.vrr_capable
    {
        return false;
    }
    {
        let ui = &mut *ui.row();
        grid_label(ui, "VRR Active");
        ui.label(effective!(m, t).vrr.to_string());
    }
    let render_settings = |ui: &mut Ui, old: VrrMode| {
        #[derive(Copy, Clone, PartialEq, Linearize)]
        enum Mode {
            Never,
            Always,
            Fullscreen,
        }
        fn name(mode: Mode) -> &'static str {
            match mode {
                Mode::Never => "Never",
                Mode::Always => "Always",
                Mode::Fullscreen => "Fullscreen",
            }
        }
        let mut mode = match old {
            VrrMode::Never => Mode::Never,
            VrrMode::Always => Mode::Always,
            VrrMode::Fullscreen { .. } => Mode::Fullscreen,
        };
        let mut surface = None;
        ui.vertical(|ui| {
            ComboBox::from_id_salt("vrr mode")
                .selected_text(name(mode))
                .show_ui(ui, |ui| {
                    for s in Mode::variants() {
                        ui.selectable_value(&mut mode, s, name(s));
                    }
                });
            if mode == Mode::Fullscreen {
                if let VrrMode::Fullscreen { surface: s } = old {
                    surface = s;
                }
                let mut limit_windows = surface.is_some();
                ui.checkbox(&mut limit_windows, "Limit Windows");
                if !limit_windows {
                    surface = None;
                } else {
                    ui.indent("limit windows", |ui| {
                        let surface = surface.get_or_insert_default();
                        let mut limit_content_type = surface.content_type.is_some();
                        ui.checkbox(&mut limit_content_type, "Limit Content Types");
                        if !limit_content_type {
                            surface.content_type = None;
                        } else {
                            ui.indent("limit content type", |ui| {
                                let limit = surface.content_type.get_or_insert_default();
                                let fields = [
                                    ("Photos", &mut limit.photo),
                                    ("Videos", &mut limit.video),
                                    ("Games", &mut limit.game),
                                ];
                                for (name, field) in fields {
                                    ui.checkbox(field, name);
                                }
                            });
                        }
                    });
                }
            }
        });
        match mode {
            Mode::Never => VrrMode::Never,
            Mode::Always => VrrMode::Always,
            Mode::Fullscreen => VrrMode::Fullscreen { surface },
        }
    };
    let ui = &mut *ui.row();
    grid_label(ui, "VRR");
    let old = effective!(m, t).vrr_mode;
    let v = render_settings(ui, old);
    if v != old {
        modify!(m, t).vrr_mode = v;
    }
    let diff = v != m.vrr_mode;
    if diff {
        ui.add_enabled_ui(false, |ui| {
            render_settings(ui, m.vrr_mode);
        });
    }
    diff
}

fn show_non_desktop(state: &State, ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    {
        let ui = &mut *ui.row();
        grid_label(ui, "Non-desktop");
        if m.inherent_non_desktop {
            ui.label("Yes");
        } else {
            ui.label("No");
        }
    }

    let ui = &mut *ui.row();
    grid_label(ui, "Override");
    let mut v = effective!(m, t).override_non_desktop;
    let mut changed = false;
    let name = |v: Option<bool>| match v {
        None => "None",
        Some(false) => "Desktop",
        Some(true) => "Non-Desktop",
    };
    ComboBox::from_id_salt("non-desktop-override")
        .selected_text(name(v))
        .show_ui(ui, |ui| {
            for s in [None, Some(false), Some(true)] {
                changed |= ui.selectable_value(&mut v, s, name(s)).changed();
            }
        });
    if changed {
        let t = modify!(m, t);
        t.override_non_desktop = v;
        t.update_in_compositor_space(state, m.wl_output);
    }
    let diff = v != m.override_non_desktop;
    if diff {
        ui.label(name(m.override_non_desktop));
    }
    diff
}

fn show_blend_space(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Blend Space");
    let mut v = effective!(m, t).blend_space;
    ui.horizontal(|ui| {
        let mut changed = false;
        ComboBox::from_id_salt("blend-space")
            .selected_text(v.name())
            .show_ui(ui, |ui| {
                for s in BlendSpace::variants() {
                    changed |= ui.selectable_value(&mut v, s, s.name()).changed();
                }
            });
        if changed {
            modify!(m, t).blend_space = v;
        }
    });
    let diff = m.blend_space != v;
    if diff {
        ui.label(m.blend_space.name());
    }
    diff
}

fn show_use_native_gamut(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let ui = &mut *ui.row();
    grid_label(ui, "Use Native Gamut");
    let mut use_native_gamut = effective!(m, t).use_native_gamut;
    if Checkbox::without_text(&mut use_native_gamut)
        .ui(ui)
        .changed()
    {
        modify!(m, t).use_native_gamut = use_native_gamut;
    }
    let diff = m.use_native_gamut != use_native_gamut;
    if diff {
        let mut old = m.use_native_gamut;
        ui.add_enabled(false, Checkbox::without_text(&mut old));
    }
    diff
}

fn show_native_gamut(ui: &mut Ui, m: &HeadState) {
    let Some(info) = &m.monitor_info else {
        return;
    };
    let p = info.primaries;
    let ui = &mut *ui.row();
    grid_label(ui, "Native Gamut");
    Grid::new("native gamut").show(ui, |ui| {
        let fields = [
            ("red:", p.r),
            ("green:", p.g),
            ("blue:", p.b),
            ("white:", p.wp),
        ];
        for (name, field) in fields {
            let ui = &mut *ui.row();
            ui.label(name);
            ui.label(format!("{:.6}", field.0));
            ui.label(format!("{:.6}", field.1));
        }
    });
}

fn show_cursor_hz(ui: &mut Ui, m: &HeadState, t: &mut Option<HeadState>) -> bool {
    if !effective!(m, t).in_compositor_space {
        return false;
    }
    let old_cursor_hz = effective!(m, t).vrr_cursor_hz.is_some();
    let mut custom_cursor_hz = old_cursor_hz;
    let mut changed = false;
    grid_label(ui, "Limit Cursor HZ");
    Checkbox::without_text(&mut custom_cursor_hz).ui(ui);
    changed |= old_cursor_hz != custom_cursor_hz;
    let diff1 = m.vrr_cursor_hz.is_some() != custom_cursor_hz;
    if diff1 {
        ui.label(match m.vrr_cursor_hz.is_some() {
            true => "enabled",
            false => "disabled",
        });
    }
    ui.end_row();

    if !custom_cursor_hz {
        if changed {
            modify!(m, t).vrr_cursor_hz = None;
        }
        return diff1;
    }

    grid_label(ui, "Cursor HZ");
    let mut cursor_hz = effective!(m, t).vrr_cursor_hz.unwrap_or(60.0);
    changed |= DragValue::new(&mut cursor_hz)
        .range(0.0..=500.0)
        .ui(ui)
        .changed();
    if changed {
        modify!(m, t).vrr_cursor_hz = Some(cursor_hz);
    }
    let mut diff2 = false;
    if let Some(t) = t
        && m.vrr_cursor_hz != t.vrr_cursor_hz
    {
        diff2 = true;
        ui.label(format!(
            "{}",
            fmt::from_fn(|f| match m.vrr_cursor_hz {
                None => f.write_str("disabled"),
                Some(v) => write!(f, "{}", v),
            })
        ));
    }
    ui.end_row();
    diff1 || diff2
}

fn show_flip_margin(
    state: &State,
    ui: &mut Ui,
    m: &HeadState,
    t: &mut Option<HeadState>,
    connector_id: ConnectorId,
) {
    if !effective!(m, t).in_compositor_space {
        return;
    }
    let Some(node) = state.root.outputs.get(&connector_id) else {
        return;
    };
    let Some(margin) = node.flip_margin_ns.get() else {
        return;
    };
    label(
        ui,
        "Flip Margin (ms)",
        format!("{}", margin as f64 / 1_000_000.0),
    );
}
