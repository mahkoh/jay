use {
    crate::{
        control_center::cc_compositor::CompositorPane,
        egui_adapter::egui_platform::{EggError, EggWindow, EggWindowOwner},
        macros::Bitflag,
        state::State,
        utils::{asyncevent::AsyncEvent, copyhashmap::CopyHashMap, numcell::NumCell},
    },
    egui::{
        Align, CentralPanel, Checkbox, Color32, ComboBox, Context, CursorIcon, DragValue, Frame,
        Grid, InnerResponse, Label, Layout, Response, Rgba, RichText, ScrollArea, Sense, SidePanel,
        Stroke, TextBuffer, TextEdit, Ui, UiBuilder, Visuals, Widget, WidgetText, emath::Numeric,
        vec2,
    },
    egui_material_icons::icons::{ICON_CLOSE, ICON_DRAG_INDICATOR, ICON_INFO},
    egui_tiles::{ResizeState, TabState, Tile, TileId, Tiles, Tree},
    linearize::{Linearize, LinearizeExt},
    std::{
        cell::RefCell,
        hash::Hash,
        mem,
        ops::{Deref, DerefMut, RangeInclusive},
        rc::Rc,
    },
    thiserror::Error,
};

mod cc_compositor;
mod cc_sidebar;

#[derive(Debug, Error)]
pub enum ControlCenterError {
    #[error("Could not get the egg context")]
    GetEggContext(#[source] EggError),
}

linear_ids!(ControlCenterIds, ControlCenterId, u64);

pub async fn redraw_control_centers(state: Rc<State>) {
    let cc = &state.control_centers;
    loop {
        cc.redraw.triggered().await;
        let interests = cc.change.take();
        for cc in cc.control_centers.lock().values() {
            if cc.inner.interests.interests.get().intersects(interests) {
                cc.inner.window.request_redraw();
            }
        }
    }
}

#[derive(Default)]
pub struct ControlCenters {
    ids: ControlCenterIds,
    change: NumCell<ControlCenterInterest>,
    redraw: AsyncEvent,
    control_centers: CopyHashMap<ControlCenterId, Rc<ControlCenter>>,
}

bitflags! {
    ControlCenterInterest: u32;
        CCI_COMPOSITOR,
}

pub struct ControlCenter {
    inner: Rc<ControlCenterInner>,
}

struct ControlCenterInner {
    id: ControlCenterId,
    state: Rc<State>,
    tree: RefCell<Settings>,
    window: Rc<EggWindow>,
    next_pane_id: NumCell<u64>,
    interests: Rc<Interests>,
}

#[derive(Default)]
struct Interests {
    interests: NumCell<ControlCenterInterest>,
    interests_array: [NumCell<u64>; <ControlCenterInterest as Bitflag>::Type::BITS as usize],
}

struct Settings {
    tree: Tree<Pane>,
}

struct Pane {
    id: u64,
    ps: PaneState,
    own_interests: ControlCenterInterest,
    cc_interests: Rc<Interests>,
    ty: PaneType,
}

struct PaneState {
    errors: Vec<String>,
}

enum PaneType {
    Compositor(CompositorPane),
}

struct CcBehavior<'a> {
    #[expect(dead_code)]
    cc: &'a Rc<ControlCenterInner>,
    close: Option<TileId>,
    open: Option<PaneType>,
}

impl ControlCenters {
    pub fn clear(&self) {
        self.control_centers.clear();
    }
}

impl Pane {
    fn title(&self, res: &mut String) {
        match &self.ty {
            PaneType::Compositor(v) => v.title(res),
        }
    }

    fn show(&mut self, _behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        match &mut self.ty {
            PaneType::Compositor(p) => p.show(ui),
        }
    }
}

impl PaneType {
    fn interest(&self) -> ControlCenterInterest {
        match self {
            PaneType::Compositor(_) => CCI_COMPOSITOR,
        }
    }
}

