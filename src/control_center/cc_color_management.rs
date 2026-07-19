use crate::control_center::ControlCenterInner;
use crate::control_center::bool;
use crate::control_center::grid;
use crate::control_center::read_only_bool;
use crate::state::State;
use egui::Ui;
use std::rc::Rc;

pub struct ColorManagementPane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_color_management_pane(self: &Rc<Self>) -> ColorManagementPane {
        ColorManagementPane {
            state: self.state.clone(),
        }
    }
}

impl ColorManagementPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Color Management");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let s = &self.state;
        grid(ui, "settings", |ui| {
            bool(ui, "Enabled", s.color_management_enabled.get(), |b| {
                s.set_color_management_enabled(b);
            });
            read_only_bool(ui, "Available", s.color_management_available());
        });
    }
}
