use {ahash::AHashMap, once_cell::sync::Lazy};

static APP_ID_BUGS: Lazy<AHashMap<&'static str, Bugs>> = Lazy::new(|| {
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
            min_width: Some(100),
            min_height: Some(100),
            ..Default::default()
        },
    );
    map
});

static COMM_BUGS: Lazy<AHashMap<&'static str, Bugs>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    map.insert(
        "bemenu-run",
        Bugs {
            immediate_configure: true,
            ..Default::default()
        },
    );
    map
});

pub fn get_by_app_id(app_id: &str) -> &'static Bugs {
    APP_ID_BUGS.get(app_id).unwrap_or(&NONE)
}

pub fn get_by_comm(comm: &str) -> &'static Bugs {
    COMM_BUGS.get(comm).unwrap_or(&NONE)
}

pub static NONE: Bugs = Bugs {
    respect_min_max_size: false,
    min_width: None,
    min_height: None,
    immediate_configure: false,
};

#[derive(Default, Debug)]
pub struct Bugs {
    pub respect_min_max_size: bool,
    pub min_width: Option<i32>,
    pub min_height: Option<i32>,
    pub immediate_configure: bool,
}