impl egui_tiles::Behavior<Pane> for CcBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        let mut drag = false;
        Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                drag = ui
                    .add(icon_label(ICON_DRAG_INDICATOR).sense(Sense::drag()))
                    .total_drag_delta()
                    .map(|d| d.length() >= 5.0)
                    .unwrap_or(false);
                let mut title = String::new();
                pane.title(&mut title);
                if ui
                    .add(icon_label(&title).sense(Sense::click()))
                    .middle_clicked()
                {
                    self.close = Some(tile_id);
                }
                if ui
                    .add(icon_label(ICON_CLOSE).sense(Sense::click()))
                    .clicked()
                {
                    self.close = Some(tile_id);
                }
            });
            ui.separator();
            show_errors(ui, &mut pane.ps);
            ui.scope_builder(UiBuilder::new().id(("pane", pane.id)), |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.allocate_space(vec2(ui.available_width(), 0.0));
                    pane.show(self, ui);
                });
            });
        });
        if drag {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }

    fn tab_title_for_pane(&mut self, _pane: &Pane) -> WidgetText {
        "".into()
    }

    fn tab_hover_cursor_icon(&self) -> CursorIcon {
        CursorIcon::Default
    }

    fn tab_title_for_tile(&mut self, tiles: &Tiles<Pane>, tile_id: TileId) -> WidgetText {
        fn add_title(tiles: &Tiles<Pane>, res: &mut String, first: &mut bool, tile_id: TileId) {
            if !mem::take(first) {
                res.push_str("/");
            }
            let Some(tile) = tiles.get(tile_id) else {
                res.push_str("MISSING TILE");
                return;
            };
            match tile {
                Tile::Pane(p) => p.title(res),
                Tile::Container(c) => {
                    let mut first = true;
                    for &tile_id in c.children() {
                        add_title(tiles, res, &mut first, tile_id);
                    }
                }
            }
        }
        let mut res = String::new();
        let mut first = true;
        add_title(tiles, &mut res, &mut first, tile_id);
        res.into()
    }

    fn on_tab_button(
        &mut self,
        _tiles: &Tiles<Pane>,
        tile_id: TileId,
        button_response: Response,
    ) -> Response {
        if button_response.middle_clicked() {
            self.close = Some(tile_id);
        }
        button_response
    }

    fn resize_stroke(&self, style: &egui::Style, resize_state: ResizeState) -> Stroke {
        match resize_state {
            ResizeState::Idle => style.visuals.widgets.noninteractive.bg_stroke,
            ResizeState::Hovering => style.visuals.widgets.hovered.fg_stroke,
            ResizeState::Dragging => style.visuals.widgets.active.fg_stroke,
        }
    }

    fn tab_bar_color(&self, visuals: &Visuals) -> Color32 {
        (Rgba::from(visuals.panel_fill) * Rgba::from_gray(0.8)).into()
    }

    fn tab_bg_color(
        &self,
        visuals: &Visuals,
        _tiles: &Tiles<Pane>,
        _tile_id: TileId,
        state: &TabState,
    ) -> Color32 {
        match state.active {
            true => visuals.panel_fill,
            false => self.tab_bar_color(visuals),
        }
    }
}

impl EggWindowOwner for ControlCenterInner {
    fn close(&self) {
        self.close();
    }

    fn render(self: Rc<Self>, ctx: &Context) {
        let settings = &mut *self.tree.borrow_mut();
        SidePanel::left("sidebar").show(ctx, |ui| self.show_sidebar(&mut settings.tree, ui));
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ctx.style())
                    .outer_margin(0.0)
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                let tree = &mut settings.tree;
                let mut behavior = CcBehavior {
                    cc: &self,
                    close: Default::default(),
                    open: Default::default(),
                };
                tree.ui(&mut behavior, ui);
                if let Some(close) = behavior.close {
                    tree.set_visible(close, false);
                    tree.remove_recursively(close);
                }
                if let Some(ty) = behavior.open {
                    self.open(tree, ty);
                }
            });
    }
}

