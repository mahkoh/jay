use {
    crate::{
        async_engine::AsyncEngine,
        cmm::cmm_manager::ColorManager,
        fixed::Fixed,
        ifs::wl_output::WlOutputGlobal,
        rect::{Rect, Region},
        renderer::renderer_base::RendererBase,
        state::State,
        theme::Color,
        time::Time,
        utils::{
            asyncevent::AsyncEvent, errorfmt::ErrorFmt, timer::TimerFd, transform_ext::TransformExt,
        },
    },
    isnt::std_1::primitive::IsntSliceExt,
    jay_config::video::Transform,
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        rc::Rc,
        time::Duration,
    },
    uapi::c::CLOCK_MONOTONIC,
};

pub async fn visualize_damage(state: Rc<State>) {
    let timer = match TimerFd::new(CLOCK_MONOTONIC) {
        Ok(t) => t,
        Err(e) => {
            log::error!("Could not create timer fd: {}", ErrorFmt(e));
            return;
        }
    };
    loop {
        state.damage_visualizer.entry_added.triggered().await;
        let duration = Duration::from_millis(50);
        let res = timer.program(Some(duration), Some(duration));
        if let Err(e) = res {
            log::error!("Could not program timer: {}", ErrorFmt(e));
            return;
        }
        loop {
            let res = timer.expired(&state.ring).await;
            if let Err(e) = res {
                log::error!("Could not wait for timer to expire: {}", ErrorFmt(e));
                return;
            }
            if state.damage_visualizer.entries.borrow_mut().is_empty() {
                break;
            }
            damage_all(&state);
        }
        let res = timer.program(None, None);
        if let Err(e) = res {
            log::error!("Could not disable timer: {}", ErrorFmt(e));
            return;
        }
    }
}

fn damage_all(state: &State) {
    for connector in state.connectors.lock().values() {
        if connector.connected.get() {
            let damage = &mut *connector.damage.borrow_mut();
            damage.clear();
            damage.push(connector.damage_intersect.get());
            connector.damage();
        }
    }
}

pub struct DamageVisualizer {
    eng: Rc<AsyncEngine>,
    entries: RefCell<VecDeque<Damage>>,
    entry_added: AsyncEvent,
    enabled: Cell<bool>,
    decay: Cell<Duration>,
    color: Cell<Color>,
    color_manager: Rc<ColorManager>,
}

const MAX_RECTS: usize = 100_000;

struct Damage {
    time: Time,
    rect: Rect,
}

impl DamageVisualizer {
    pub fn new(eng: &Rc<AsyncEngine>, color_manager: &Rc<ColorManager>) -> Self {
        Self {
            eng: eng.clone(),
            entries: Default::default(),
            entry_added: Default::default(),
            enabled: Default::default(),
            decay: Cell::new(Duration::from_secs(2)),
            color: Cell::new(Color::from_srgba_straight(255, 0, 0, 128)),
            color_manager: color_manager.clone(),
        }
    }

    pub fn add(&self, rect: Rect) {
        if !self.enabled.get() {
            return;
        }
        let entries = &mut *self.entries.borrow_mut();
        if entries.is_empty() {
            self.entry_added.trigger();
        }
        entries.push_back(Damage {
            time: self.eng.now(),
            rect,
        });
        if entries.len() > MAX_RECTS {
            entries.pop_front();
        }
    }

    pub fn set_enabled(&self, state: &State, enabled: bool) {
        self.enabled.set(enabled);
        if !enabled {
            self.entries.borrow_mut().clear();
            damage_all(state);
        }
    }

    pub fn set_decay(&self, decay: Duration) {
        let millis = decay.as_millis();
        if millis == 0 || millis > u64::MAX as u128 {
            return;
        }
        self.decay.set(decay);
    }

    pub fn set_color(&self, color: Color) {
        self.color.set(color);
    }

