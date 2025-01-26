use {crate::utils::clonecell::CloneCell, std::rc::Rc};

pub struct Opt<T> {
    t: CloneCell<Option<Rc<T>>>,
}

impl<T> Default for Opt<T> {
    fn default() -> Self {
        Self {
            t: Default::default(),
        }
    }
}

impl<T> Opt<T> {
    pub fn set(&self, t: Option<Rc<T>>) {
        self.t.set(t);
    }

    pub fn get(&self) -> Option<Rc<T>> {
        self.t.get()
    }

    pub fn is_none(&self) -> bool {
        self.t.is_none()
    }
}
