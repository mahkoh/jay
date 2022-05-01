mod generate;
mod idle;
mod log;
mod quit;
pub mod screenshot;
mod set_log_level;

use {
    crate::compositor::start_compositor,
    ::log::Level,
    clap::{ArgEnum, Args, Parser, Subcommand},
    clap_complete::Shell,
};

/// A wayland compositor.
#[derive(Parser, Debug)]
struct Jay {
    #[clap(flatten)]
    global: GlobalArgs,
    #[clap(subcommand)]
    command: Cmd,
}

#[derive(Args, Debug)]
pub struct GlobalArgs {
    /// The log level.
    #[clap(arg_enum, long, default_value_t)]
    pub log_level: CliLogLevel,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Run the compositor.
    Run(RunArgs),
    /// Generate shell completion scripts for jay.
    GenerateCompletion(GenerateArgs),
    /// Open the log file.
    Log(LogArgs),
    /// Sets the log level.
    SetLogLevel(SetLogArgs),
    /// Stop the compositor.
    Quit,
    /// Take a screenshot.
    Screenshot(ScreenshotArgs),
    /// Inspect/modify the idle (screensaver) settings.
    Idle(IdleArgs),
    #[cfg(feature = "it")]
    RunTests,
}

#[derive(Args, Debug)]
pub struct IdleArgs {
    /// The filename of the saved screenshot
    ///
    /// If no filename is given, the screenshot will be saved under %Y-%m-%d-%H%M%S_jay.qoi
    /// in the current directory.
    ///
    /// The filename can contain the usual strftime parameters.
    #[clap(subcommand)]
    pub command: Option<IdleCmd>,
}

#[derive(Subcommand, Debug)]
pub enum IdleCmd {
    /// Print the idle status.
    Status,
    /// Set the idle interval.
    Set(IdleSetArgs),
}

impl Default for IdleCmd {
    fn default() -> Self {
        Self::Status
    }
}

#[derive(Args, Debug)]
pub struct IdleSetArgs {
    /// The interval of inactivity after which to disable the screens.
    ///
    /// This can be either a number in minutes and seconds or the keyword `disabled` to
    /// disable the screensaver.
    ///
    /// Minutes and seconds can be specified in any of the following formats:
    ///
    /// * 1m
    /// * 1m5s
    /// * 1m 5s
    /// * 1min 5sec
    /// * 1 minute 5 seconds
    #[clap(verbatim_doc_comment, required = true)]
    pub interval: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ScreenshotArgs {
    /// The filename of the saved screenshot
    ///
    /// If no filename is given, the screenshot will be saved under %Y-%m-%d-%H%M%S_jay.qoi
    /// in the current directory.
    ///
    /// The filename can contain the usual strftime parameters.
    pub filename: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct RunArgs {
    /// The backends to try.
    ///
    /// By default, jay will try to start the available backends in this order: x11,metal.
    /// The first backend that can be started will be used.
    ///
    /// Using this option, you can change which backends will be tried and change the order in
    /// which they will be tried. Multiple backends can be supplied as a comma-separated list.
    #[clap(arg_enum, use_value_delimiter = true, long)]
    pub backends: Vec<CliBackend>,
}

#[derive(Args, Debug)]
pub struct LogArgs {
    /// Print the path of the log file.
    #[clap(long)]
    path: bool,
    /// Follow the log.
    #[clap(long, short)]
    follow: bool,
    /// Immediately jump to the end in the pager.
    #[clap(long, short = 'e')]
    pager_end: bool,
}

#[derive(Args, Debug)]
pub struct SetLogArgs {
    /// The new log level.
    #[clap(arg_enum)]
    level: CliLogLevel,
}

#[derive(ArgEnum, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CliBackend {
    X11,
    Metal,
}

#[derive(ArgEnum, Debug, Copy, Clone, Hash)]
pub enum CliLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Into<Level> for CliLogLevel {
    fn into(self) -> Level {
        match self {
            CliLogLevel::Trace => Level::Trace,
            CliLogLevel::Debug => Level::Debug,
            CliLogLevel::Info => Level::Info,
            CliLogLevel::Warn => Level::Warn,
            CliLogLevel::Error => Level::Error,
        }
    }
}

impl Default for CliLogLevel {
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// The shell to generate completions for
    #[clap(arg_enum)]
    shell: Shell,
}

pub fn main() {
    let cli = Jay::parse();
    match cli.command {
        Cmd::Run(a) => start_compositor(cli.global, a),
        Cmd::GenerateCompletion(g) => generate::main(g),
        Cmd::Log(a) => log::main(cli.global, a),
        Cmd::Quit => quit::main(cli.global),
        Cmd::SetLogLevel(a) => set_log_level::main(cli.global, a),
        Cmd::Screenshot(a) => screenshot::main(cli.global, a),
        Cmd::Idle(a) => idle::main(cli.global, a),
        #[cfg(feature = "it")]
        Cmd::RunTests => crate::it::run_tests(),
    }
}
