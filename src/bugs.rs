use {ahash::AHashMap, once_cell::sync::Lazy};

static BUGS: Lazy<AHashMap<&'static str, Bugs>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    map.insert(
        "chromium",
        Bugs {
            respect_min_max_size: true,
        },
    );
    map
});

pub fn get(app_id: &str) -> &'static Bugs {
    BUGS.get(app_id).unwrap_or(&NONE)
}

pub static NONE: Bugs = Bugs {
    respect_min_max_size: false,
};

#[derive(Debug)]
pub struct Bugs {
    pub respect_min_max_size: bool,
}
