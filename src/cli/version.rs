use crate::{
    cli::{GlobalArgs, json::jsonl},
    version::VERSION,
};

pub fn main(global: GlobalArgs) {
    if global.json {
        jsonl(&VERSION);
    } else {
        println!("{VERSION}");
    }
}
