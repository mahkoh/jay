use {
    crate::{
        control_center::{ControlCenterInner, grid, row},
        state::State,
    },
    egui::{CollapsingHeader, DragValue, Ui, Widget},
    std::{rc::Rc, time::Duration},
};

pub struct IdlePane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_idle_pane(self: &Rc<Self>) -> IdlePane {
        IdlePane {
            state: self.state.clone(),
        }
    }
}

impl IdlePane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Idle");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        grid(ui, "sliders", |ui| {
            for interval in [true, false] {
                let label = match interval {
                    true => "Interval",
                    false => "Grace period",
                };
                let idle = &self.state.idle;
                let field = match interval {
                    true => &idle.timeout,
                    false => &idle.grace_period,
                };
                row(ui, label, |ui| {
                    let secs = field.get().as_secs();
                    let mut minutes = secs / 60;
                    let mut seconds = secs % 60;
                    let mut changed = false;
                    changed |= DragValue::new(&mut minutes).ui(ui).changed();
                    ui.label("minutes");
                    changed |= DragValue::new(&mut seconds).range(0..=59).ui(ui).changed();
                    ui.label("seconds");
                    if changed {
                        let duration =
                            Duration::from_secs(minutes.saturating_mul(60).saturating_add(seconds));
                        match interval {
                            true => idle.set_timeout(&self.state, duration),
                            false => idle.set_grace_period(&self.state, duration),
                        }
                    }
                });
            }
        });
        let inhibitors = self.state.idle.inhibitors.lock();
        let mut is: Vec<_> = inhibitors.values().collect();
        is.sort_by_key(|is| is.inhibit_id);
        CollapsingHeader::new(format!("Inhibitors ({})", is.len()))
            .id_salt("Inhibitors")
            .show(ui, |ui| {
                for i in is {
                    ui.horizontal(|ui| {
                        ui.label(&i.client.pid_info.comm);
                    });
                }
            });
    }
}
