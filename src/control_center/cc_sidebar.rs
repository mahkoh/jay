use {
    crate::control_center::{ControlCenterInner, Pane, PaneType},
    egui::{Align, Layout, ScrollArea, Ui, ViewportCommand},
    egui_tiles::{Tile, Tree},
    linearize::{Linearize, LinearizeExt},
    std::{rc::Rc, sync::LazyLock},
};

#[derive(Copy, Clone, Linearize)]
enum PaneTypes {
    Idle,
    X(u8),
}

impl PaneTypes {
    fn name(self) -> &'static str {
        match self {
            PaneTypes::Idle => "Idle",
            PaneTypes::X(_) => "X",
        }
    }
}

static TYPES: LazyLock<Vec<PaneTypes>> = LazyLock::new(|| {
    let mut res: Vec<_> = PaneTypes::variants().collect();
    res.sort_by_key(|t| t.name());
    res
});

impl ControlCenterInner {
    pub fn show_sidebar(self: &Rc<Self>, tree: &mut Tree<Pane>, ui: &mut Ui) {
        ui.with_layout(
            Layout::top_down(Align::Center).with_cross_justify(true),
            |ui| {
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    for &ty in &*TYPES {
                        if ui.button(ty.name()).clicked() {
                            let ty = match ty {
                                PaneTypes::Idle => PaneType::Idle(self.create_idle_pane()),
                                PaneTypes::X(_) => PaneType::Idle(self.create_idle_pane()),
                            };
                            let pane = Pane {
                                id: self.next_pane_id.fetch_add(1),
                                ty,
                            };
                            let id = tree.tiles.insert_pane(pane);
                            if let Some(root) = tree.root
                                && let Some(tile) = tree.tiles.get_mut(root)
                            {
                                match tile {
                                    Tile::Container(c) => {
                                        c.add_child(id);
                                    }
                                    Tile::Pane(_) => {
                                        let root =
                                            tree.tiles.insert_horizontal_tile(vec![root, id]);
                                        tree.root = Some(root);
                                    }
                                }
                            } else {
                                tree.root = Some(id);
                            }
                        }
                    }
                })
            },
        );
    }
}
