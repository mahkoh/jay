use crate::cli::{GenerateArgs, Jay};
use clap::CommandFactory;
use std::io::stdout;

pub fn main(args: GenerateArgs) {
    let stdout = stdout();
    let mut stdout = stdout.lock();
    clap_complete::generate(args.shell, &mut Jay::command(), "jay", &mut stdout);
}
