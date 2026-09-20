use crate::cmm::cmm_eotf::Eotf;
use crate::control_center::CcBehavior;
use crate::control_center::ControlCenterInner;
use crate::control_center::GridExt;
use crate::control_center::PaneType;
use crate::control_center::cc_clients::ClientCrit;
use crate::control_center::cc_clients::show_client_collapsible;
use crate::control_center::cc_criterion::CcCriterion;
use crate::control_center::cc_criterion::CritImpl;
use crate::control_center::cc_criterion::CritRegex;
use crate::control_center::grid;
use crate::control_center::grid_label;
use crate::control_center::icon_label;
use crate::control_center::label;
use crate::control_center::read_only_bool;
use crate::criteria::CritMgrExt;
use crate::criteria::CritUpstreamNode;
use crate::criteria::crit_leaf::CritLeafMatcher;
use crate::egui_adapter::egui_platform::icons::ICON_CLOSE;
use crate::egui_adapter::egui_platform::icons::ICON_OPEN_IN_NEW;
use crate::gfx_api::AlphaMode;
use crate::state::State;
use crate::theme::Color;
use crate::theme::ContainerBorders;
use crate::theme::ContainerBordersSetting;
use crate::theme::ToplevelThemeColored;
use crate::theme::ToplevelThemeSized;
use crate::tree::NodeId;
use crate::tree::ToplevelData;
use crate::tree::ToplevelIdentifier;
use crate::tree::ToplevelNode;
use crate::tree::ToplevelNodeBase;
use crate::tree::ToplevelThemeType;
use crate::tree::ToplevelThemeType::ParentTheme;
use crate::tree::ToplevelType;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::bhash::BHashMap;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::event_listener::EventListener;
use crate::utils::lazy_event_source::LazyEventSourceListener;
use crate::utils::reset_immutable::ResetImmutable;
use crate::utils::static_text::StaticText;
use ToplevelThemeType::SelfTheme;
use derivative::Derivative;
use egui::Button;
use egui::Checkbox;
use egui::CollapsingHeader;
use egui::ComboBox;
use egui::DragValue;
use egui::Sense;
use egui::TextEdit;
use egui::TextFormat;
use egui::Ui;
use egui::Widget;
use egui::cache::CacheTrait;
use egui::text::LayoutJob;
use egui::vec2;
use isnt::std_1::primitive::IsntStrExt;
use jay_config::window::ContentType;
use jay_config::window::GAME_CONTENT;
use jay_config::window::NO_CONTENT_TYPE;
use jay_config::window::PHOTO_CONTENT;
use jay_config::window::VIDEO_CONTENT;
use linearize::Linearize;
use linearize::LinearizeExt;
use std::cell::LazyCell;
use std::mem;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;

#[derive(Derivative)]
#[derivative(Default)]
enum WindowCrit {
    Client(CcCriterion<ClientCrit>),
    #[derivative(Default)]
    Title(CritRegex),
    AppId(CritRegex),
    Floating,
    Visible,
    Urgent,
    Fullscreen,
    WorkspaceContainer,
    Tag(CritRegex),
    XClass(CritRegex),
    XInstance(CritRegex),
    XRole(CritRegex),
    Workspace(CritRegex),
    ContentTypes(ContentType),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Linearize)]
enum WindowCritTy {
    Client,
    Title,
    AppId,
    Floating,
    Visible,
    Urgent,
    Fullscreen,
    WorkspaceContainer,
    Tag,
    XClass,
    XInstance,
    XRole,
    Workspace,
    ContentTypes,
}