impl State {
    pub fn open_control_center(self: &Rc<Self>) -> Result<Rc<ControlCenter>, ControlCenterError> {
        let ctx = self
            .get_egg_context()
            .map_err(ControlCenterError::GetEggContext)?;
        let window = ctx.create_window("Control Center");
        let cc = Rc::new(ControlCenter {
            inner: Rc::new(ControlCenterInner {
                id: self.control_centers.ids.next(),
                window,
                state: self.clone(),
                tree: RefCell::new(Settings {
                    tree: Tree::new_tabs("abcd", vec![]),
                }),
                next_pane_id: Default::default(),
                interests: Default::default(),
            }),
        });
        cc.inner.window.set_owner(Some(cc.inner.clone()));
        self.control_centers
            .control_centers
            .set(cc.inner.id, cc.clone());
        Ok(cc)
    }

    pub fn trigger_cci(&self, cci: ControlCenterInterest) {
        self.control_centers.change.or_assign(cci);
        self.control_centers.redraw.trigger();
    }
}

impl ControlCenterInner {
    fn close(&self) {
        self.window.set_owner(None);
        self.tree.borrow_mut().tree = Tree::empty("");
        self.state.control_centers.control_centers.remove(&self.id);
    }
}

impl Drop for ControlCenter {
    fn drop(&mut self) {
        self.inner.close();
    }
}

impl ControlCenterInner {
    fn create_pane(&self, ty: PaneType) -> Pane {
        let pane = Pane {
            id: self.next_pane_id.fetch_add(1),
            ps: PaneState {
                errors: Default::default(),
            },
            own_interests: ty.interest(),
            cc_interests: self.interests.clone(),
            ty,
        };
        let own = pane.own_interests;
        for (idx, v) in pane.cc_interests.interests_array.iter().enumerate() {
            let interest = ControlCenterInterest(1 << idx);
            if own.intersects(interest) && v.fetch_add(1) == 0 {
                pane.cc_interests.interests.or_assign(interest);
            }
        }
        pane
    }

    fn open(&self, tree: &mut Tree<Pane>, ty: PaneType) {
        let pane = self.create_pane(ty);
        let id = tree.tiles.insert_pane(pane);
        if let Some(root) = tree.root
            && let Some(tile) = tree.tiles.get_mut(root)
        {
            match tile {
                Tile::Container(c) => {
                    c.add_child(id);
                }
                Tile::Pane(_) => {
                    let root = tree.tiles.insert_tab_tile(vec![root, id]);
                    tree.root = Some(root);
                }
            }
        } else {
            tree.root = Some(id);
        }
        tree.make_active(|t, _| t == id);
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let own = self.own_interests;
        for (idx, v) in self.cc_interests.interests_array.iter().enumerate() {
            let interest = ControlCenterInterest(1 << idx);
            if own.intersects(interest) && v.fetch_sub(1) == 1 {
                self.cc_interests.interests.and_assign(!interest);
            }
        }
    }
}

fn icon_label(icon: &str) -> Label {
    Label::new(icon).selectable(false)
}

#[expect(dead_code)]
fn grid_label(ui: &mut Ui, label: &str) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(label);
    });
}

fn grid_label_ui<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    ui.with_layout(Layout::right_to_left(Align::Center), add_contents)
}

#[expect(dead_code)]
fn tip(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    icon_label(ICON_INFO).ui(ui).on_hover_ui(add_contents);
}

#[expect(dead_code)]
fn text_edit(ui: &mut Ui, v: &mut dyn TextBuffer) -> Response {
    TextEdit::singleline(v)
        .clip_text(false)
        .min_size(vec2(200.0, 0.0))
        .ui(ui)
}

fn show_errors(ui: &mut Ui, pane: &mut PaneState) {
    if pane.errors.is_empty() {
        return;
    }
    let mut to_remove = None;
    for (idx, e) in pane.errors.iter().enumerate() {
        ui.horizontal(|ui| {
            Frame::new().inner_margin(5.0).show(ui, |ui| {
                if ui.button(ICON_CLOSE).clicked() {
                    to_remove = Some(idx);
                }
                ui.label(
                    RichText::new("Error:")
                        .strong()
                        .color(ui.style().visuals.error_fg_color),
                );
                ui.add(Label::new(e).wrap());
            });
        });
    }
    if let Some(idx) = to_remove {
        pane.errors.remove(idx);
    }
    ui.separator();
}

