use crate::cli::GlobalArgs;
use crate::cli::json::jsonl;
use crate::version::VERSION;

pub fn main(global: GlobalArgs) {
    if global.json {
        jsonl(&VERSION);
    } else {
        println!("{VERSION}");
    }
}