impl StaticText for WindowCritTy {
    fn text(&self) -> &'static str {
        match self {
            WindowCritTy::Client => "Client",
            WindowCritTy::Title => "Title",
            WindowCritTy::AppId => "App ID",
            WindowCritTy::Floating => "Floating",
            WindowCritTy::Visible => "Visible",
            WindowCritTy::Urgent => "Urgent",
            WindowCritTy::Fullscreen => "Fullscreen",
            WindowCritTy::WorkspaceContainer => "Workspace Container",
            WindowCritTy::Tag => "Tag",
            WindowCritTy::XClass => "X Class",
            WindowCritTy::XInstance => "X Instance",
            WindowCritTy::XRole => "X Role",
            WindowCritTy::Workspace => "Workspace",
            WindowCritTy::ContentTypes => "Content Types",
        }
    }
}

impl CritImpl for WindowCrit {
    type Type = WindowCritTy;
    type Target = ToplevelData;

    fn ty(&self) -> Self::Type {
        macro_rules! map {
            ($($n:ident,)*) => {
                match self {
                    $(
                        Self::$n { .. } => WindowCritTy::$n,
                    )*
                }
            };
        }
        map! {
            Client,
            Title,
            AppId,
            Floating,
            Visible,
            Urgent,
            Fullscreen,
            WorkspaceContainer,
            Tag,
            XClass,
            XInstance,
            XRole,
            Workspace,
            ContentTypes,
        }
    }

    fn from_ty(ty: Self::Type) -> Self {
        match ty {
            WindowCritTy::Client => Self::Client(Default::default()),
            WindowCritTy::Title => Self::Title(Default::default()),
            WindowCritTy::AppId => Self::AppId(Default::default()),
            WindowCritTy::Floating => Self::Floating,
            WindowCritTy::Visible => Self::Visible,
            WindowCritTy::Urgent => Self::Urgent,
            WindowCritTy::Fullscreen => Self::Fullscreen,
            WindowCritTy::WorkspaceContainer => Self::WorkspaceContainer,
            WindowCritTy::Tag => Self::Tag(Default::default()),
            WindowCritTy::XClass => Self::XClass(Default::default()),
            WindowCritTy::XInstance => Self::XInstance(Default::default()),
            WindowCritTy::XRole => Self::XRole(Default::default()),
            WindowCritTy::Workspace => Self::Workspace(Default::default()),
            WindowCritTy::ContentTypes => {
                Self::ContentTypes(PHOTO_CONTENT | VIDEO_CONTENT | GAME_CONTENT)
            }
        }
    }

    fn show(&mut self, ui: &mut Ui) -> bool {
        match self {
            WindowCrit::Client(v) => v.show(ui),
            WindowCrit::Title(v) => v.show(ui),
            WindowCrit::AppId(v) => v.show(ui),
            WindowCrit::Floating => false,
            WindowCrit::Visible => false,
            WindowCrit::Urgent => false,
            WindowCrit::Fullscreen => false,
            WindowCrit::WorkspaceContainer => false,
            WindowCrit::Tag(v) => v.show(ui),
            WindowCrit::XClass(v) => v.show(ui),
            WindowCrit::XInstance(v) => v.show(ui),
            WindowCrit::XRole(v) => v.show(ui),
            WindowCrit::Workspace(v) => v.show(ui),
            WindowCrit::ContentTypes(v) => show_content_types(ui, v),
        }
    }

    fn to_crit(&self, state: &Rc<State>) -> Option<Rc<dyn CritUpstreamNode<Self::Target>>> {
        let m = &state.tl_matcher_manager;
        let res = match self {
            WindowCrit::Client(v) => m.client(state, &v.to_crit(state)?),
            WindowCrit::Title(v) => m.title(v.to_crit()?),
            WindowCrit::AppId(v) => m.app_id(v.to_crit()?),
            WindowCrit::Floating => m.floating(),
            WindowCrit::Visible => m.visible(),
            WindowCrit::Urgent => m.urgent(),
            WindowCrit::Fullscreen => m.fullscreen(),
            WindowCrit::WorkspaceContainer => m.is_workspace_container(),
            WindowCrit::Tag(v) => m.tag(v.to_crit()?),
            WindowCrit::XClass(v) => m.class(v.to_crit()?),
            WindowCrit::XInstance(v) => m.instance(v.to_crit()?),
            WindowCrit::XRole(v) => m.role(v.to_crit()?),
            WindowCrit::Workspace(v) => m.workspace(v.to_crit()?),
            WindowCrit::ContentTypes(v) => m.content_type(*v),
        };
        Some(res)
    }

