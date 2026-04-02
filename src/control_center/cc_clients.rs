use {
    crate::{
        client::{Client, ClientId},
        control_center::{
            CcBehavior, ControlCenterInner, PaneType,
            cc_criterion::{CcCriterion, CritImpl, CritRegex},
            cc_window::show_window_collapsible,
            grid, icon_label, label, read_only_bool,
        },
        criteria::{CritMgrExt, CritUpstreamNode, crit_leaf::CritLeafMatcher},
        egui_adapter::egui_platform::icons::ICON_OPEN_IN_NEW,
        state::State,
        tree::{ToplevelData, ToplevelIdentifier},
        utils::{copyhashmap::CopyHashMap, static_text::StaticText},
    },
    ahash::AHashMap,
    egui::{
        CollapsingHeader, DragValue, Sense, TextFormat, Ui, Widget, cache::CacheTrait,
        text::LayoutJob,
    },
    linearize::Linearize,
    std::rc::{Rc, Weak},
};

pub enum ClientCrit {
    SandboxEngine(CritRegex),
    SandboxAppId(CritRegex),
    SandboxInstanceId(CritRegex),
    Sandboxed,
    Uid(i32),
    Pid(i32),
    IsXwayland,
    Comm(CritRegex),
    Exe(CritRegex),
    Tag(CritRegex),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Linearize)]
pub enum ClientCritTy {
    SandboxEngine,
    SandboxAppId,
    SandboxInstanceId,
    Sandboxed,
    Uid,
    Pid,
    IsXwayland,
    Comm,
    Exe,
    Tag,
}

impl Default for ClientCrit {
    fn default() -> Self {
        ClientCrit::Comm(Default::default())
    }
}

impl StaticText for ClientCritTy {
    fn text(&self) -> &'static str {
        match self {
            ClientCritTy::SandboxEngine => "Sandbox Engine",
            ClientCritTy::SandboxAppId => "Sandbox App ID",
            ClientCritTy::SandboxInstanceId => "Sandbox Instance ID",
            ClientCritTy::Sandboxed => "Sandboxed",
            ClientCritTy::Uid => "UID",
            ClientCritTy::Pid => "PID",
            ClientCritTy::IsXwayland => "Is Xwayland",
            ClientCritTy::Comm => "Comm",
            ClientCritTy::Exe => "Exe",
            ClientCritTy::Tag => "Tag",
        }
    }
}

impl CritImpl for ClientCrit {
    type Type = ClientCritTy;
    type Target = Rc<Client>;

    fn ty(&self) -> Self::Type {
        macro_rules! map {
            ($($n:ident,)*) => {
                match self {
                    $(
                        Self::$n { .. } => ClientCritTy::$n,
                    )*
                }
            };
        }
        map! {
            SandboxEngine,
            SandboxAppId,
            SandboxInstanceId,
            Sandboxed,
            Uid,
            Pid,
            IsXwayland,
            Comm,
            Exe,
            Tag,
        }
    }

    fn from_ty(ty: Self::Type) -> Self {
        match ty {
            ClientCritTy::SandboxEngine => Self::SandboxEngine(Default::default()),
            ClientCritTy::SandboxAppId => Self::SandboxAppId(Default::default()),
            ClientCritTy::SandboxInstanceId => Self::SandboxInstanceId(Default::default()),
            ClientCritTy::Sandboxed => Self::Sandboxed,
            ClientCritTy::Uid => Self::Uid(Default::default()),
            ClientCritTy::Pid => Self::Pid(Default::default()),
            ClientCritTy::IsXwayland => Self::IsXwayland,
            ClientCritTy::Comm => Self::Comm(Default::default()),
            ClientCritTy::Exe => Self::Exe(Default::default()),
            ClientCritTy::Tag => Self::Tag(Default::default()),
        }
    }

    fn show(&mut self, ui: &mut Ui) -> bool {
        match self {
            ClientCrit::SandboxEngine(v) => v.show(ui),
            ClientCrit::SandboxAppId(v) => v.show(ui),
            ClientCrit::SandboxInstanceId(v) => v.show(ui),
            ClientCrit::Sandboxed => false,
            ClientCrit::Uid(v) => DragValue::new(v).ui(ui).changed(),
            ClientCrit::Pid(v) => DragValue::new(v).ui(ui).changed(),
            ClientCrit::IsXwayland => false,
            ClientCrit::Comm(v) => v.show(ui),
            ClientCrit::Exe(v) => v.show(ui),
            ClientCrit::Tag(v) => v.show(ui),
        }
    }

