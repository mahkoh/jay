use {
    crate::{
        control_center::ControlCenterInner, egui_adapter::egui_platform::icons::ICON_CLOSE,
        state::State,
    },
    egui::Ui,
    std::rc::Rc,
};

pub struct VirtualOutputsPane {
    state: Rc<State>,
    new: String,
}

impl ControlCenterInner {
    pub fn create_virtual_outputs_pane(self: &Rc<Self>) -> VirtualOutputsPane {
        VirtualOutputsPane {
            state: self.state.clone(),
            new: Default::default(),
        }
    }
}

impl VirtualOutputsPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Virtual Outputs");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let s = &self.state;
        let mut outputs: Vec<_> = s.virtual_outputs.outputs.lock().keys().cloned().collect();
        outputs.sort();
        for o in &outputs {
            ui.horizontal(|ui| {
                if ui.button(ICON_CLOSE).clicked() {
                    s.virtual_outputs.remove_output(s, o);
                }
                ui.label(o);
            });
        }
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new);
            if ui.button("Add").clicked() {
                s.virtual_outputs.get_or_create(s, &self.new);
                ui.ctx().request_repaint();
            }
        });
    }
}
