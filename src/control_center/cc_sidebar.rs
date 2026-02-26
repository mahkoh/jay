use {
    crate::control_center::{ControlCenterInner, Pane, PaneType},
    egui::{Align, Layout, ScrollArea, Ui, ViewportCommand},
    egui_tiles::Tree,
    linearize::{Linearize, LinearizeExt},
    std::{rc::Rc, sync::LazyLock},
};

#[derive(Copy, Clone, Linearize)]
enum PaneName {
    Compositor,
    Idle,
    ColorManagement,
    Xwayland,
    Outputs,
    GPUs,
    Input,
    LookAndFeel,
    Clients,
    WindowSearch,
}

impl PaneName {
    fn name(self) -> &'static str {
        match self {
            PaneName::Compositor => "Compositor",
            PaneName::Idle => "Idle",
            PaneName::ColorManagement => "Color Management",
            PaneName::Xwayland => "Xwayland",
            PaneName::Outputs => "Outputs",
            PaneName::GPUs => "GPUs",
            PaneName::Input => "Input",
            PaneName::LookAndFeel => "Look and Feel",
            PaneName::Clients => "Clients",
            PaneName::WindowSearch => "Window Search",
        }
    }
}

static TYPES: LazyLock<Vec<PaneName>> = LazyLock::new(|| {
    let mut res: Vec<_> = PaneName::variants().collect();
    res.sort_by_key(|t| t.name());
    res
});

impl ControlCenterInner {
    pub fn show_sidebar(self: &Rc<Self>, tree: &mut Tree<Pane>, ui: &mut Ui) {
        ui.with_layout(
            Layout::top_down(Align::Center).with_cross_justify(true),
            |ui| {
                if ui.button("Close").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    for &ty in &*TYPES {
                        if ui.button(ty.name()).clicked() {
                            let ty = match ty {
                                PaneName::Compositor => {
                                    PaneType::Compositor(self.create_compositor_pane())
                                }
                                PaneName::Idle => PaneType::Idle(self.create_idle_pane()),
                                PaneName::ColorManagement => {
                                    PaneType::ColorManagement(self.create_color_management_pane())
                                }
                                PaneName::Xwayland => {
                                    PaneType::Xwayland(self.create_xwayland_pane())
                                }
                                PaneName::Outputs => {
                                    PaneType::Outputs(Box::new(self.create_outputs_pane()))
                                }
                                PaneName::GPUs => PaneType::GPUs(self.create_gpus_pane()),
                                PaneName::Input => PaneType::Input(self.create_input_pane()),
                                PaneName::LookAndFeel => {
                                    PaneType::LookAndFeel(self.create_look_and_feel_pane())
                                }
                                PaneName::Clients => PaneType::Clients(self.create_clients_pane()),
                                PaneName::WindowSearch => {
                                    PaneType::WindowSearch(self.create_window_search_pane())
                                }
                            };
                            self.open(tree, ty);
                        }
                    }
                })
            },
        );
    }
}
