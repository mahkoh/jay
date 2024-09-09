use std::cell::Cell;

pub struct GeometricDecay {
    p1: f64,
    p2: f64,
    v: Cell<f64>,
}

impl GeometricDecay {
    #[expect(dead_code)]
    pub fn new(mut p1: f64, reset: u64) -> Self {
        if p1.is_nan() || p1 < 0.01 {
            p1 = 0.01;
        }
        if p1 > 0.99 {
            p1 = 0.99;
        }
        let p2 = 1.0 - p1;
        Self {
            p1,
            p2,
            v: Cell::new(reset as f64 / p1),
        }
    }

    #[expect(dead_code)]
    pub fn reset(&self, v: u64) {
        self.v.set(v as f64 / self.p1);
    }

    #[expect(dead_code)]
    pub fn get(&self) -> u64 {
        (self.p1 * self.v.get()) as u64
    }

    #[expect(dead_code)]
    pub fn add(&self, n: u64) {
        let v = n as f64 + self.p2 * self.v.get();
        self.v.set(v);
    }
}
