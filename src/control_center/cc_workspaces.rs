use crate::control_center::ControlCenterInner;
use crate::control_center::bool;
use crate::control_center::grid;
use crate::control_center::label;
use crate::control_center::row;
use crate::state::State;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::WorkspaceType;
use egui::CollapsingHeader;
use egui::ComboBox;
use egui::TextFormat;
use egui::Ui;
use egui::text::LayoutJob;
use std::rc::Rc;

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
            let ns = &ws.node_state[LiveTL];
            let output = ns.output.get();
            let ty = match ws.ty {
                WorkspaceType::Normal => "Normal",
                WorkspaceType::Overlay => "Overlay",
            };
            let mut layout_job = LayoutJob::default();
            layout_job.append(
                ty,
                0.0,
                TextFormat {
                    color: ui.style().visuals.widgets.inactive.text_color(),
                    ..Default::default()
                },
            );
            layout_job.append(
                &ws.name,
                10.0,
                TextFormat {
                    color: ui.style().visuals.widgets.active.text_color(),
                    ..Default::default()
                },
            );
            CollapsingHeader::new(layout_job).show(ui, |ui| {
                grid(ui, "settings", |ui| {
                    label(ui, "Type", ty);
                    row(ui, "Position", |ui| {
                        let p = ns.position.get();
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
                    bool(ui, "Visible", ns.visible.get(), |v| {
                        if v {
                            ws.node_make_visible();
                        } else if ws.ty == WorkspaceType::Overlay {
                            ns.output.get().hide_overlay();
                        }
                    });
                    row(ui, "Output", |ui| {
                        let mut new = &output;
                        let mut cb = ComboBox::from_id_salt("output");
                        if !output.is_dummy {
                            cb = cb.selected_text(&*output.global.connector.name);
                        }
                        cb.show_ui(ui, |ui| {
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