    fn not(
        state: &State,
        upstream: &Rc<dyn CritUpstreamNode<Self::Target>>,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.tl_matcher_manager.not(upstream)
    }

    fn list(
        state: &State,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
        all: bool,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.tl_matcher_manager.list(upstream, all)
    }

    fn exactly(
        state: &State,
        n: usize,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.tl_matcher_manager.exactly(upstream, n)
    }
}

pub struct WindowSearchPane {
    state: Rc<State>,
    criterion: CcCriterion<WindowCrit>,
    matched: Rc<Matched>,
    leaf: Option<Rc<CritLeafMatcher<ToplevelData>>>,
}

struct Matched {
    slf: Weak<ControlCenterInner>,
    windows: CopyHashMap<ToplevelIdentifier, ()>,
}

impl Matched {
    fn request_frame(&self) {
        if let Some(slf) = self.slf.upgrade() {
            slf.window.request_redraw();
        }
    }
}

impl ControlCenterInner {
    pub fn create_window_search_pane(self: &Rc<Self>) -> WindowSearchPane {
        let mut pane = WindowSearchPane {
            state: self.state.clone(),
            criterion: Default::default(),
            matched: Rc::new(Matched {
                slf: Rc::downgrade(self),
                windows: Default::default(),
            }),
            leaf: Default::default(),
        };
        pane.update_matcher();
        pane
    }
}

impl WindowSearchPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Window Search");
    }

    pub fn show(&mut self, behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        let mut clear = false;
        if self.criterion.show(ui) {
            clear = self.update_matcher();
        }
        ui.separator();
        let mut windows: Vec<_> = self.matched.windows.lock().keys().copied().collect();
        windows.sort();
        for id in windows {
            let Some(window) = self.state.toplevels.get(&id).and_then(|v| v.upgrade()) else {
                continue;
            };
            show_window_collapsible(behavior, ui, &window);
        }
        if clear {
            self.matched.windows.clear();
        }
    }

    fn update_matcher(&mut self) -> bool {
        let mut clear = false;
        let state = &self.state;
        if let Some(new) = self.criterion.to_crit(state) {
            clear = true;
            let matched = self.matched.clone();
            let leaf = state.tl_matcher_manager.leaf(&new, move |data| {
                matched.windows.set(data, ());
                matched.request_frame();
                Box::new({
                    let matched = matched.clone();
                    move || {
                        matched.windows.remove(&data);
                        matched.request_frame();
                    }
                })
            });
            state.tl_matcher_manager.rematch_all(state);
            if self.criterion.any(|c| matches!(c, WindowCrit::Client(_))) {
                state.cl_matcher_manager.rematch_all(state);
            }
            self.leaf = Some(leaf);
        }
        clear
    }
}

pub struct WindowPane {
    window: Rc<dyn ToplevelNode>,
}

impl ControlCenterInner {
    fn create_window_pane(self: &Rc<Self>, window: &Rc<dyn ToplevelNode>) -> WindowPane {
        WindowPane {
            window: window.clone(),
        }
    }
}

impl WindowPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Window");
    }

    pub fn show(&mut self, behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        show_window(behavior, ui, &*self.window);
    }
}

