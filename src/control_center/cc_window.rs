use {
    crate::{
        control_center::{
            CcBehavior, ControlCenterInner, PaneType,
            cc_clients::{ClientCrit, show_client_collapsible},
            cc_criterion::{CcCriterion, CritImpl, CritRegex},
            grid, icon_label, label, read_only_bool,
        },
        criteria::{CritMgrExt, CritUpstreamNode, crit_leaf::CritLeafMatcher},
        egui_adapter::egui_platform::icons::ICON_OPEN_IN_NEW,
        state::State,
        tree::{NodeId, ToplevelData, ToplevelNode, ToplevelType},
        utils::{
            copyhashmap::CopyHashMap,
            event_listener::{EventListener, LazyEventSourceListener},
            static_text::StaticText,
            toplevel_identifier::ToplevelIdentifier,
        },
    },
    ahash::AHashMap,
    egui::{CollapsingHeader, Sense, TextFormat, Ui, Widget, cache::CacheTrait, text::LayoutJob},
    isnt::std_1::primitive::IsntStrExt,
    jay_config::window::{
        ContentType, GAME_CONTENT, NO_CONTENT_TYPE, PHOTO_CONTENT, VIDEO_CONTENT,
    },
    linearize::Linearize,
    std::{
        any::Any,
        mem,
        rc::{Rc, Weak},
    },
};

enum WindowClit {
    Client(CcCriterion<ClientCrit>),
    Title(CritRegex),
    AppId(CritRegex),
    Floating,
    Visible,
    Urgent,
    Fullscreen,
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
    Tag,
    XClass,
    XInstance,
    XRole,
    Workspace,
    ContentTypes,
}

impl Default for WindowClit {
    fn default() -> Self {
        WindowClit::Title(Default::default())
    }
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
            WindowCritTy::Tag => "Tag",
            WindowCritTy::XClass => "X Class",
            WindowCritTy::XInstance => "X Instance",
            WindowCritTy::XRole => "X Role",
            WindowCritTy::Workspace => "Workspace",
            WindowCritTy::ContentTypes => "Content Types",
        }
    }
}

impl CritImpl for WindowClit {
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
            WindowClit::Client(v) => v.show(ui),
            WindowClit::Title(v) => v.show(ui),
            WindowClit::AppId(v) => v.show(ui),
            WindowClit::Floating => false,
            WindowClit::Visible => false,
            WindowClit::Urgent => false,
            WindowClit::Fullscreen => false,
            WindowClit::Tag(v) => v.show(ui),
            WindowClit::XClass(v) => v.show(ui),
            WindowClit::XInstance(v) => v.show(ui),
            WindowClit::XRole(v) => v.show(ui),
            WindowClit::Workspace(v) => v.show(ui),
            WindowClit::ContentTypes(v) => show_content_types(ui, v),
        }
    }

    fn to_crit(&self, state: &Rc<State>) -> Option<Rc<dyn CritUpstreamNode<Self::Target>>> {
        let m = &state.tl_matcher_manager;
        let res = match self {
            WindowClit::Client(v) => m.client(state, &v.to_crit(state)?),
            WindowClit::Title(v) => m.title(v.to_crit()?),
            WindowClit::AppId(v) => m.app_id(v.to_crit()?),
            WindowClit::Floating => m.floating(),
            WindowClit::Visible => m.visible(),
            WindowClit::Urgent => m.urgent(),
            WindowClit::Fullscreen => m.fullscreen(),
            WindowClit::Tag(v) => m.tag(v.to_crit()?),
            WindowClit::XClass(v) => m.class(v.to_crit()?),
            WindowClit::XInstance(v) => m.instance(v.to_crit()?),
            WindowClit::XRole(v) => m.role(v.to_crit()?),
            WindowClit::Workspace(v) => m.workspace(v.to_crit()?),
            WindowClit::ContentTypes(v) => m.content_type(*v),
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
    criterion: CcCriterion<WindowClit>,
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
            if self.criterion.any(|c| matches!(c, WindowClit::Client(_))) {
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
    pub fn create_window_pane(self: &Rc<Self>, window: &Rc<dyn ToplevelNode>) -> WindowPane {
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
        show_window(behavior, ui, &*self.window)
    }
}

pub fn show_window_collapsible(
    behavior: &mut CcBehavior,
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
            show_window(behavior, ui, &**window)
        })
        .fully_closed();
    if closed {
        ensure_listener(ui, behavior, data);
    }
}

pub fn show_window(behavior: &mut CcBehavior<'_>, ui: &mut Ui, window: &dyn ToplevelNode) {
    let data = window.tl_data();
    ensure_listener(ui, behavior, data);
    grid(ui, ("window", data.identifier.get()), |ui| {
        label(ui, "ID", &*data.identifier.get().to_string());
        label(ui, "Title", &*data.title.borrow());
        if let Some(w) = data.workspace.get() {
            label(ui, "Workspace", &w.name);
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
        read_only_bool(ui, "Visible", data.visible.get());
        read_only_bool(ui, "Urgent", data.wants_attention.get());
        read_only_bool(ui, "Fullscreen", data.is_fullscreen.get());
        if let Some(ct) = data.content_type.get() {
            label(ui, "Content Type", ct.text());
        }
    });
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
    listeners: AHashMap<NodeId, WindowPropertyListener>,
}

struct WindowPropertyListener {
    _listener: EventListener<dyn LazyEventSourceListener>,
    generation: u64,
}

impl WindowPropertyListeners {
    fn ensure(&mut self, cc: &Rc<ControlCenterInner>, data: &ToplevelData) {
        let listener = self.listeners.entry(data.node_id).or_insert_with(|| {
            let listener =
                EventListener::new(Rc::downgrade(cc) as Weak<dyn LazyEventSourceListener>);
            listener.attach(data.property_changed_source());
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
