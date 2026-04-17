use uapi::c;

cfg_select! {
    target_env = "musl" => {
        pub type IoctlNumber = c::c_int;
        pub type IovLength = c::c_int;
    }
    _ => {
        pub type IoctlNumber = c::c_ulong;
        pub type IovLength = usize;
    }
}
