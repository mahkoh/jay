use {
    crate::{
        compositor::{LIBEI_SOCKET, WAYLAND_DISPLAY},
        control_center::{ControlCenterInner, bool, grid, label, row},
        state::State,
        version::VERSION,
    },
    egui::{DragValue, OpenUrl, Ui, Widget},
    std::rc::Rc,
};

pub struct CompositorPane {
    state: Rc<State>,
    switch_to_vt: u32,
}

impl ControlCenterInner {
    pub fn create_compositor_pane(self: &Rc<Self>) -> CompositorPane {
        CompositorPane {
            state: self.state.clone(),
            switch_to_vt: 1,
        }
    }
}

impl CompositorPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Compositor");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let s = &self.state;
        grid(ui, "compositor", |ui| {
            row(ui, "Repository", |ui| {
                let url = "https://github.com/mahkoh/jay";
                if ui.link(url).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(url));
                }
            });
            label(ui, "Version", VERSION);
            label(ui, "PID", s.pid.to_string());
            if let Some(acceptor) = s.acceptor.get() {
                label(ui, WAYLAND_DISPLAY, acceptor.socket_name());
            }
            if let Some(dir) = &s.config_dir {
                label(ui, "Config DIR", dir);
            }
            bool(ui, "Libei Socket", s.enable_ei_acceptor.get(), |v| {
                s.set_ei_socket_enabled(v);
            });
            if let Some(a) = s.ei_acceptor.get() {
                label(ui, LIBEI_SOCKET, a.socket_name());
            }
            if let Some(logger) = &s.logger {
                row(ui, "Log File", |ui| {
                    let path = logger.path().to_string();
                    if ui
                        .link(&path)
                        .on_hover_text_at_pointer("Copy to clipboard")
                        .clicked()
                    {
                        ui.ctx().copy_text(path);
                    }
                });
            }
        });
        if ui.button("Quit").clicked() {
            s.quit();
        }
        if ui.button("Reload Config").clicked() {
            s.reload_config();
        }
        ui.horizontal(|ui| {
            let button = ui.button("Switch to VT");
            DragValue::new(&mut self.switch_to_vt).ui(ui);
            if button.clicked() {
                s.backend.get().switch_to(self.switch_to_vt);
            }
        });
    }
}