pub fn show_window_collapsible(
    behavior: &mut CcBehavior<'_>,
    ui: &mut Ui,
    window: &Rc<dyn ToplevelNode>,
) {
    let data = window.tl_data();
    let mut layout_job = LayoutJob::default();
    layout_job.append(
        "Window",
        0.0,
        TextFormat {
            color: ui.style().visuals.widgets.inactive.text_color(),
            ..Default::default()
        },
    );
    layout_job.append(
        &data.title.borrow(),
        10.0,
        TextFormat {
            color: ui.style().visuals.widgets.active.text_color(),
            ..Default::default()
        },
    );
    let closed = CollapsingHeader::new(layout_job)
        .id_salt(("window", data.identifier.get()))
        .show(ui, |ui| {
            if icon_label(ICON_OPEN_IN_NEW)
                .sense(Sense::CLICK)
                .ui(ui)
                .clicked()
            {
                behavior.open = Some(PaneType::Window(behavior.cc.create_window_pane(window)));
            }
            show_window(behavior, ui, &**window);
        })
        .fully_closed();
    if closed {
        ensure_listener(ui, behavior, data);
    }
}

fn show_window(behavior: &mut CcBehavior<'_>, ui: &mut Ui, window: &dyn ToplevelNode) {
    let data = window.tl_data();
    ensure_listener(ui, behavior, data);
    grid(ui, ("window", data.identifier.get()), |ui| {
        label(ui, "ID", &*data.identifier.get().to_string());
        label(ui, "Title", &*data.title.borrow());
        if let Some(w) = data.workspace[LiveTL].get() {
            label(ui, "Workspace", &*w.name);
        }
        match &data.kind {
            ToplevelType::Container => {
                label(ui, "Type", "Container");
            }
            ToplevelType::Placeholder(_) => {
                label(ui, "Type", "Placeholder");
            }
            ToplevelType::XdgToplevel(t) => {
                label(ui, "Type", "xdg_toplevel");
                let tag = &*t.tag.borrow();
                if tag.is_not_empty() {
                    label(ui, "Tag", tag);
                }
            }
            ToplevelType::XWindow(t) => {
                label(ui, "Type", "X Window");
                if let Some(class) = &*t.info.class.borrow() {
                    label(ui, "Class", class);
                }
                if let Some(instance) = &*t.info.instance.borrow() {
                    label(ui, "Instance", instance);
                }
                if let Some(role) = &*t.info.role.borrow() {
                    label(ui, "Role", role);
                }
            }
        }
        let app_id = &*data.app_id.borrow();
        if app_id.is_not_empty() {
            label(ui, "App ID", app_id);
        }
        read_only_bool(ui, "Floating", data.parent_is_float.get());
        read_only_bool(ui, "Visible", data.visible[LiveTL].get());
        read_only_bool(ui, "Urgent", data.wants_attention.get());
        read_only_bool(ui, "Fullscreen", data.is_fullscreen[LiveTL].get());
        read_only_bool(
            ui,
            "Workspace Container",
            data.is_root_container[LiveTL].get(),
        );
        if let Some(ct) = data.content_type.get() {
            label(ui, "Content Type", ct.text());
        }
    });
    let theme = &behavior.cc.state.theme;
    let usage = LazyCell::new(|| TlThemeUsage::new(&behavior.cc.state, data));
    for ttt in ToplevelThemeType::variants() {
        let name = match ttt {
            ParentTheme => "Theme",
            SelfTheme if not_matches!(data.kind, ToplevelType::Container) => continue,
            SelfTheme => "Container Theme",
        };
        ui.collapsing(name, |ui| {
            let t = data.theme(ttt);
            if ui
                .add_enabled(t.is_some(), Button::new("Unset All"))
                .clicked()
            {
                data.modify_theme(ttt, |t| t.reset_immutable());
            }
            macro_rules! map {
                ($($path:ident).+) => {
                    t.and_then(|t| t.$($path).+.get())
                };
            }
            let unused = |setting| usage.unused(ttt, setting);
            macro_rules! bool {
                ($ui:expr, $name:expr, $field:ident, $setting:ident $(,)?) => {
                    let v = map!($field);
                    tl_theme_row(
                        $ui,
                        $name,
                        unused(TlThemeSetting::$setting),
                        v.is_some(),
                        || data.modify_theme(ttt, |t| t.$field.set(None)),
                        |ui| {
                            let mut b = v.unwrap_or(theme.$field.get());
                            if Checkbox::without_text(&mut b).ui(ui).changed() {
                                data.modify_theme(ttt, |t| t.$field.set(Some(b)));
                            }
                        },
                    );
                };
            }
            grid(ui, "settings", |ui| {
                bool!(ui, "Show Titles", show_titles, ShowTitles);
                bool!(ui, "Show Window Icons", show_window_icons, ShowWindowIcons);
                bool!(
                    ui,
                    "Window Icons Grayscale",
                    window_icons_grayscale,
                    WindowIconsGrayscale,
                );
                let font = map!(title_font);
                tl_theme_row(
                    ui,
                    "Title Font",
                    unused(TlThemeSetting::TitleFont),
                    font.is_some(),
                    || {
                        data.modify_theme(ttt, |t| t.title_font.set(None));
                    },
                    |ui| {
                        let mut v = font.map(|v| v.to_string()).unwrap_or_default();
                        let res = TextEdit::singleline(&mut v)
                            .clip_text(false)
                            .min_size(vec2(200.0, 0.0))
                            .hint_text(&**theme.title_font())
                            .ui(ui);
                        if res.changed() {
                            data.modify_theme(ttt, |t| {
                                t.title_font
                                    .set(v.is_not_empty().then(|| Rc::new(Arc::from(v))))
                            });
                        }
                    },
                );
                if ttt == SelfTheme {
                    let borders = map!(container_borders);
                    tl_theme_row(
                        ui,
                        "Container Borders",
                        unused(TlThemeSetting::ContainerBorders),
                        borders.is_some(),
                        || data.modify_theme(ttt, |t| t.container_borders.set(None)),
                        |ui| {
                            let v = borders.unwrap_or(theme.container_borders.get());
                            let mut selected = None;
                            ComboBox::from_id_salt("Container Borders")
                                .selected_text(v.text())
                                .show_ui(ui, |ui| {
                                    for s in ContainerBordersSetting::variants() {
                                        if ui.selectable_label(v == s, s.text()).clicked() {
                                            selected = Some(s);
                                        }
                                    }
                                });
                            if let Some(s) = selected {
                                data.modify_theme(ttt, |t| t.container_borders.set(Some(s)));
                            }
                        },
                    );
                }
            });
            ui.collapsing("Sizes", |ui| {
                grid(ui, "Sizes", |ui| {
                    for ts in ToplevelThemeSized::variants() {
                        let v = t.and_then(|t| ts.field(t).get());
                        tl_theme_row(
                            ui,
                            ts.text(),
                            unused(TlThemeSetting::Size(ts)),
                            v.is_some(),
                            || data.modify_theme(ttt, |t| ts.field(t).set(None)),
                            |ui| {
                                let mut i = v.unwrap_or_else(|| ts.theme().field(theme).val.get());
                                if DragValue::new(&mut i)
                                    .range(ts.min()..=ts.max())
                                    .speed(1.0)
                                    .ui(ui)
                                    .changed()
                                {
                                    data.modify_theme(ttt, |t| ts.field(t).set(Some(i)));
                                }
                            },
                        );
                    }
                });
            });
            ui.collapsing("Colors", |ui| {
                grid(ui, "Colors", |ui| {
                    for tc in ToplevelThemeColored::variants() {
                        let c = t.and_then(|t| tc.field(t).get());
                        tl_theme_row(
                            ui,
                            tc.text(),
                            unused(TlThemeSetting::Color(tc)),
                            c.is_some(),
                            || data.modify_theme(ttt, |t| tc.field(t).set(None)),
                            |ui| {
                                let mut v = c
                                    .unwrap_or_else(|| tc.theme().field(theme).val.get())
                                    .to_array(Eotf::Linear);
                                if ui.color_edit_button_rgba_premultiplied(&mut v).changed() {
                                    let [r, g, b, a] = v;
                                    let c = Color::new(
                                        Eotf::Linear,
                                        AlphaMode::PremultipliedOptical,
                                        r,
                                        g,
                                        b,
                                        a,
                                    );
                                    data.modify_theme(ttt, |t| tc.field(t).set(Some(c)));
                                }
                            },
                        );
                    }
                });
            });
        });
    }
    if let Some(client) = &data.client {
        show_client_collapsible(behavior, ui, client);
    }
}

