mod clients;
mod color;
mod color_management;
mod damage_tracking;
mod duration;
mod generate;
mod idle;
mod input;
mod log;
mod quit;
mod randr;
mod reexec;
mod run_privileged;
pub mod screenshot;
mod seat_test;
mod set_log_level;
mod tree;
mod unlock;
mod version;
mod xwayland;

use {
    crate::{
        cli::{
            clients::ClientsArgs, color_management::ColorManagementArgs,
            damage_tracking::DamageTrackingArgs, idle::IdleCmd, input::InputArgs, randr::RandrArgs,
            reexec::ReexecArgs, tree::TreeArgs, xwayland::XwaylandArgs,
        },
        compositor::start_compositor,
        format::{Format, ref_formats},
        portal,
        pr_caps::drop_all_pr_caps,
    },
    ::log::Level,
    clap::{Args, Parser, Subcommand, ValueEnum, ValueHint, builder::PossibleValue},
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
    #[clap(value_enum, long, default_value_t)]
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
    /// Unlocks the compositor.
    Unlock,
    /// Take a screenshot.
    Screenshot(ScreenshotArgs),
    /// Inspect/modify the idle (screensaver) settings.
    Idle(IdleArgs),
    /// Run a privileged program.
    RunPrivileged(RunPrivilegedArgs),
    /// Tests the events produced by a seat.
    SeatTest(SeatTestArgs),
    /// Run the desktop portal.
    Portal,
    /// Inspect/modify graphics card and connector settings.
    Randr(RandrArgs),
    /// Inspect/modify input settings.
    Input(InputArgs),
    /// Modify damage tracking settings. (Only for debugging.)
    #[clap(hide = true)]
    DamageTracking(DamageTrackingArgs),
    /// Inspect/modify xwayland settings.
    Xwayland(XwaylandArgs),
    /// Inspect/modify the color-management settings.
    ColorManagement(ColorManagementArgs),
    /// Replace the compositor by another process. (Only for development.)
    #[clap(hide = true)]
    Reexec(ReexecArgs),
    /// Inspect/manipulate the connected clients.
    Clients(ClientsArgs),
    /// Inspect the surface tree.
    Tree(TreeArgs),
    /// Prints the Jay version and exits.
    Version,
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

#[derive(Args, Debug)]
pub struct RunPrivilegedArgs {
    /// The program to run
    #[clap(required = true, trailing_var_arg = true, value_hint = ValueHint::CommandWithArguments)]
    pub program: Vec<String>,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, Default, PartialEq)]
pub enum ScreenshotFormat {
    /// The PNG image format.
    #[default]
    Png,
    /// The QOI image format.
    Qoi,
}

#[derive(Args, Debug)]
pub struct ScreenshotArgs {
    /// The format to use for the image.
    #[clap(value_enum, long, default_value_t)]
    pub format: ScreenshotFormat,
    /// The filename of the saved screenshot
    ///
    /// If no filename is given, the screenshot will be saved under %Y-%m-%d-%H%M%S_jay.<ext>
    /// in the current directory.
    ///
    /// The filename can contain the usual strftime parameters.
    #[clap(value_hint = ValueHint::FilePath)]
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
    #[clap(value_enum, use_value_delimiter = true, long)]
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
    #[clap(value_enum)]
    level: CliLogLevel,
}

#[derive(Args, Debug)]
pub struct SeatTestArgs {
    /// Test all seats.
    #[clap(long, short = 'a')]
    all: bool,
    /// The seat to test.
    seat: Option<String>,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CliBackend {
    X11,
    Metal,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash)]
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
    #[clap(value_enum)]
    shell: Shell,
}

impl ValueEnum for &'static Format {
    fn value_variants<'a>() -> &'a [Self] {
        ref_formats()
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name))
    }
}

pub fn main() {
    let cli = Jay::parse();
    if not_matches!(cli.command, Cmd::Run(_)) {
        drop_all_pr_caps();
    }
    match cli.command {
        Cmd::Run(a) => start_compositor(cli.global, a),
        Cmd::GenerateCompletion(g) => generate::main(g),
        Cmd::Log(a) => log::main(cli.global, a),
        Cmd::Quit => quit::main(cli.global),
        Cmd::SetLogLevel(a) => set_log_level::main(cli.global, a),
        Cmd::Screenshot(a) => screenshot::main(cli.global, a),
        Cmd::Idle(a) => idle::main(cli.global, a),
        Cmd::Unlock => unlock::main(cli.global),
        Cmd::RunPrivileged(a) => run_privileged::main(cli.global, a),
        Cmd::SeatTest(a) => seat_test::main(cli.global, a),
        Cmd::Portal => portal::run_freestanding(cli.global),
        Cmd::Randr(a) => randr::main(cli.global, a),
        Cmd::Input(a) => input::main(cli.global, a),
        Cmd::DamageTracking(a) => damage_tracking::main(cli.global, a),
        Cmd::Xwayland(a) => xwayland::main(cli.global, a),
        Cmd::ColorManagement(a) => color_management::main(cli.global, a),
        Cmd::Clients(a) => clients::main(cli.global, a),
        Cmd::Tree(a) => tree::main(cli.global, a),
        Cmd::Version => version::main(cli.global),
        #[cfg(feature = "it")]
        Cmd::RunTests => crate::it::run_tests(),
        Cmd::Reexec(a) => reexec::main(cli.global, a),
    }
}
