use {
    crate::utils::{clonecell::CloneCell, syncqueue::SyncQueue},
    std::{
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct OnChange<T> {
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
    pub events: SyncQueue<T>,
}

impl<T> OnChange<T> {
    pub fn send_event(&self, event: T) {
        self.events.push(event);
        if let Some(cb) = self.on_change.get() {
            cb();
        }
    }
}

impl<T> Default for OnChange<T> {
    fn default() -> Self {
        Self {
            on_change: Default::default(),
            events: Default::default(),
        }
    }
}

impl<T> Debug for OnChange<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.on_change.get() {
            None => f.write_str("None"),
            Some(_) => f.write_str("Some"),
        }
    }
}