fn ensure_listener(ui: &mut Ui, behavior: &CcBehavior<'_>, data: &ToplevelData) {
    ui.memory_mut(|m| {
        m.caches
            .cache::<WindowPropertyListeners>()
            .ensure(behavior.cc, data);
    });
}

#[derive(Default)]
struct WindowPropertyListeners {
    generation: u64,
    listeners: BHashMap<NodeId, WindowPropertyListener>,
}

struct WindowPropertyListener {
    _listener: EventListener<dyn LazyEventSourceListener>,
    generation: u64,
}

impl WindowPropertyListeners {
    fn ensure(&mut self, cc: &Rc<ControlCenterInner>, data: &ToplevelData) {
        let listener = self.listeners.entry(data.node_id).or_insert_with(|| {
            let listener = EventListener::attached(
                Rc::downgrade(cc) as Weak<dyn LazyEventSourceListener>,
                data.property_changed_source(),
            );
            WindowPropertyListener {
                _listener: listener,
                generation: 0,
            }
        });
        listener.generation = self.generation;
    }
}

unsafe impl Sync for WindowPropertyListeners {}
unsafe impl Send for WindowPropertyListeners {}

impl CacheTrait for WindowPropertyListeners {
    fn update(&mut self) {
        self.listeners
            .retain(|_, m| m.generation == self.generation);
        self.generation += 1;
    }

