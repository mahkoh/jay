use crate::compositor::LIBEI_SOCKET;
use crate::compositor::WAYLAND_DISPLAY;
use crate::control_center::ControlCenterInner;
use crate::control_center::bool;
use crate::control_center::combo_box;
use crate::control_center::grid;
use crate::control_center::label;
use crate::control_center::row;
use crate::control_center::row_ui;
use crate::control_center::tip;
use crate::state::State;
use crate::version::VERSION;
use egui::DragValue;
use egui::OpenUrl;
use egui::Ui;
use egui::Widget;
use std::rc::Rc;

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
                    ui.open_url(OpenUrl::new_tab(url));
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
            combo_box(
                ui,
                "Workspace Display Order",
                s.workspace_display_order.get(),
                |o| s.set_workspace_display_order(o),
            );
            if let Some(logger) = &s.logger {
                combo_box(ui, "Log Level", logger.level(), |l| s.set_log_level(l));
                row(ui, "Log File", |ui| {
                    let path = logger.path().to_string();
                    if ui
                        .link(&path)
                        .on_hover_text_at_pointer("Copy to clipboard")
                        .clicked()
                    {
                        ui.copy_text(path);
                    }
                });
            }
            bool(
                ui,
                "Session Management",
                s.session_management_enabled.get(),
                |v| s.set_session_management_enabled(v),
            );
            bool(
                ui,
                "Visualize Compositing",
                s.visualize_compositing.get(),
                |v| s.set_visualize_compositing(v),
            );
            let mut timeout = |name: &str, label: &str, old: u64, set: &dyn Fn(u64)| {
                row_ui(
                    ui,
                    &format!("{name} Timeout"),
                    |ui| {
                        tip(ui, |ui| {
                            ui.label(format!("The timeout for {label}."));
                            ui.label("See the book for more details.");
                        });
                    },
                    |ui| {
                        ui.horizontal(|ui| {
                            let micros = old / 1_000;
                            let mut millis = micros / 1_000;
                            let mut micros = micros % 1_000;
                            let mut changed = false;
                            changed |= DragValue::new(&mut millis).ui(ui).changed();
                            ui.label("millis");
                            changed |= DragValue::new(&mut micros).ui(ui).changed();
                            ui.label("micros");
                            if changed {
                                let ns = millis
                                    .saturating_mul(1_000_000)
                                    .saturating_add(micros.saturating_mul(1_000));
                                set(ns);
                            }
                        })
                    },
                )
            };
            timeout(
                "Transaction",
                "desktop transactions",
                s.tree.transactions.timeout_ns(),
                &|v| s.set_transaction_timeout_ns(v),
            );
            timeout(
                "Configure",
                "configure requests",
                s.tree.configure_groups.timeout_ns(),
                &|v| s.set_configure_timeout_ns(v),
            );
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
