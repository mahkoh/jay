#![allow(
    clippy::mem_replace_with_default,
    clippy::comparison_chain,
    clippy::collapsible_else_if,
    clippy::needless_lifetimes,
    clippy::needless_late_init,
    clippy::should_implement_trait
)]

pub mod jar;
pub mod lut;
pub mod mmap;
pub mod oserror;
pub mod qoi;
pub mod rect;
pub mod tar;
pub mod tf;
pub mod triangles;
mod windows;