    fn len(&self) -> usize {
        self.listeners.len()
    }
}

fn show_content_types(ui: &mut Ui, ct: &mut ContentType) -> bool {
    let mut v = *ct;
    let mut photo = (v & PHOTO_CONTENT).0 != 0;
    let mut video = (v & VIDEO_CONTENT).0 != 0;
    let mut game = (v & GAME_CONTENT).0 != 0;
    ui.checkbox(&mut photo, "Photo");
    ui.checkbox(&mut video, "Video");
    ui.checkbox(&mut game, "Game");
    v = NO_CONTENT_TYPE;
    if photo {
        v |= PHOTO_CONTENT;
    }
    if video {
        v |= VIDEO_CONTENT;
    }
    if game {
        v |= GAME_CONTENT;
    }
    mem::replace(ct, v) != v
}

#[derive(Copy, Clone)]
enum TlThemeSetting {
    ShowTitles,
    ShowWindowIcons,
    WindowIconsGrayscale,
    TitleFont,
    ContainerBorders,
    Size(ToplevelThemeSized),
    Color(ToplevelThemeColored),
}

struct TlThemeUsage {
    parent: TlParentUsage,
    container: Option<TlContainerUsage>,
}

enum TlParentUsage {
    Floating {
        show_titles: bool,
        titles_visible: bool,
        borders_visible: bool,
        icons_visible: bool,
    },
    Tiled {
        titles_visible: bool,
        icons_visible: bool,
        focused_border_visible: bool,
        focused_border_set: bool,
    },
    Other,
}

struct TlContainerUsage {
    show_titles: bool,
    titles_visible: bool,
    has_borders: bool,
    borders_visible: bool,
    focused_border_visible: bool,
}

