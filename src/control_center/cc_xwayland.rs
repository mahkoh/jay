use {
    crate::{
        compositor::DISPLAY,
        control_center::{
            CcBehavior, ControlCenterInner, bool, cc_clients::show_client_collapsible,
            combo_box_ui, grid, label, read_only_bool, tip,
        },
        state::State,
        utils::{errorfmt::ErrorFmt, oserror::OsError, static_text::StaticText},
    },
    egui::Ui,
    linearize::Linearize,
    std::rc::Rc,
    uapi::c,
};

pub struct XwaylandPane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_xwayland_pane(self: &Rc<Self>) -> XwaylandPane {
        XwaylandPane {
            state: self.state.clone(),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Linearize)]
enum ScalingMode {
    Default,
    Downscaled,
}

impl StaticText for ScalingMode {
    fn text(&self) -> &'static str {
        match self {
            ScalingMode::Default => "default",
            ScalingMode::Downscaled => "downscaled",
        }
    }
}

impl XwaylandPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Xwayland");
    }

    pub fn show(&mut self, behavior: &mut CcBehavior<'_>, ui: &mut Ui) {
        let s = &self.state;
        grid(ui, "settings", |ui| {
            bool(ui, "Enabled", s.xwayland.enabled.get(), |b| {
                s.set_xwayland_enabled(b)
            });
            let mode = match self.state.xwayland.use_wire_scale.get() {
                true => ScalingMode::Downscaled,
                false => ScalingMode::Default,
            };
            combo_box_ui(
                ui,
                "Scaling Mode",
                |ui| {
                    tip(ui, |ui| {
                        ui.label(r#"`downscaled` is known as "X applications scale themselves""#);
                    });
                },
                mode,
                |v| {
                    self.state
                        .set_xwayland_use_wire_scale(v == ScalingMode::Downscaled);
                },
            );
            if let Some(display) = self.state.xwayland.display.get() {
                label(ui, DISPLAY, &*display);
            }
            read_only_bool(ui, "Running", self.state.xwayland.running.get());
            if let Some(client) = self.state.xwayland.client.get() {
                label(ui, "PID", client.pid_info.pid.to_string());
            }
        });
        if let Some(client) = self.state.xwayland.client.get()
            && ui.button("Kill").clicked()
            && let Err(e) = uapi::kill(client.pid_info.pid, c::SIGTERM)
        {
            log::error!("Could not kill Xwayland: {}", ErrorFmt(OsError::from(e)));
        }
        if let Some(client) = self.state.xwayland.client.get() {
            show_client_collapsible(behavior, ui, &client);
        }
    }
}
