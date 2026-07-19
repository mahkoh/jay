use crate::builder::BuilderError;
use crate::builder::handle_file;
use std::env::args;

mod ast;
mod builder;
mod parser;

fn main() -> Result<(), BuilderError> {
    for xml in args().skip(1) {
        handle_file(&xml)?;
    }
    Ok(())
}
