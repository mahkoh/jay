use {
    crate::{
        control_center::cc_idle::IdlePane,
        egui_adapter::egui_platform::{EggError, EggWindow, EggWindowOwner},
        state::State,
        utils::numcell::NumCell,
    },
    egui::{
        Align, CentralPanel, Color32, Context, CursorIcon, Frame, Label, Layout, Rgba, ScrollArea,
        Sense, SidePanel, Stroke, Ui, UiBuilder, Visuals, WidgetText,
    },
    egui_material_icons::icons::{ICON_CLOSE, ICON_DRAG_INDICATOR},
    egui_tiles::{ResizeState, TabState, Tile, TileId, Tiles, Tree},
    std::{cell::RefCell, mem, rc::Rc},
    thiserror::Error,
};

mod cc_idle;
mod cc_sidebar;

#[derive(Debug, Error)]
pub enum ControlCenterError {
    #[error("Could not get the egg context")]
    GetEggContext(#[source] EggError),
}

linear_ids!(ControlCenterIds, ControlCenterId, u64);

pub struct ControlCenter {
    inner: Rc<ControlCenterInner>,
}

struct ControlCenterInner {
    id: ControlCenterId,
    state: Rc<State>,
    tree: RefCell<Settings>,
    window: Rc<EggWindow>,
    next_pane_id: NumCell<u64>,
}

struct Settings {
    tree: Tree<Pane>,
}

struct Pane {
    id: u64,
    ty: PaneType,
}

enum PaneType {
    Idle(IdlePane),
}

struct TreeBehavior<'a> {
    #[expect(dead_code)]
    state: &'a Rc<State>,
    close: Option<TileId>,
}

fn icon_label(icon: &str) -> Label {
    Label::new(icon).selectable(false)
}

impl Pane {
    fn title(&self, res: &mut String) {
        match &self.ty {
            PaneType::Idle(i) => i.title(res),
        }
    }
}

impl egui_tiles::Behavior<Pane> for TreeBehavior<'_> {
    fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        let mut drag = false;
        Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                drag = ui
                    .add(icon_label(ICON_DRAG_INDICATOR).sense(Sense::drag()))
                    .total_drag_delta()
                    .map(|d| d.length() >= 5.0)
                    .unwrap_or(false);
                ui.add_space(5.0);
                let mut title = String::new();
                pane.title(&mut title);
                ui.label(title);
                ui.add_space(5.0);
                if ui
                    .add(icon_label(ICON_CLOSE).sense(Sense::click()))
                    .clicked()
                {
                    self.close = Some(tile_id);
                }
            });
            ui.add_space(5.0);
            ui.separator();
            ui.add_space(5.0);
            let ui = &mut ui.new_child(
                UiBuilder::new()
                    .layout(Layout::top_down(Align::LEFT).with_cross_justify(true))
                    .id(("pane", pane.id)),
            );
            ScrollArea::vertical().show(ui, |ui| match &mut pane.ty {
                PaneType::Idle(p) => p.show(ui),
            });
        });
        if drag {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }

    fn tab_title_for_pane(&mut self, _pane: &Pane) -> WidgetText {
        "".into()
    }

    fn tab_hover_cursor_icon(&self) -> CursorIcon {
        CursorIcon::Default
    }

    fn tab_title_for_tile(&mut self, tiles: &Tiles<Pane>, tile_id: TileId) -> WidgetText {
        fn add_title(tiles: &Tiles<Pane>, res: &mut String, first: &mut bool, tile_id: TileId) {
            if !mem::take(first) {
                res.push_str("/");
            }
            let Some(tile) = tiles.get(tile_id) else {
                res.push_str("MISSING TILE");
                return;
            };
            match tile {
                Tile::Pane(p) => p.title(res),
                Tile::Container(c) => {
                    let mut first = true;
                    for &tile_id in c.children() {
                        add_title(tiles, res, &mut first, tile_id);
                    }
                }
            }
        }
        let mut res = String::new();
        let mut first = true;
        add_title(tiles, &mut res, &mut first, tile_id);
        res.into()
    }

    fn resize_stroke(&self, style: &egui::Style, resize_state: ResizeState) -> Stroke {
        match resize_state {
            ResizeState::Idle => style.visuals.widgets.noninteractive.bg_stroke,
            ResizeState::Hovering => style.visuals.widgets.hovered.fg_stroke,
            ResizeState::Dragging => style.visuals.widgets.active.fg_stroke,
        }
    }

    fn tab_bar_color(&self, visuals: &Visuals) -> Color32 {
        (Rgba::from(visuals.panel_fill) * Rgba::from_gray(0.8)).into()
    }

    fn tab_bg_color(
        &self,
        visuals: &Visuals,
        _tiles: &Tiles<Pane>,
        _tile_id: TileId,
        state: &TabState,
    ) -> Color32 {
        match state.active {
            true => visuals.panel_fill,
            false => self.tab_bar_color(visuals),
        }
    }
}

impl EggWindowOwner for ControlCenterInner {
    fn close(&self) {
        self.close();
    }

    fn render(self: Rc<Self>, ctx: &Context) {
        let settings = &mut *self.tree.borrow_mut();
        SidePanel::left("sidebar").show(ctx, |ui| self.show_sidebar(&mut settings.tree, ui));
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ctx.style())
                    .outer_margin(0.0)
                    .inner_margin(0.0),
            )
            .show(ctx, |ui| {
                let mut behavior = TreeBehavior {
                    state: &self.state,
                    close: Default::default(),
                };
                settings.tree.ui(&mut behavior, ui);
                if let Some(close) = behavior.close {
                    settings.tree.tiles.remove(close);
                }
            });
    }
}

impl State {
    pub fn spawn_control_center(self: &Rc<Self>) -> Result<Rc<ControlCenter>, ControlCenterError> {
        let ctx = self
            .get_egg_context()
            .map_err(ControlCenterError::GetEggContext)?;
        let window = ctx.create_window("Control Center");
        let cc = Rc::new(ControlCenter {
            inner: Rc::new(ControlCenterInner {
                id: self.control_center_ids.next(),
                window,
                state: self.clone(),
                tree: RefCell::new(Settings {
                    tree: Tree::new_tabs("abcd", vec![]),
                }),
                next_pane_id: Default::default(),
            }),
        });
        cc.inner.window.set_owner(Some(cc.inner.clone()));
        self.control_centers.set(cc.inner.id, cc.clone());
        Ok(cc)
    }
}

impl ControlCenterInner {
    fn close(&self) {
        self.window.set_owner(None);
        self.tree.borrow_mut().tree = Tree::empty("");
        self.state.control_centers.remove(&self.id);
    }
}

impl Drop for ControlCenter {
    fn drop(&mut self) {
        self.inner.close();
    }
}
