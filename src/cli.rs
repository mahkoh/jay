use clap::{ArgEnum, Args, Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(flatten)]
    pub global: GlobalArgs,
    #[clap(subcommand)]
    pub command: Cmd,
}

#[derive(Args, Debug)]
pub struct GlobalArgs {
    #[clap(long)]
    hurr: String,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Run the compositor
    Run,
    Test(Test),
}

#[derive(Args, Debug)]
pub struct Test {
    /// a
    ///
    /// b
    ///
    /// c
    #[clap(long, use_value_delimiter = true, arg_enum)]
    shell: Vec<Hurr>,
}

#[derive(ArgEnum, Debug, Copy, Clone)]
pub enum Hurr {
    Bash,
    Fish,
    Zsh,
}
