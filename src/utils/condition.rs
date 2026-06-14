use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub struct Condition {
    enabled: AtomicU64,
}

pub struct EnabledCondition {
    condition: &'static Condition,
}

impl Condition {
    #[allow(clippy::allow_attributes, dead_code)]
    pub fn enable(&'static self) -> EnabledCondition {
        self.enabled.fetch_add(1, Relaxed);
        EnabledCondition { condition: self }
    }

    #[allow(clippy::allow_attributes, dead_code)]
    pub fn enabled(&self) -> bool {
        self.enabled.load(Relaxed) > 0
    }
}

impl Drop for EnabledCondition {
    fn drop(&mut self) {
        self.condition.enabled.fetch_sub(1, Relaxed);
    }
}

macro_rules! ad_hoc_conditions {
    ($($ident:ident,)*) => {
        $(
            pub static $ident: Condition = Condition {
                enabled: AtomicU64::new(0),
            };
        )*
    };
}

ad_hoc_conditions! {}