    fn to_crit(&self, state: &Rc<State>) -> Option<Rc<dyn CritUpstreamNode<Self::Target>>> {
        let m = &state.cl_matcher_manager;
        let res = match self {
            ClientCrit::SandboxEngine(v) => m.sandbox_engine(v.to_crit()?),
            ClientCrit::SandboxAppId(v) => m.sandbox_app_id(v.to_crit()?),
            ClientCrit::SandboxInstanceId(v) => m.sandbox_instance_id(v.to_crit()?),
            ClientCrit::Sandboxed => m.sandboxed(),
            ClientCrit::Uid(v) => m.uid(*v),
            ClientCrit::Pid(v) => m.pid(*v),
            ClientCrit::IsXwayland => m.is_xwayland(),
            ClientCrit::Comm(v) => m.comm(v.to_crit()?),
            ClientCrit::Exe(v) => m.exe(v.to_crit()?),
            ClientCrit::Tag(v) => m.tag(v.to_crit()?),
        };
        Some(res)
    }

    fn not(
        state: &State,
        upstream: &Rc<dyn CritUpstreamNode<Self::Target>>,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.cl_matcher_manager.not(upstream)
    }

    fn list(
        state: &State,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
        all: bool,
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.cl_matcher_manager.list(upstream, all)
    }

    fn exactly(
        state: &State,
        n: usize,
        upstream: &[Rc<dyn CritUpstreamNode<Self::Target>>],
    ) -> Rc<dyn CritUpstreamNode<Self::Target>> {
        state.cl_matcher_manager.exactly(upstream, n)
    }
}

pub struct ClientsPane {
    state: Rc<State>,
    filter: bool,
    criterion: CcCriterion<ClientCrit>,
    matched: Rc<Matched>,
    leaf: Option<Rc<CritLeafMatcher<Rc<Client>>>>,
}

struct Matched {
    slf: Weak<ControlCenterInner>,
    clients: CopyHashMap<ClientId, ()>,
}

impl Matched {
    fn request_frame(&self) {
        if let Some(slf) = self.slf.upgrade() {
            slf.window.request_redraw();
        }
    }
}

impl ControlCenterInner {
    pub fn create_clients_pane(self: &Rc<Self>) -> ClientsPane {
        let mut pane = ClientsPane {
            state: self.state.clone(),
            filter: false,
            criterion: Default::default(),
            matched: Rc::new(Matched {
                slf: Rc::downgrade(self),
                clients: Default::default(),
            }),
            leaf: Default::default(),
        };
        pane.update_matcher();
        pane
    }
}

impl ClientsPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Clients");
    }

    pub fn show(&mut self, behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        if ui.checkbox(&mut self.filter, "Filter").changed() && !self.filter {
            self.criterion = Default::default();
            self.update_matcher();
        }
        let mut clear_clients = false;
        if self.filter && self.criterion.show(ui) {
            clear_clients = self.update_matcher();
        }
        ui.separator();
        let mut clients: Vec<_> = self.matched.clients.lock().keys().copied().collect();
        clients.sort();
        for id in clients {
            let Ok(client) = self.state.clients.get(id) else {
                continue;
            };
            show_client_collapsible(behavior, ui, &client);
        }
        if clear_clients {
            self.matched.clients.clear();
        }
    }

    fn update_matcher(&mut self) -> bool {
        let mut clear_clients = false;
        let state = &self.state;
        if let Some(new) = self.criterion.to_crit(state) {
            clear_clients = true;
            let matched = self.matched.clone();
            let leaf = state.cl_matcher_manager.leaf(&new, move |data| {
                matched.clients.set(data, ());
                matched.request_frame();
                Box::new({
                    let matched = matched.clone();
                    move || {
                        matched.clients.remove(&data);
                        matched.request_frame();
                    }
                })
            });
            state.cl_matcher_manager.rematch_all(state);
            self.leaf = Some(leaf);
        }
        clear_clients
    }
}

pub struct ClientPane {
    client: Rc<Client>,
}

impl ControlCenterInner {
    pub fn create_client_pane(self: &Rc<Self>, client: &Rc<Client>) -> ClientPane {
        ClientPane {
            client: client.clone(),
        }
    }
}

impl ClientPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Client ");
        res.push_str(&self.client.pid_info.comm);
    }

    pub fn show(&mut self, behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        show_client(behavior, ui, &self.client);
    }
}

pub fn show_client_collapsible(behavior: &mut CcBehavior, ui: &mut Ui, client: &Rc<Client>) {
    let mut layout_job = LayoutJob::default();
    layout_job.append(
        "Client",
        0.0,
        TextFormat {
            color: ui.style().visuals.widgets.inactive.text_color(),
            ..Default::default()
        },
    );
    layout_job.append(
        &client.id.to_string(),
        10.0,
        TextFormat {
            color: ui.style().visuals.widgets.active.text_color(),
            ..Default::default()
        },
    );
    layout_job.append(
        &client.pid_info.comm,
        10.0,
        TextFormat {
            color: ui.style().visuals.widgets.inactive.text_color(),
            ..Default::default()
        },
    );
    CollapsingHeader::new(layout_job)
        .id_salt(("client", client.id))
        .show(ui, |ui| {
            if icon_label(ICON_OPEN_IN_NEW)
                .sense(Sense::CLICK)
                .ui(ui)
                .clicked()
            {
                behavior.open = Some(PaneType::Client(behavior.cc.create_client_pane(client)));
            }
            show_client(behavior, ui, client)
        });
}