const NOT_FLOATING_OR_TILED: &str = "The window is neither floating nor tiled.";
const TITLES_HIDDEN: &str = "Titles are hidden.";
const TITLES_NOT_VISIBLE: &str = "Titles are not visible.";
const ICONS_NOT_VISIBLE: &str = "Window icons are not visible.";
const BORDERS_NOT_VISIBLE: &str = "No borders are visible.";
const NO_BORDERS: &str = "The container has no borders because it has only one child and \
    does not use full borders.";
const NOT_FLOATING: &str = "This setting is not used for floating windows.";
const NOT_TILED: &str = "This setting is not used for tiled windows. \
    The value is taken from the container theme of the parent container.";
const FOCUSED_BORDER_NOT_VISIBLE: &str = "Focused borders are only visible if the parent \
    container uses full borders and has a border width larger than 0.";
const BORDER_NOT_VISIBLE_TILED: &str = "For tiled windows, this color is only used as the \
    default of the focused border color. Focused borders are only visible if the parent \
    container uses full borders and has a border width larger than 0.";
const BORDER_FOCUSED_BORDER_SET: &str = "For tiled windows, this color is only used as the \
    default of the focused border color, but the focused border color is set.";
const CONTAINER_FOCUSED_BORDER_NOT_VISIBLE: &str = "Focused borders are only visible if the \
    container uses full borders and has a border width larger than 0.";

