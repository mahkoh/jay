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
}

impl PaneName {
    fn name(self) -> &'static str {
        match self {
            PaneName::Compositor => "Compositor",
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
                ui.add_space(6.0);
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
                            };
                            self.open(tree, ty);
                            ui.ctx().request_repaint();
                        }
                    }
                    ui.add_space(3.0);
                })
            },
        );
    }
}
