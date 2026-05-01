use {
    crate::{
        control_center::{ControlCenterInner, bool, grid, row},
        state::State,
        tree::Node,
    },
    egui::{CollapsingHeader, ComboBox, Ui},
    std::rc::Rc,
};

pub struct WorkspacesPane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_workspaces_pane(self: &Rc<Self>) -> WorkspacesPane {
        WorkspacesPane {
            state: self.state.clone(),
        }
    }
}

impl WorkspacesPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Workspaces");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let mut ws: Vec<_> = self.state.workspaces.lock().values().cloned().collect();
        ws.sort_unstable_by_key(|ws| ws.name.clone());
        let mut outputs: Vec<_> = self.state.root.outputs.lock().values().cloned().collect();
        outputs.sort_unstable_by_key(|o| o.global.connector.name.clone());
        for ws in ws {
            let output = ws.output.get();
            CollapsingHeader::new(&*ws.name).show(ui, |ui| {
                grid(ui, "settings", |ui| {
                    row(ui, "Position", |ui| {
                        let p = ws.position.get();
                        if output.is_dummy {
                            ui.label("hidden");
                        } else {
                            ui.label(format!(
                                "{}x{} + {}x{}",
                                p.x1(),
                                p.y1(),
                                p.width(),
                                p.height(),
                            ));
                        }
                    });
                    bool(ui, "Visible", ws.visible.get(), |v| {
                        if v {
                            ws.clone().node_make_visible();
                        }
                    });
                    row(ui, "Output", |ui| {
                        let mut new = &output;
                        ComboBox::from_id_salt("output")
                            .selected_text(&*output.global.connector.name)
                            .show_ui(ui, |ui| {
                                for o in &outputs {
                                    ui.selectable_value(&mut new, o, &*o.global.connector.name);
                                }
                            });
                        if output.id != new.id {
                            self.state.move_ws_to_output(&ws, new);
                        }
                    });
                });
            });
        }
    }
}
