pub const XRD: &str = "XDG_RUNTIME_DIR";

pub fn xrd() -> Option<String> {
    std::env::var(XRD).ok()
}
