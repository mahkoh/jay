use {
    crate::{
        control_center::ControlCenterInner,
        state::{IdleChangedListener, State},
        utils::event_listener::EventListener,
    },
    egui::{CollapsingHeader, DragValue, Grid, Ui, Widget},
    std::{rc::Rc, time::Duration},
};

pub struct IdlePane {
    state: Rc<State>,
    listener: EventListener<dyn IdleChangedListener>,
}

impl IdleChangedListener for ControlCenterInner {
    fn changed(&self) {
        self.window.request_redraw();
    }
}

impl ControlCenterInner {
    pub fn create_idle_pane(self: &Rc<Self>) -> IdlePane {
        let pane = IdlePane {
            state: self.state.clone(),
            listener: EventListener::new2(self.clone()),
        };
        pane.listener.attach(&self.state.idle.idle_changed_event);
        pane
    }
}

impl IdlePane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Idle");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        Grid::new("sliders").show(ui, |ui| {
            for interval in [true, false] {
                let label = match interval {
                    true => "Interval:",
                    false => "Grace period:",
                };
                let idle = &self.state.idle;
                let field = match interval {
                    true => &idle.timeout,
                    false => &idle.grace_period,
                };
                ui.label(label);
                let secs = field.get().as_secs();
                let mut minutes = secs / 60;
                let mut seconds = secs % 60;
                let mut changed = false;
                changed |= DragValue::new(&mut minutes).ui(ui).changed();
                ui.label("minutes");
                changed |= DragValue::new(&mut seconds).range(0..=59).ui(ui).changed();
                ui.label("seconds");
                if changed {
                    let duration = Duration::from_secs(minutes * 60 + seconds);
                    match interval {
                        true => idle.set_timeout(duration),
                        false => idle.set_grace_period(duration),
                    }
                }
                ui.end_row();
            }
        });
        ui.add_space(5.0);
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
