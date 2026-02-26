use {
    crate::{
        cmm::cmm_eotf::Eotf,
        control_center::{
            ControlCenterInner, bool, bool_ui, combo_box, drag_value, grid, grid_label, row,
            text_edit, tip,
        },
        gfx_api::AlphaMode,
        state::State,
        theme::{Color, ThemeColor, ThemeSized},
        utils::static_text::StaticText,
    },
    egui::Ui,
    isnt::std_1::primitive::IsntStrExt,
    linearize::LinearizeExt,
    std::rc::Rc,
};

pub struct LookAndFeelPane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_look_and_feel_pane(self: &Rc<Self>) -> LookAndFeelPane {
        LookAndFeelPane {
            state: self.state.clone(),
        }
    }
}

impl LookAndFeelPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("Look and Feel");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let t = &self.state.theme;
        grid(ui, "settings", |ui| {
            bool(ui, "Show Bar", self.state.show_bar.get(), |v| {
                self.state.set_show_bar(v)
            });
            combo_box(ui, "Bar Position", t.bar_position.get(), |p| {
                self.state.set_bar_position(p);
            });
            bool(ui, "Show Titles", t.show_titles.get(), |v| {
                self.state.set_show_titles(v)
            });
            bool_ui(
                ui,
                "Primary Selection",
                |ui| {
                    tip(ui, |ui| {
                        ui.label("Requires applications to be restarted.");
                    });
                },
                self.state.enable_primary_selection.get(),
                |v| self.state.set_primary_selection_enabled(v),
            );
            bool_ui(
                ui,
                "UI Drag",
                |ui| {
                    tip(ui, |ui| {
                        ui.label("Allows dragging workspaces and tiled windows with the mouse.");
                    });
                },
                self.state.ui_drag_enabled.get(),
                |v| self.state.set_ui_drag_enabled(v),
            );
            drag_value(
                ui,
                "UI Drag Threshold (px)",
                self.state.ui_drag_threshold_squared.get().isqrt(),
                1..=i32::MAX,
                1.0,
                |v| {
                    self.state.set_ui_drag_threshold(v);
                },
            );
            bool_ui(
                ui,
                "Float Pin Icon",
                |ui| {
                    tip(ui, |ui| {
                        ui.label("Show the pin icon even if the window is not pinned.");
                        ui.label("Pinned floating windows are shown on all workspaces.");
                    });
                },
                self.state.show_pin_icon.get(),
                |v| self.state.set_show_pin_icon(v),
            );
            bool_ui(
                ui,
                "Float Above Fullscreen",
                |ui| {
                    tip(ui, |ui| {
                        ui.label("Show floating windows above fullscreen windows.");
                    });
                },
                self.state.float_above_fullscreen.get(),
                |v| self.state.set_float_above_fullscreen(v),
            );
            row(ui, "Font", |ui| {
                let mut v = self.state.theme.font.get().to_string();
                if text_edit(ui, &mut v).changed() {
                    self.state.set_font(&v);
                }
            });
            row(ui, "Title Font", |ui| {
                let mut v = t
                    .title_font
                    .get()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                if text_edit(ui, &mut v).changed() {
                    self.state.set_title_font(v.is_not_empty().then_some(&v));
                }
            });
            row(ui, "Bar Font", |ui| {
                let mut v = t.bar_font.get().map(|v| v.to_string()).unwrap_or_default();
                if text_edit(ui, &mut v).changed() {
                    self.state.set_bar_font(v.is_not_empty().then_some(&v));
                }
            });
        });
        if ui.button("Reset Sizes").clicked() {
            self.state.reset_sizes();
        }
        if ui.button("Reset Colors").clicked() {
            self.state.reset_colors();
        }
        if ui.button("Reset Fonts").clicked() {
            self.state.reset_fonts();
        }
        ui.collapsing("Sizes", |ui| {
            grid(ui, "Sizes", |ui| {
                for v in ThemeSized::variants() {
                    let f = v.field(&self.state.theme);
                    drag_value(ui, v.text(), f.get(), v.min()..=v.max(), 1.0, |i| {
                        self.state.set_size(v, i);
                    });
                }
            });
        });
        ui.collapsing("Colors", |ui| {
            grid(ui, "Colors", |ui| {
                for tc in ThemeColor::variants() {
                    let f = tc.field(t);
                    let mut v = f.get().to_array(Eotf::Linear);
                    grid_label(ui, tc.text());
                    let changed = ui.color_edit_button_rgba_premultiplied(&mut v).changed();
                    ui.end_row();
                    if changed {
                        let [r, g, b, a] = v;
                        let c =
                            Color::new(Eotf::Linear, AlphaMode::PremultipliedOptical, r, g, b, a);
                        self.state.set_color(tc, c);
                    }
                }
            });
        });
    }
}
