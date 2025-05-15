use {cfg_if::cfg_if, uapi::c};

cfg_if! {
    if #[cfg(target_env = "musl")] {
        pub type IoctlNumber = c::c_int;
        pub type IovLength = c::c_int;
    } else {
        pub type IoctlNumber = c::c_ulong;
        pub type IovLength = usize;
    }
}
