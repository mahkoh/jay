#![allow(dead_code)]

use std::{cell::Cell, future::Future};

#[derive(Default)]
pub struct Once {
    done: Cell<bool>,
}

impl Once {
    pub fn set(&self) -> bool {
        !self.done.replace(true)
    }

    pub fn exec<F: FnOnce()>(&self, f: F) {
        if !self.done.replace(true) {
            f();
        }
    }

    pub async fn exec_async<G: Future<Output = ()>, F: FnOnce() -> G>(&self, f: F) {
        if !self.done.replace(true) {
            f().await;
        }
    }
}
