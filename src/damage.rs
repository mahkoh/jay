use {
    crate::{
        async_engine::AsyncEngine,
        rect::{Rect, Region},
        renderer::renderer_base::RendererBase,
        state::State,
        theme::Color,
        time::Time,
        utils::{asyncevent::AsyncEvent, errorfmt::ErrorFmt, timer::TimerFd},
    },
    isnt::std_1::primitive::IsntSliceExt,
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
            connector.connector.damage();
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
}

const MAX_RECTS: usize = 100_000;

struct Damage {
    time: Time,
    rect: Rect,
}

impl DamageVisualizer {
    pub fn new(eng: &Rc<AsyncEngine>) -> Self {
        Self {
            eng: eng.clone(),
            entries: Default::default(),
            entry_added: Default::default(),
            enabled: Default::default(),
            decay: Cell::new(Duration::from_secs(2)),
            color: Cell::new(Color::from_rgba_straight(255, 0, 0, 128)),
        }
    }

    #[allow(dead_code)]
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

    pub fn render(&self, cursor_rect: &Rect, renderer: &mut RendererBase<'_>) {
        if !self.enabled.get() {
            return;
        }
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
        let base_color = self.color.get();
        let mut used = Region::empty();
        let dx = -cursor_rect.x1();
        let dy = -cursor_rect.y1();
        let decay_millis = decay.as_millis() as u64 as f32;
        for entry in entries.iter().rev() {
            let region = Region::new(entry.rect);
            let region = region.subtract(&used);
            if region.is_not_empty() {
                let age = (now - entry.time).as_millis() as u64 as f32 / decay_millis;
                let color = base_color * (1.0 - age);
                renderer.fill_boxes2(region.rects(), &color, dx, dy);
                used = used.union(&region);
            }
        }
    }
}
