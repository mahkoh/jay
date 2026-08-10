use derivative::Derivative;
use std::cell::OnceCell;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct WeakOnceCell<T> {
    cell: OnceCell<Weak<T>>,
}

impl<T> WeakOnceCell<T> {
    pub fn get_or_init(&self, init: impl FnOnce() -> T, handle_rc: impl FnOnce(Rc<T>)) -> Weak<T> {
        if let Some(v) = self.cell.get() {
            return v.clone();
        }
        let rc = Rc::new_cyclic(|weak| {
            let _ = self.cell.set(weak.clone());
            init()
        });
        let weak = Rc::downgrade(&rc);
        handle_rc(rc);
        weak
    }
}
