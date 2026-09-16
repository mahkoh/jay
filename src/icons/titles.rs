use crate::gfx_api::GfxContext;
use crate::icons::IconState;
use crate::icons::Icons;
use crate::icons::IconsError;
use crate::icons::OVERLAY_PATH;
use crate::icons::PathBuilderExt;
use crate::icons::SizedTitleIcons;
use crate::icons::calculate_accents;
use crate::icons::create_icon;
use crate::scale::Scale;
use crate::scale::ScaleIndex;
use crate::theme::Color;
use crate::utils::clonecell::CloneCell;
use crate::utils::copy_vec::CopyVec;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fx_hash::FxBuildHasher;
use crate::utils::numcell::NumCell;
use jay_proc::jay_hash;
use linearize::static_map;
use std::rc::Rc;
use std::sync::LazyLock;
use tiny_skia::Path;
use tiny_skia::PathBuilder;

pub(super) struct Titles {
    inner: Rc<TitlesInner>,
    default: TitleIconsUser,
}

#[derive(Default)]
struct TitlesInner {
    ctx: CloneCell<Option<Rc<dyn GfxContext>>>,
    scales: CopyHashMap<(Scale, ScaleIndex), (), FxBuildHasher>,
    users_by_key: CopyHashMap<TitleIconKey, Rc<TitleIcons>, FxBuildHasher>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
pub struct TitleIconKey {
    pub title_height: i32,
    pub unfocused_title_background: Color,
    pub focused_title_background: Color,
    pub attention_requested_background: Color,
    pub focused_inactive_title_background: Color,
}

pub struct TitleIconsUser {
    inner: Rc<TitlesInner>,
    icons: CloneCell<Rc<TitleIcons>>,
}

impl Clone for TitleIconsUser {
    fn clone(&self) -> Self {
        let icons = self.icons.get();
        icons.users.add_fetch(1);
        Self {
            inner: self.inner.clone(),
            icons: CloneCell::new(icons),
        }
    }
}

struct TitleIcons {
    users: NumCell<usize>,
    key: TitleIconKey,
    by_scale: CopyVec<Option<Rc<SizedTitleIcons>>>,
}

impl Default for Titles {
    fn default() -> Self {
        let inner = Rc::new(TitlesInner::default());
        let icons = inner.acquire(TitleIconKey {
            title_height: 0,
            unfocused_title_background: Color::TRANSPARENT,
            focused_title_background: Color::TRANSPARENT,
            attention_requested_background: Color::TRANSPARENT,
            focused_inactive_title_background: Color::TRANSPARENT,
        });
        let default = TitleIconsUser {
            inner: inner.clone(),
            icons: CloneCell::new(icons),
        };
        Self { inner, default }
    }
}

impl TitleIcons {
    fn get(&self, idx: ScaleIndex) -> Option<Rc<SizedTitleIcons>> {
        self.by_scale.get(idx.0)?
    }
}

impl TitleIconsUser {
    pub fn get(&self, idx: ScaleIndex) -> Option<Rc<SizedTitleIcons>> {
        self.icons.get().get(idx)
    }
}

impl Icons {
    pub fn title_user(&self) -> TitleIconsUser {
        self.titles.default.clone()
    }
}

impl TitlesInner {
    fn acquire(&self, key: TitleIconKey) -> Rc<TitleIcons> {
        let icons = match self.users_by_key.get(&key) {
            Some(v) => v,
            None => {
                let users = Rc::new(TitleIcons {
                    users: Default::default(),
                    key,
                    by_scale: Default::default(),
                });
                let ctx = self.ctx.get();
                for &(scale, idx) in self.scales.lock().keys() {
                    users.update(scale, idx, ctx.as_ref());
                }
                self.users_by_key.set(key, users.clone());
                users
            }
        };
        icons.users.add_fetch(1);
        icons
    }
}

impl TitleIconsUser {
    fn release(&self, icons: &TitleIcons) {
        if icons.users.sub_fetch(1) == 0 {
            self.inner.users_by_key.remove(&icons.key);
        }
    }