pub fn show_client(behavior: &mut CcBehavior<'_>, ui: &mut Ui, client: &Client) {
    grid(ui, ("client", client.id), |ui| {
        label(ui, "ID", client.id.to_string());
        label(ui, "PID", client.pid_info.pid.to_string());
        label(ui, "UID", client.pid_info.uid.to_string());
        label(ui, "comm", &client.pid_info.comm);
        label(ui, "exe", &client.pid_info.exe);
        if client.acceptor.sandboxed {
            read_only_bool(ui, "Sandboxed", true);
        }
        if client.acceptor.secure {
            read_only_bool(ui, "Secure", true);
        }
        if client.is_xwayland {
            read_only_bool(ui, "Xwayland", true);
        }
        if let Some(v) = &client.acceptor.sandbox_engine {
            label(ui, "Sandbox Engine", v);
        }
        if let Some(v) = &client.acceptor.app_id {
            label(ui, "App ID", v);
        }
        if let Some(v) = &client.acceptor.instance_id {
            label(ui, "Instance ID", v);
        }
        if let Some(v) = &client.acceptor.tag {
            label(ui, "Tag", v);
        }
    });
    if ui.button("Kill").clicked() {
        client.state.clients.kill(client.id);
    }
    ui.collapsing("Capabilities", |ui| {
        ui.add_enabled_ui(false, |ui| {
            for (k, v) in client.effective_caps.get().to_map() {
                if v {
                    ui.checkbox(&mut true, k.text());
                }
            }
        });
    });
    ui.collapsing("Windows", |ui| {
        let matcher = ui.memory_mut(|m| {
            m.caches
                .cache::<ClientWindowMatchersCache>()
                .get(behavior.cc, client.id)
                .clone()
        });
        let mut windows: Vec<_> = matcher.windows.lock().keys().copied().collect();
        windows.sort();
        for id in windows {
            let Some(window) = client.state.toplevels.get(&id).and_then(|v| v.upgrade()) else {
                continue;
            };
            show_window_collapsible(behavior, ui, &window);
        }
    });
}

#[derive(Default)]
struct ClientWindowMatchersCache {
    generation: u64,
    matchers: AHashMap<ClientId, CachedWindowMatcher>,
}

struct CachedWindowMatcher {
    generation: u64,
    _matcher: Rc<CritLeafMatcher<ToplevelData>>,
    matchers: Rc<WindowMatchers>,
}

struct WindowMatchers {
    cc: Weak<ControlCenterInner>,
    windows: CopyHashMap<ToplevelIdentifier, ()>,
}

impl ClientWindowMatchersCache {
    fn get(&mut self, cc: &Rc<ControlCenterInner>, id: ClientId) -> &Rc<WindowMatchers> {
        let res = self.matchers.entry(id).or_insert_with(|| {
            let state = &cc.state;
            let node = state.cl_matcher_manager.id(id);
            let node = state.tl_matcher_manager.client(state, &node);
            let matchers = Rc::new(WindowMatchers {
                cc: Rc::downgrade(&cc),
                windows: Default::default(),
            });
            let matchers2 = matchers.clone();
            let matcher = state.tl_matcher_manager.leaf(&node, move |id| {
                matchers2.windows.set(id, ());
                if let Some(cc) = matchers2.cc.upgrade() {
                    cc.window.request_redraw();
                }
                let matchers2 = matchers2.clone();
                Box::new(move || {
                    matchers2.windows.remove(&id);
                    if let Some(cc) = matchers2.cc.upgrade() {
                        cc.window.request_redraw();
                    }
                })
            });
            let res = CachedWindowMatcher {
                generation: 0,
                _matcher: matcher,
                matchers,
            };
            state.cl_matcher_manager.rematch_all(state);
            state.tl_matcher_manager.rematch_all(state);
            res
        });
        res.generation = self.generation;
        &res.matchers
    }
}

unsafe impl Sync for ClientWindowMatchersCache {}
unsafe impl Send for ClientWindowMatchersCache {}

impl CacheTrait for ClientWindowMatchersCache {
    fn update(&mut self) {
        self.matchers.retain(|_, m| m.generation == self.generation);
        self.generation += 1;
    }

    fn len(&self) -> usize {
        self.matchers.len()
    }
}
