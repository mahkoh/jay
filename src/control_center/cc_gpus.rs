use {
    crate::{
        control_center::{
            ControlCenterInner, GridExt, bool, combo_box, grid, grid_label, label, row,
        },
        egui_adapter::egui_platform::icons::{ICON_ADD, ICON_REMOVE},
        state::{DrmDevData, State},
    },
    egui::{Checkbox, CollapsingHeader, DragValue, TextFormat, Ui, Widget, text::LayoutJob},
    std::rc::Rc,
};

pub struct GpusPane {
    state: Rc<State>,
}

impl ControlCenterInner {
    pub fn create_gpus_pane(self: &Rc<Self>) -> GpusPane {
        GpusPane {
            state: self.state.clone(),
        }
    }
}

impl GpusPane {
    pub fn title(&self, res: &mut String) {
        res.push_str("GPUs");
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let devs = self.state.drm_devs.lock();
        let mut devs: Vec<_> = devs.iter().collect();
        devs.sort_by_key(|d| d.0);
        for dev in devs {
            self.show_dev(ui, dev.1);
        }
    }

    fn show_dev(&self, ui: &mut Ui, dev: &DrmDevData) {
        let title_buf;
        let title = match dev.devnode.as_deref() {
            Some(t) => t,
            _ => {
                let dev_t = dev.dev.dev_t();
                title_buf = format!("{}:{}", uapi::major(dev_t), uapi::minor(dev_t));
                &title_buf
            }
        };
        let mut layout_job = LayoutJob::default();
        layout_job.append(
            title,
            0.0,
            TextFormat {
                color: ui.style().visuals.widgets.active.text_color(),
                ..Default::default()
            },
        );
        if let Some(model) = &dev.model {
            layout_job.append(
                model,
                10.0,
                TextFormat {
                    color: ui.style().visuals.widgets.inactive.text_color(),
                    ..Default::default()
                },
            );
        }
        ui.collapsing(layout_job, |ui| {
            grid(ui, ("settings", dev.dev.id()), |ui| {
                macro_rules! string {
                    ($field:ident, $name:expr) => {
                        if let Some(v) = &dev.$field {
                            label(ui, $name, v);
                        }
                    };
                }
                string!(vendor, "Vendor");
                string!(model, "Model");
                string!(devnode, "Devnode");
                string!(syspath, "Syspath");
                if let Some(v) = dev.pci_id {
                    label(ui, "PCI ID", format!("{:x}:{:x}", v.vendor, v.model));
                }
                {
                    let v = dev.dev.dev_t();
                    label(ui, "Dev", format!("{}:{}", uapi::major(v), uapi::minor(v)));
                }
                combo_box(ui, "API", dev.dev.gtx_api(), |v| dev.dev.set_gfx_api(v));
                row(ui, "Primary Device", |ui| {
                    let mut v = dev.dev.is_render_device();
                    let old = v;
                    ui.add_enabled(!v, Checkbox::without_text(&mut v));
                    if v != old {
                        dev.dev.make_render_device();
                    }
                });
                bool(
                    ui,
                    "Direct Scanout",
                    dev.dev.direct_scanout_enabled(),
                    |v| dev.set_direct_scanout_enabled(&self.state, v),
                );
                if let Some(mut v) = dev.dev.flip_margin() {
                    let ui = &mut *ui.row();
                    grid_label(ui, "Flip Margin");
                    let old = v;
                    let denom = 1_000_000.0;
                    ui.horizontal(|ui| {
                        let mut s = v as f64 / denom;
                        let res = DragValue::new(&mut s)
                            .range(0.0..=50.0)
                            .speed(0.1)
                            .fixed_decimals(1)
                            .ui(ui);
                        if res.changed() {
                            v = (s * denom) as u64;
                        }
                        if ui.button(ICON_REMOVE).clicked() {
                            v = v.saturating_sub(100_000);
                        }
                        if ui.button(ICON_ADD).clicked() {
                            v += 100_000;
                        }
                    });
                    if old != v {
                        dev.set_flip_margin(&self.state, v);
                    }
                }
            });
            CollapsingHeader::new("Connectors")
                .default_open(true)
                .show(ui, |ui| {
                    let mut cs: Vec<_> = dev
                        .connectors
                        .lock()
                        .values()
                        .map(|v| v.connector.kernel_id().to_string())
                        .collect::<Vec<_>>();
                    cs.sort();
                    for c in cs {
                        ui.label(c);
                    }
                });
        });
    }
}