fn grid<R>(
    ui: &mut Ui,
    id_salt: impl Hash,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let mut spacing = ui.spacing().item_spacing;
    spacing.x *= 3.0;
    Grid::new(id_salt).spacing(spacing).show(ui, add_contents)
}

fn row<R>(ui: &mut Ui, name: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    row_ui(ui, name, |_| (), add_contents)
}

fn row_ui<R, S>(
    ui: &mut Ui,
    name: &str,
    label: impl FnOnce(&mut Ui) -> S,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let ui = &mut *ui.row();
    grid_label_ui(ui, |ui| {
        ui.label(name);
        label(ui);
    });
    add_contents(ui)
}

fn bool(ui: &mut Ui, name: &str, old: bool, set: impl FnOnce(bool)) {
    bool_ui(ui, name, |_| (), old, set);
}

fn bool_ui<R>(
    ui: &mut Ui,
    name: &str,
    label: impl FnOnce(&mut Ui) -> R,
    mut v: bool,
    set: impl FnOnce(bool),
) {
    row_ui(ui, name, label, |ui| {
        if Checkbox::without_text(&mut v).ui(ui).changed() {
            set(v);
        }
    });
}

#[expect(dead_code)]
fn read_only_bool(ui: &mut Ui, name: &str, old: bool) {
    read_only_bool_ui(ui, name, |_| (), old);
}

fn read_only_bool_ui<R>(ui: &mut Ui, name: &str, label: impl FnOnce(&mut Ui) -> R, mut v: bool) {
    row_ui(ui, name, label, |ui| {
        ui.add_enabled_ui(false, |ui| Checkbox::without_text(&mut v).ui(ui));
    });
}

#[expect(dead_code)]
fn combo_box<T>(ui: &mut Ui, name: &str, old: T, set: impl FnOnce(T))
where
    T: EnumText + Linearize + PartialEq + Copy,
{
    combo_box_ui(ui, name, |_| (), old, set);
}

fn combo_box_ui<R, T>(
    ui: &mut Ui,
    name: &str,
    label: impl FnOnce(&mut Ui) -> R,
    mut v: T,
    set: impl FnOnce(T),
) where
    T: EnumText + Linearize + PartialEq + Copy,
{
    row_ui(ui, name, label, |ui| {
        let old = v;
        ComboBox::from_id_salt(name)
            .selected_text(v.text())
            .show_ui(ui, |ui| {
                for s in T::variants() {
                    ui.selectable_value(&mut v, s, s.text());
                }
            });
        if old != v {
            set(v);
        }
    });
}

#[expect(dead_code)]
fn drag_value<N>(
    ui: &mut Ui,
    name: &str,
    old: N,
    range: RangeInclusive<N>,
    speed: f64,
    set: impl FnOnce(N),
) where
    N: Numeric,
{
    drag_value_ui(ui, name, |_| (), old, range, speed, set);
}

fn drag_value_ui<R, N>(
    ui: &mut Ui,
    name: &str,
    label: impl FnOnce(&mut Ui) -> R,
    mut v: N,
    range: RangeInclusive<N>,
    speed: f64,
    set: impl FnOnce(N),
) where
    N: Numeric,
{
    row_ui(ui, name, label, |ui| {
        if DragValue::new(&mut v)
            .range(range)
            .speed(speed)
            .ui(ui)
            .changed()
        {
            set(v);
        }
    });
}

fn label(ui: &mut Ui, name: &str, text: impl Into<WidgetText>) {
    row(ui, name, |ui| ui.label(text));
}

pub trait EnumText {
    fn text(self) -> &'static str;
}

trait GridExt {
    fn row(&mut self) -> impl DerefMut<Target = Ui>;
}

impl GridExt for Ui {
    fn row(&mut self) -> impl DerefMut<Target = Ui> {
        GridRow { ui: self }
    }
}

struct GridRow<'a> {
    ui: &'a mut Ui,
}

impl Deref for GridRow<'_> {
    type Target = Ui;

    fn deref(&self) -> &Self::Target {
        self.ui
    }
}

impl DerefMut for GridRow<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.ui
    }
}

impl Drop for GridRow<'_> {
    fn drop(&mut self) {
        self.end_row();
    }
}
