use {
    crate::{
        ifs::wl_seat::{BTN_LEFT, wl_pointer::PRESSED},
        portal::{
            ptl_display::{PortalDisplay, PortalOutput, PortalSeat},
            ptl_screencast::{
                PortalSession, ScreencastPhase, ScreencastTarget, SelectingWindowScreencast,
                SelectingWorkspaceScreencast,
            },
            ptr_gui::{
                Align, Button, ButtonOwner, Flow, GuiElement, Label, Orientation, OverlayWindow,
                OverlayWindowOwner,
            },
        },
        theme::Color,
        utils::{copyhashmap::CopyHashMap, hash_map_ext::HashMapExt},
        wl_usr::usr_ifs::{
            usr_jay_select_toplevel::UsrJaySelectToplevelOwner,
            usr_jay_select_workspace::UsrJaySelectWorkspaceOwner, usr_jay_toplevel::UsrJayToplevel,
            usr_jay_workspace::UsrJayWorkspace,
        },
    },
    std::rc::Rc,
};

const H_MARGIN: f32 = 30.0;
const V_MARGIN: f32 = 20.0;

pub struct SelectionGui {
    screencast_session: Rc<PortalSession>,
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
    Restore,
    Accept,
    SelectWorkspace,
    SelectWindow,
    Reject,
}

impl SelectionGui {
    pub fn kill(&self, upwards: bool) {
        for surface in self.surfaces.lock().drain_values() {
            surface.overlay.data.kill(false);
        }
        if let ScreencastPhase::Selecting(s) = self.screencast_session.sc_phase.get() {
            s.guis.remove(&self.dpy.id);
            if upwards && s.guis.is_empty() {
                self.screencast_session.kill();
            }
        }
    }
}

