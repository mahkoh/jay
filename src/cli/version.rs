use crate::{cli::GlobalArgs, version::VERSION};

pub fn main(_global: GlobalArgs) {
    println!("{VERSION}");
}
