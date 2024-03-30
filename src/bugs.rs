use {ahash::AHashMap, once_cell::sync::Lazy};

static BUGS: Lazy<AHashMap<&'static str, Bugs>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    map.insert(
        "chromium",
        Bugs {
            respect_min_max_size: true,
            ..Default::default()
        },
    );
    map.insert(
        "Alacritty",
        Bugs {
            min_size: Some((100, 100)),
            ..Default::default()
        },
    );
    map
});

pub fn get(app_id: &str) -> &'static Bugs {
    BUGS.get(app_id).unwrap_or(&NONE)
}

pub static NONE: Bugs = Bugs {
    respect_min_max_size: false,
    min_size: None,
};

#[derive(Default, Debug)]
pub struct Bugs {
    pub respect_min_max_size: bool,
    pub min_size: Option<(i32, i32)>,
}
