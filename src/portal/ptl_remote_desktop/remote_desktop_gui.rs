use {
    crate::{
        ifs::wl_seat::{wl_pointer::PRESSED, BTN_LEFT},
        portal::{
            ptl_display::{PortalDisplay, PortalOutput, PortalSeat},
            ptl_remote_desktop::{PortalSession, RemoteDesktopPhase},
            ptr_gui::{
                Align, Button, ButtonOwner, Flow, GuiElement, Label, Orientation, OverlayWindow,
                OverlayWindowOwner,
            },
        },
        theme::Color,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt},
    },
    std::rc::Rc,
};

const H_MARGIN: f32 = 30.0;
const V_MARGIN: f32 = 20.0;

pub struct SelectionGui {
    remote_desktop_session: Rc<PortalSession>,
    dpy: Rc<PortalDisplay>,
    surfaces: CopyHashMap<u32, Rc<SelectionGuiSurface>>,
}

pub struct SelectionGuiSurface {
    gui: Rc<SelectionGui>,
    output: Rc<PortalOutput>,
    overlay: Rc<OverlayWindow>,
}

struct StaticButton {
    surface: Rc<SelectionGuiSurface>,
    role: ButtonRole,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ButtonRole {
    Accept,
    Reject,
}

impl SelectionGui {
    pub fn kill(&self, upwards: bool) {
        for surface in self.surfaces.lock().drain_values() {
            surface.overlay.data.kill(false);
        }
        if let RemoteDesktopPhase::Selecting(s) = self.remote_desktop_session.rd_phase.get() {
            s.guis.remove(&self.dpy.id);
            if upwards && s.guis.is_empty() {
                self.remote_desktop_session.kill();
            }
        }
    }
}

fn create_accept_gui(surface: &Rc<SelectionGuiSurface>) -> Rc<dyn GuiElement> {
    let app = &surface.gui.remote_desktop_session.app;
    let text = if app.is_empty() {
        format!("An application wants to generate/monitor input")
    } else {
        format!("`{}` wants to generate/monitor input", app)
    };
    let label = Rc::new(Label::default());
    *label.text.borrow_mut() = text;
    let accept_button = static_button(surface, ButtonRole::Accept, "Allow");
    let reject_button = static_button(surface, ButtonRole::Reject, "Reject");
    for button in [&accept_button, &reject_button] {
        button.border_color.set(Color::from_gray(100));
        button.border.set(2.0);
        button.padding.set(5.0);
    }
    accept_button.bg_color.set(Color::from_rgb(170, 200, 170));
    accept_button
        .bg_hover_color
        .set(Color::from_rgb(170, 255, 170));
    reject_button.bg_color.set(Color::from_rgb(200, 170, 170));
    reject_button
        .bg_hover_color
        .set(Color::from_rgb(255, 170, 170));
    let flow = Rc::new(Flow::default());
    flow.orientation.set(Orientation::Vertical);
    flow.cross_align.set(Align::Center);
    flow.in_margin.set(V_MARGIN);
    flow.cross_margin.set(H_MARGIN);
    *flow.elements.borrow_mut() = vec![label, accept_button, reject_button];
    flow
}

impl OverlayWindowOwner for SelectionGuiSurface {
    fn kill(&self, upwards: bool) {
        self.gui.dpy.windows.remove(&self.overlay.data.surface.id);
        self.gui.surfaces.remove(&self.output.global_id);
        if upwards && self.gui.surfaces.is_empty() {
            self.gui.kill(true);
        }
    }
}

impl SelectionGui {
    pub fn new(ss: &Rc<PortalSession>, dpy: &Rc<PortalDisplay>) -> Rc<Self> {
        let gui = Rc::new(SelectionGui {
            remote_desktop_session: ss.clone(),
            dpy: dpy.clone(),
            surfaces: Default::default(),
        });
        for output in dpy.outputs.lock().values() {
            let sgs = Rc::new(SelectionGuiSurface {
                gui: gui.clone(),
                output: output.clone(),
                overlay: OverlayWindow::new(output),
            });
            let element = create_accept_gui(&sgs);
            sgs.overlay.data.content.set(Some(element));
            gui.dpy
                .windows
                .set(sgs.overlay.data.surface.id, sgs.overlay.data.clone());
            gui.surfaces.set(output.global_id, sgs);
        }
        gui
    }
}

impl ButtonOwner for StaticButton {
    fn button(&self, _seat: &PortalSeat, button: u32, state: u32) {
        if button != BTN_LEFT || state != PRESSED {
            return;
        }
        match self.role {
            ButtonRole::Accept => {
                log::info!("User has accepted the request");
                let selecting = match self.surface.gui.remote_desktop_session.rd_phase.get() {
                    RemoteDesktopPhase::Selecting(selecting) => selecting,
                    _ => return,
                };
                for gui in selecting.guis.lock().drain_values() {
                    gui.kill(false);
                }
                selecting.starting(&self.surface.output.dpy);
            }
            ButtonRole::Reject => {
                log::info!("User has rejected the remote desktop request");
                self.surface.gui.remote_desktop_session.kill();
            }
        }
    }
}

fn static_button(surface: &Rc<SelectionGuiSurface>, role: ButtonRole, text: &str) -> Rc<Button> {
    let button = Rc::new(Button::default());
    let slf = Rc::new(StaticButton {
        surface: surface.clone(),
        role,
    });
    button.owner.set(Some(slf));
    *button.text.borrow_mut() = text.to_string();
    button
}
