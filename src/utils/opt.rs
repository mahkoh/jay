use crate::utils::clonecell::CloneCell;
use derivative::Derivative;
use std::rc::Rc;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct Opt<T> {
    t: CloneCell<Option<Rc<T>>>,
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
