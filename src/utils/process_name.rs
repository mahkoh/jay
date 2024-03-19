use uapi::{c, IntoUstr};

pub fn set_process_name(name: &str) {
    unsafe {
        let name = name.into_ustr();
        c::prctl(c::PR_SET_NAME, name.as_ptr());
    }
}
