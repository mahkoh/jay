use {
    crate::{
        ifs::wl_seat::{wl_pointer::PRESSED, BTN_LEFT},
        portal::{
            ptl_display::{PortalDisplay, PortalOutput},
            ptl_screencast::{
                ScreencastPhase, ScreencastSession, StartingScreencast,
            },
            ptr_gui::{
                Align, Button, ButtonOwner, Flow, GuiElement, Label, Orientation, OverlayWindow,
                OverlayWindowOwner,
            },
        },
        theme::Color,
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap},
    },
    std::rc::Rc,
};

const H_MARGIN: f32 = 30.0;
const V_MARGIN: f32 = 20.0;

pub struct SelectionGui {
    screencast_session: Rc<ScreencastSession>,
    dpy: Rc<PortalDisplay>,
    surfaces: CopyHashMap<u32, Rc<SelectionGuiSurface>>,
}

pub struct SelectionGuiSurface {
    gui: Rc<SelectionGui>,
    output: Rc<PortalOutput>,
    state: CloneCell<Rc<SelectionState>>,
    overlay: Rc<OverlayWindow>,
}

enum SelectionState {
    Accept,
    Select,
}

struct StaticButton {
    surface: Rc<SelectionGuiSurface>,
    role: ButtonRole,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ButtonRole {
    Accept,
    Reject,
    ShareAll,
    ShareSelected,
    Cancel,
}

impl SelectionGui {
    pub fn kill(&self, upwards: bool) {
        for (_, surface) in self.surfaces.lock().drain() {
            surface.overlay.data.kill(false);
        }
        if let ScreencastPhase::Selecting(s) = self.screencast_session.phase.get() {
            s.guis.remove(&self.dpy.id);
            if upwards && s.guis.is_empty() {
                self.screencast_session.kill();
            }
        }
    }
}

fn create_accept_gui(surface: &Rc<SelectionGuiSurface>) -> Rc<dyn GuiElement> {
    let app = &surface.gui.screencast_session.app;
    let text = if app.is_empty() {
        format!("An application wants to capture the screen")
    } else {
        format!("`{}` wants to capture the screen", app)
    };
    let label = Rc::new(Label::default());
    *label.text.borrow_mut() = text;
    let accept_button = static_button(surface, ButtonRole::Accept, "Share This Output");
    let reject_button = static_button(surface, ButtonRole::Reject, "Reject");
    let buttons = [&accept_button, &reject_button];
    for button in buttons {
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

fn create_select_gui(surface: &Rc<SelectionGuiSurface>) -> Rc<dyn GuiElement> {
    let share_all_button = static_button(surface, ButtonRole::ShareAll, "Share All Workspaces");
    let share_selected_button = static_button(
        surface,
        ButtonRole::ShareSelected,
        "Share Selected Workspaces",
    );
    let cancel_button = static_button(surface, ButtonRole::Cancel, "Cancel");
    let buttons = [&share_all_button, &share_selected_button, &cancel_button];
    for button in buttons {
        button.border_color.set(Color::from_gray(100));
        button.border.set(2.0);
        button.padding.set(5.0);
    }
    let buttons = [&share_all_button, &share_selected_button];
    for button in buttons {
        button.bg_color.set(Color::from_rgb(170, 200, 170));
        button.bg_hover_color.set(Color::from_rgb(170, 255, 170));
    }
    cancel_button.bg_color.set(Color::from_rgb(200, 170, 170));
    cancel_button
        .bg_hover_color
        .set(Color::from_rgb(255, 170, 170));
    let flow = Rc::new(Flow::default());
    flow.orientation.set(Orientation::Horizontal);
    flow.cross_align.set(Align::Center);
    flow.in_margin.set(H_MARGIN);
    flow.cross_margin.set(V_MARGIN);
    *flow.elements.borrow_mut() = vec![share_all_button, share_selected_button, cancel_button];
    flow
}

impl OverlayWindowOwner for SelectionGuiSurface {
    fn kill(&self, upwards: bool) {
        self.gui.surfaces.remove(&self.output.global_id);
        if upwards && self.gui.surfaces.is_empty() {
            self.gui.kill(true);
        }
    }
}

impl SelectionGui {
    pub fn new(ss: &Rc<ScreencastSession>, dpy: &Rc<PortalDisplay>) -> Rc<Self> {
        let gui = Rc::new(SelectionGui {
            screencast_session: ss.clone(),
            dpy: dpy.clone(),
            surfaces: Default::default(),
        });
        for output in dpy.outputs.lock().values() {
            let sgs = Rc::new(SelectionGuiSurface {
                gui: gui.clone(),
                output: output.clone(),
                state: CloneCell::new(Rc::new(SelectionState::Accept)),
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
    fn button(&self, button: u32, state: u32) {
        if button != BTN_LEFT || state != PRESSED {
            return;
        }
        match self.role {
            ButtonRole::Accept => {
                log::info!("User has accepted the request");
                for surface in self.surface.gui.surfaces.lock().values() {
                    if surface.output.global_id != self.surface.output.global_id {
                        surface.overlay.data.kill(false);
                    }
                }
                self.surface
                    .gui
                    .surfaces
                    .lock()
                    .retain(|s, _| *s == self.surface.output.global_id);
                let selecting = match self.surface.gui.screencast_session.phase.get() {
                    ScreencastPhase::Selecting(selecting) => selecting,
                    _ => return,
                };
                for gui in selecting.guis.lock().values() {
                    if gui.dpy.id != self.surface.gui.dpy.id {
                        gui.kill(false);
                    }
                }
                selecting
                    .guis
                    .lock()
                    .retain(|s, _| *s == self.surface.gui.dpy.id);
                self.surface.state.set(Rc::new(SelectionState::Select));
                let old = self
                    .surface
                    .overlay
                    .data
                    .content
                    .set(Some(create_select_gui(&self.surface)));
                if let Some(old) = old {
                    old.destroy();
                }
                self.surface.overlay.data.layout();
                self.surface.overlay.data.allocate_buffers();
            }
            ButtonRole::Reject | ButtonRole::Cancel => {
                log::info!("User has rejected the screencast request");
                self.surface.gui.screencast_session.kill();
            }
            ButtonRole::ShareAll | ButtonRole::ShareSelected => {
                let share_all = self.role == ButtonRole::ShareAll;
                let selecting = match self.surface.gui.screencast_session.phase.get() {
                    ScreencastPhase::Selecting(selecting) => selecting,
                    _ => return,
                };
                for gui in selecting.guis.lock().values() {
                    gui.kill(false);
                }
                selecting.guis.clear();
                let node = self.surface.gui.dpy.state.pw_con.create_client_node(&[
                    ("media.class".to_string(), "Video/Source".to_string()),
                    ("node.name".to_string(), "jay-desktop-portal".to_string()),
                    ("node.driver".to_string(), "true".to_string()),
                ]);
                let starting = Rc::new(StartingScreencast {
                    share_all,
                    session: self.surface.gui.screencast_session.clone(),
                    request_obj: selecting.request_obj.clone(),
                    reply: selecting.reply.clone(),
                    node,
                    dpy: self.surface.gui.dpy.clone(),
                    output: self.surface.output.clone(),
                });
                self.surface
                    .gui
                    .screencast_session
                    .phase
                    .set(ScreencastPhase::Starting(starting.clone()));
                starting.node.owner.set(Some(starting.clone()));
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