impl TlThemeUsage {
    fn new(state: &State, data: &ToplevelData) -> Self {
        let container = data
            .slf
            .upgrade()
            .and_then(|tl| tl.node_into_container())
            .map(|c| {
                let ns = &c.node_state[LiveTL];
                let theme = &ns.theme;
                let bw = theme.sizes.border_width.get();
                let full = c.container_borders(LiveTL) == ContainerBorders::Full;
                let has_borders = full || ns.num_children.get() > 1;
                TlContainerUsage {
                    show_titles: theme.show_titles.get(),
                    titles_visible: theme.sizes.title_height.get() > 0,
                    has_borders,
                    borders_visible: bw > 0 && has_borders,
                    focused_border_visible: bw > 0 && full,
                }
            });
        let parent = 'parent: {
            if data.is_fullscreen[LiveTL].get() {
                break 'parent TlParentUsage::Other;
            }
            let Some(parent) = data.parent.get() else {
                break 'parent TlParentUsage::Other;
            };
            if let Some(float) = parent.clone().node_into_float() {
                let theme = &float.node_state[LiveTL].theme;
                break 'parent TlParentUsage::Floating {
                    show_titles: theme.sizes.title_underline_height.get() > 0,
                    titles_visible: theme.sizes.title_height.get() > 0,
                    borders_visible: theme.sizes.border_width.get() > 0,
                    icons_visible: theme.sizes.title_icon_size.get() > 0,
                };
            }
            let Some(container) = parent.node_into_container() else {
                break 'parent TlParentUsage::Other;
            };
            let Some(child) = container.get_child_node(data.node_id) else {
                break 'parent TlParentUsage::Other;
            };
            let ptheme = &container.node_state[LiveTL].theme;
            let focused_border_set = state.theme.colors.focused_border.get_opt().is_some()
                || [
                    container.tl_data().theme(SelfTheme),
                    data.theme(ParentTheme),
                ]
                .into_iter()
                .flatten()
                .any(|t| t.colors.focused_border.get().is_some());
            TlParentUsage::Tiled {
                titles_visible: ptheme.sizes.title_height.get() > 0,
                icons_visible: child.node_state[LiveTL].theme.sizes.title_icon_size.get() > 0,
                focused_border_visible: ptheme.sizes.border_width.get() > 0
                    && container.container_borders(LiveTL) == ContainerBorders::Full,
                focused_border_set,
            }
        };
        Self { parent, container }
    }

    fn unused(&self, ttt: ToplevelThemeType, setting: TlThemeSetting) -> Option<&'static str> {
        use TlThemeSetting::*;
        use ToplevelThemeColored::*;
        use ToplevelThemeSized::*;
        match ttt {
            ParentTheme => self.parent_unused(setting),
            SelfTheme => {
                let c = self.container.as_ref()?;
                let titles = || (!c.titles_visible).then_some(TITLES_NOT_VISIBLE);
                match setting {
                    ShowTitles | ContainerBorders => None,
                    Size(title_height) | Color(separator) => {
                        (!c.show_titles).then_some(TITLES_HIDDEN)
                    }
                    Size(border_width) => (!c.has_borders).then_some(NO_BORDERS),
                    Color(border) => (!c.borders_visible).then_some(BORDERS_NOT_VISIBLE),
                    Color(focused_border) => {
                        (!c.focused_border_visible).then_some(CONTAINER_FOCUSED_BORDER_NOT_VISIBLE)
                    }
                    ShowWindowIcons | WindowIconsGrayscale | TitleFont | Color(_) => titles(),
                }
            }
        }
    }

    fn parent_unused(&self, setting: TlThemeSetting) -> Option<&'static str> {
        use TlThemeSetting::*;
        use ToplevelThemeColored::*;
        use ToplevelThemeSized::*;
        match self.parent {
            TlParentUsage::Other => Some(NOT_FLOATING_OR_TILED),
            TlParentUsage::Floating {
                show_titles,
                titles_visible,
                borders_visible,
                icons_visible,
            } => {
                let titles = || (!titles_visible).then_some(TITLES_NOT_VISIBLE);
                match setting {
                    ShowTitles | ContainerBorders => None,
                    Size(title_height) | Color(separator) => {
                        (!show_titles).then_some(TITLES_HIDDEN)
                    }
                    Size(border_width) => None,
                    Color(border | focused_border) => {
                        (!borders_visible).then_some(BORDERS_NOT_VISIBLE)
                    }
                    Color(focused_inactive_title_background | focused_inactive_title_text) => {
                        Some(NOT_FLOATING)
                    }
                    WindowIconsGrayscale => {
                        titles().or((!icons_visible).then_some(ICONS_NOT_VISIBLE))
                    }
                    ShowWindowIcons | TitleFont | Color(_) => titles(),
                }
            }
            TlParentUsage::Tiled {
                titles_visible,
                icons_visible,
                focused_border_visible,
                focused_border_set,
            } => {
                let titles = || (!titles_visible).then_some(TITLES_NOT_VISIBLE);
                match setting {
                    ShowTitles | ContainerBorders | Size(_) | Color(separator) => Some(NOT_TILED),
                    Color(focused_border) => {
                        (!focused_border_visible).then_some(FOCUSED_BORDER_NOT_VISIBLE)
                    }
                    Color(border) => {
                        if !focused_border_visible {
                            Some(BORDER_NOT_VISIBLE_TILED)
                        } else if focused_border_set {
                            Some(BORDER_FOCUSED_BORDER_SET)
                        } else {
                            None
                        }
                    }
                    WindowIconsGrayscale => {
                        titles().or((!icons_visible).then_some(ICONS_NOT_VISIBLE))
                    }
                    ShowWindowIcons | TitleFont | Color(_) => titles(),
                }
            }
        }
    }
}

fn tl_theme_row(
    ui: &mut Ui,
    name: &str,
    unused: Option<&str>,
    is_set: bool,
    unset: impl FnOnce(),
    add_contents: impl FnOnce(&mut Ui),
) {
    let ui = &mut *ui.row();
    grid_label(ui, name);
    ui.scope(|ui| {
        if !is_set {
            ui.multiply_opacity(0.5);
        }
        add_contents(ui);
    });
    if is_set {
        if ui.button(ICON_CLOSE).on_hover_text("Unset").clicked() {
            unset();
        }
    } else {
        ui.weak("Unset").on_hover_text(
            "This setting is not set. The displayed value is taken from the global theme.",
        );
    }
    if let Some(reason) = unused {
        ui.weak("Unused").on_hover_text(reason);
    }
}
