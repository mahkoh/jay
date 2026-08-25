use std::rc::Rc;

thread_local! {
    static RC: &'static Rc<()> = Box::leak(Box::new(Rc::new(())));
}

pub fn static_rc() -> &'static Rc<()> {
    RC.with(|v| *v)
}