    fn trim(&self) {
        let now = self.eng.now();
        let entries = &mut *self.entries.borrow_mut();
        let decay = self.decay.get();
        while let Some(first) = entries.front() {
            if now - first.time >= decay {
                entries.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn render(&self, cursor_rect: &Rect, renderer: &mut RendererBase<'_>) {
        if !self.enabled.get() {
            return;
        }
        self.trim();
        let now = self.eng.now();
        let entries = &*self.entries.borrow();
        let decay = self.decay.get();
        let base_color = self.color.get();
        let mut used = Region::default();
        let dx = -cursor_rect.x1();
        let dy = -cursor_rect.y1();
        let decay_millis = decay.as_millis() as u64 as f32;
        renderer.sync();
        let srgb = &self.color_manager.srgb_gamma22().linear;
        for entry in entries.iter().rev() {
            let region = Region::new(entry.rect);
            let region = region.subtract_cow(&used);
            if region.is_not_empty() {
                let age = (now - entry.time).as_millis() as u64 as f32 / decay_millis;
                let color = base_color * (1.0 - age);
                renderer.fill_boxes2(region.rects(), &color, srgb, dx, dy);
                used = used.union_cow(&region).into_owned();
            }
        }
    }

    pub fn copy_damage(&self, output: &WlOutputGlobal) {
        if !self.enabled.get() {
            return;
        }
        self.trim();
        let entries = &*self.entries.borrow();
        let pos = output.pos.get();
        for entry in entries {
            if entry.rect.intersects(&pos) {
                output.add_damage_area(&entry.rect);
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DamageMatrix {
    transform: Transform,
    mx: f64,
    my: f64,
    dx: f64,
    dy: f64,
    smear: i32,
}

impl Default for DamageMatrix {
    fn default() -> Self {
        Self {
            transform: Default::default(),
            mx: 1.0,
            my: 1.0,
            dx: 0.0,
            dy: 0.0,
            smear: 0,
        }
    }
}

impl DamageMatrix {
    pub fn apply(&self, dx: i32, dy: i32, rect: Rect) -> Rect {
        let x1 = rect.x1() - self.smear;
        let x2 = rect.x2() + self.smear;
        let y1 = rect.y1() - self.smear;
        let y2 = rect.y2() + self.smear;
        let [x1, y1, x2, y2] = match self.transform {
            Transform::None => [x1, y1, x2, y2],
            Transform::Rotate90 => [-y2, x1, -y1, x2],
            Transform::Rotate180 => [-x2, -y2, -x1, -y1],
            Transform::Rotate270 => [y1, -x2, y2, -x1],
            Transform::Flip => [-x2, y1, -x1, y2],
            Transform::FlipRotate90 => [y1, x1, y2, x2],
            Transform::FlipRotate180 => [x1, -y2, x2, -y1],
            Transform::FlipRotate270 => [-y2, -x2, -y1, -x1],
        };
        let x1 = (x1 as f64 * self.mx + self.dx).floor() as i32 + dx;
        let y1 = (y1 as f64 * self.my + self.dy).floor() as i32 + dy;
        let x2 = (x2 as f64 * self.mx + self.dx).ceil() as i32 + dx;
        let y2 = (y2 as f64 * self.my + self.dy).ceil() as i32 + dy;
        Rect::new_saturating(x1, y1, x2, y2)
    }

    pub fn new(
        transform: Transform,
        legacy_scale: i32,
        buffer_width: i32,
        buffer_height: i32,
        viewport: Option<[Fixed; 4]>,
        dst_width: i32,
        dst_height: i32,
    ) -> DamageMatrix {
        let mut buffer_width = buffer_width as f64;
        let mut buffer_height = buffer_height as f64;
        let dst_width = dst_width as f64;
        let dst_height = dst_height as f64;

        let mut mx = 1.0;
        let mut my = 1.0;
        if legacy_scale != 1 {
            let scale_inv = 1.0 / (legacy_scale as f64);
            mx = scale_inv;
            my = scale_inv;
            buffer_width *= scale_inv;
            buffer_height *= scale_inv;
        }
        let (mut buffer_width, mut buffer_height) =
            transform.maybe_swap((buffer_width, buffer_height));
        let (mut dx, mut dy) = match transform {
            Transform::None => (0.0, 0.0),
            Transform::Rotate90 => (buffer_width, 0.0),
            Transform::Rotate180 => (buffer_width, buffer_height),
            Transform::Rotate270 => (0.0, buffer_height),
            Transform::Flip => (buffer_width, 0.0),
            Transform::FlipRotate90 => (0.0, 0.0),
            Transform::FlipRotate180 => (0.0, buffer_height),
            Transform::FlipRotate270 => (buffer_width, buffer_height),
        };
        if let Some([x, y, w, h]) = viewport {
            dx -= x.to_f64();
            dy -= y.to_f64();
            buffer_width = w.to_f64();
            buffer_height = h.to_f64();
        }
        let mut smear = false;
        if dst_width != buffer_width {
            let scale = dst_width / buffer_width;
            mx *= scale;
            dx *= scale;
            smear |= dst_width > buffer_width;
        }
        if dst_height != buffer_height {
            let scale = dst_height / buffer_height;
            my *= scale;
            dy *= scale;
            smear |= dst_height > buffer_height;
        }
        DamageMatrix {
            transform,
            mx,
            my,
            dx,
            dy,
            smear: smear as _,
        }
    }
}
