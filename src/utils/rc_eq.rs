use std::rc::Rc;

pub fn rc_eq<T: ?Sized>(a: &Rc<T>, b: &Rc<T>) -> bool {
    Rc::as_ptr(a) as *const u8 == Rc::as_ptr(b) as *const u8
}