fn create_accept_gui(surface: &Rc<SelectionGuiSurface>, for_restore: bool) -> Rc<dyn GuiElement> {
    let app = &surface.gui.screencast_session.app;
    let text = if app.is_empty() {
        format!("An application wants to capture the screen")
    } else {
        format!("`{}` wants to capture the screen", app)
    };
    let label = Rc::new(Label::default());
    *label.text.borrow_mut() = text;
    let restore_button = static_button(surface, ButtonRole::Restore, "Restore Session");
    let accept_button = static_button(surface, ButtonRole::Accept, "Share This Output");
    let workspace_button = static_button(surface, ButtonRole::SelectWorkspace, "Share A Workspace");
    let window_button = static_button(surface, ButtonRole::SelectWindow, "Share A Window");
    let reject_button = static_button(surface, ButtonRole::Reject, "Reject");
    for button in [
        &restore_button,
        &accept_button,
        &workspace_button,
        &window_button,
        &reject_button,
    ] {
        button.border_color.set(Color::from_gray_srgb(100));
        button.border.set(2.0);
        button.padding.set(5.0);
    }
    restore_button.bg_color.set(Color::from_srgb(170, 170, 200));
    restore_button
        .bg_hover_color
        .set(Color::from_srgb(170, 170, 255));
    for button in [&accept_button, &workspace_button, &window_button] {
        button.bg_color.set(Color::from_srgb(170, 200, 170));
        button.bg_hover_color.set(Color::from_srgb(170, 255, 170));
    }
    reject_button.bg_color.set(Color::from_srgb(200, 170, 170));
    reject_button
        .bg_hover_color
        .set(Color::from_srgb(255, 170, 170));
    let flow = Rc::new(Flow::default());
    flow.orientation.set(Orientation::Vertical);
    flow.cross_align.set(Align::Center);
    flow.in_margin.set(V_MARGIN);
    flow.cross_margin.set(H_MARGIN);
    let mut elements: Vec<Rc<dyn GuiElement>> = vec![label];
    if for_restore {
        elements.push(restore_button);
    }
    elements.push(accept_button);
    if surface.gui.dpy.jc.caps.select_workspace.get() {
        elements.push(workspace_button);
    }
    if surface.gui.dpy.jc.caps.window_capture.get() {
        elements.push(window_button);
    }
    elements.push(reject_button);
    *flow.elements.borrow_mut() = elements;
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
    pub fn new(ss: &Rc<PortalSession>, dpy: &Rc<PortalDisplay>, for_restore: bool) -> Rc<Self> {
        let gui = Rc::new(SelectionGui {
            screencast_session: ss.clone(),
            dpy: dpy.clone(),
            surfaces: Default::default(),
        });
        for output in dpy.outputs.lock().values() {
            let sgs = Rc::new(SelectionGuiSurface {
                gui: gui.clone(),
                output: output.clone(),
                overlay: OverlayWindow::new(output),
            });
            let element = create_accept_gui(&sgs, for_restore);
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
    fn button(&self, seat: &PortalSeat, button: u32, state: u32) {
        if button != BTN_LEFT || state != PRESSED {
            return;
        }
        match self.role {
            ButtonRole::Restore
            | ButtonRole::Accept
            | ButtonRole::SelectWorkspace
            | ButtonRole::SelectWindow => {
                log::info!("User has accepted the request");
                let selecting = match self.surface.gui.screencast_session.sc_phase.get() {
                    ScreencastPhase::Selecting(selecting) => selecting,
                    _ => return,
                };
                for gui in selecting.guis.lock().drain_values() {
                    gui.kill(false);
                }
                let dpy = &self.surface.output.dpy;
                if self.role == ButtonRole::Restore {
                    selecting.core.session.screencast_restore(
                        &selecting.core.request_obj,
                        selecting.restore_data.take().map(Ok),
                        Some(self.surface.gui.dpy.clone()),
                    );
                } else if self.role == ButtonRole::Accept {
                    selecting
                        .core
                        .starting(dpy, ScreencastTarget::Output(self.surface.output.clone()));
                } else if self.role == ButtonRole::SelectWorkspace {
                    let selector = dpy.jc.select_workspace(&seat.wl);
                    let selecting = Rc::new(SelectingWorkspaceScreencast {
                        core: selecting.core.clone(),
                        dpy: dpy.clone(),
                        selector: selector.clone(),
                    });
                    selector.owner.set(Some(selecting.clone()));
                    self.surface
                        .gui
                        .screencast_session
                        .sc_phase
                        .set(ScreencastPhase::SelectingWorkspace(selecting));
                } else {
                    let selector = dpy.jc.select_toplevel(&seat.wl);
                    let selecting = Rc::new(SelectingWindowScreencast {
                        core: selecting.core.clone(),
                        dpy: dpy.clone(),
                        selector: selector.clone(),
                        restoring: false,
                    });
                    selector.owner.set(Some(selecting.clone()));
                    self.surface
                        .gui
                        .screencast_session
                        .sc_phase
                        .set(ScreencastPhase::SelectingWindow(selecting));
                }
            }
            ButtonRole::Reject => {
                log::info!("User has rejected the screencast request");
                self.surface.gui.screencast_session.kill();
            }
        }
    }
}

impl UsrJaySelectToplevelOwner for SelectingWindowScreencast {
    fn done(&self, tl: Option<Rc<UsrJayToplevel>>) {
        let Some(tl) = tl else {
            if self.restoring {
                log::warn!("Could not restore session because toplevel no longer exists");
                self.core
                    .session
                    .start_interactive_selection(&self.core.request_obj, None);
                return;
            }
            log::info!("User has aborted the selection");
            self.core.session.kill();
            return;
        };
        match self.core.session.sc_phase.get() {
            ScreencastPhase::SelectingWindow(s) => {
                self.dpy.con.remove_obj(&*s.selector);
            }
            _ => {
                self.dpy.con.remove_obj(&*tl);
                return;
            }
        }
        log::info!("User has selected a window");
        self.core
            .starting(&self.dpy, ScreencastTarget::Toplevel(tl));
    }
}

impl UsrJaySelectWorkspaceOwner for SelectingWorkspaceScreencast {
    fn done(&self, output: u32, ws: Option<Rc<UsrJayWorkspace>>) {
        let Some(ws) = ws else {
            log::info!("User has aborted the selection");
            self.core.session.kill();
            return;
        };
        match self.core.session.sc_phase.get() {
            ScreencastPhase::SelectingWorkspace(s) => {
                self.dpy.con.remove_obj(&*s.selector);
            }
            _ => {
                self.dpy.con.remove_obj(&*ws);
                return;
            }
        }
        log::info!("User has selected a workspace");
        let output = match self.dpy.outputs.get(&output) {
            Some(o) => o,
            _ => {
                log::warn!("Workspace does not belong to any known output");
                self.dpy.con.remove_obj(&*ws);
                self.core.session.kill();
                return;
            }
        };
        self.core
            .starting(&self.dpy, ScreencastTarget::Workspace(output, ws, true));
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