    pub fn set_key(&self, key: TitleIconKey) {
        let old = self.icons.get();
        if old.key == key {
            return;
        }
        self.release(&old);
        let icons = self.inner.acquire(key);
        self.icons.set(icons);
    }
}

impl Drop for TitleIconsUser {
    fn drop(&mut self) {
        let icons = self.icons.get();
        self.release(&icons);
    }
}

impl Titles {
    pub(super) fn add_scale(&self, scale: Scale, idx: ScaleIndex) {
        self.inner.scales.set((scale, idx), ());
        let ctx = self.inner.ctx.get();
        self.update(scale, idx, ctx.as_ref());
    }

    pub(super) fn remove_scale(&self, scale: Scale, idx: ScaleIndex) {
        self.inner.scales.remove(&(scale, idx));
        self.update(scale, idx, None);
    }

    pub(super) fn set_render_ctx(&self, ctx: Option<&Rc<dyn GfxContext>>) {
        self.inner.ctx.set(ctx.cloned());
        for &(scale, idx) in self.inner.scales.lock().keys() {
            self.update(scale, idx, ctx);
        }
    }

    fn update(&self, scale: Scale, idx: ScaleIndex, ctx: Option<&Rc<dyn GfxContext>>) {
        for users in self.inner.users_by_key.lock().values() {
            users.update(scale, idx, ctx);
        }
    }
}

impl TitleIcons {
    fn update(&self, scale: Scale, idx: ScaleIndex, ctx: Option<&Rc<dyn GfxContext>>) {
        let sized = ctx.and_then(|ctx| {
            create_title_icons(scale, &self.key, ctx)
                .inspect_err(|e| {
                    log::error!(
                        "Could not create icons for scale {scale} and key {:?}: {}",
                        self.key,
                        ErrorFmt(e),
                    );
                })
                .ok()?
                .map(Rc::new)
        });
        self.by_scale.set(idx.0, sized);
    }
}

fn create_title_icons(
    scale: Scale,
    key: &TitleIconKey,
    ctx: &Rc<dyn GfxContext>,
) -> Result<Option<SizedTitleIcons>, IconsError> {
    let [size] = scale.pixel_size([key.title_height]);
    if size <= 0 {
        return Ok(None);
    }
    let size = size as u32;

    let create_icon = |path: &Path, color: tiny_skia::Color| create_icon(size, ctx, path, color);
    let create_pins = |color: Color| {
        let colors = calculate_accents(color);
        Ok(static_map! {
            IconState::Passive => create_icon(&PIN_PATH, colors[0])?,
            IconState::Active => create_icon(&PIN_PATH, colors[1])?,
        })
    };
    let create_overlay = |color: Color| {
        let colors = calculate_accents(color);
        create_icon(&OVERLAY_PATH, colors[0])
    };

    Ok(Some(SizedTitleIcons {
        pin_unfocused_title: create_pins(key.unfocused_title_background)?,
        pin_focused_title: create_pins(key.focused_title_background)?,
        pin_attention_requested: create_pins(key.attention_requested_background)?,
        overlay_unfocused_title: create_overlay(key.unfocused_title_background)?,
        overlay_attention_requested: create_overlay(key.attention_requested_background)?,
        overlay_focused_title: create_overlay(key.focused_title_background)?,
        overlay_focused_inactive_title: create_overlay(key.focused_inactive_title_background)?,
    }))
}

static PIN_PATH: LazyLock<Path> = LazyLock::new(|| {
    let cx = 50.0f32;
    let cy = 40.0f32;
    let r = 30.0f32;
    let xx = cx;
    let xy = 90.0f32;
    let d = xy - cy;
    let v1 = r / d * (d * d - r * r).sqrt();
    let v2 = 1.0 / d * (d * d - r * r);

    let mut path = PathBuilder::new();
    path.move_to(cx, cy - r);
    path.arc_cw_to(cx, cy, cx + r, cy);
    path.arc_cw_to(cx, cy, xx + v1, xy - v2);
    path.line_to(xx, xy);
    path.line_to(xx - v1, xy - v2);
    path.arc_cw_to(cx, cy, cx - r, cy);
    path.arc_cw_to(cx, cy, cx, cy - r);
    path.close();
    path.push_circle(cx, cy, r / 2.5);
    path.finish().unwrap()
});

#[test]
fn pin_path() {
    let _path = &*PIN_PATH;
}
