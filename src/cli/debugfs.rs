use crate::cli::GlobalArgs;
use crate::tools::tool_client::Handle;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::jar_to_tar::JarToTarError;
use crate::utils::jar_to_tar::jar_to_tar;
use crate::wire::JayDebugfsId;
use crate::wire::jay_compositor;
use crate::wire::jay_debugfs;
use crate::wire::jay_debugfs_mount_request;
use crate::wire::jay_debugfs_snapshot;
use clap::Args;
use clap::Subcommand;
use clap_complete::ValueHint;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use std::cell::Cell;
use std::cell::LazyCell;
use std::future::pending;
use std::rc::Rc;
use std::time::SystemTime;
use thiserror::Error;
use uapi::c;

/// Inspects/manipulates the debug filesystem.
#[derive(Args, Debug)]
pub struct DebugfsArgs {
    #[clap(subcommand)]
    pub command: DebugfsCmd,
}

#[derive(Subcommand, Debug)]
pub enum DebugfsCmd {
    /// Creates an atomic .tar.gz snapshot of the filesystem.
    ///
    /// The global --json flag can be used to generate JSON files.
    Snapshot(SnapshotArgs),
    /// Mounts the debug filesystem and prints its path.
    ///
    /// This command is idempotent.
    Mount,
    /// Unmounts the debug filesystem.
    Unmount,
}

#[derive(Args, Debug)]
pub struct SnapshotArgs {
    /// The filename of the created archive.
    ///
    /// The filename must end in .tar.gz. If no filename is given, the archive is stored
    /// under jay-debugfs-<timestamp>.tar.gz in the current directory.
    ///
    /// If the filename is -, the archive is written to stdout.
    #[clap(value_hint = ValueHint::FilePath)]
    pub path: Option<String>,
}

pub fn main(global: GlobalArgs, args: DebugfsArgs) {
    with_tool_client(async move |tc| {
        let debugfs = Debugfs { tc: tc.clone() };
        debugfs.run(&global, args).await;
    });
}

struct Debugfs {
    tc: Rc<ToolClient>,
}

const SUFFIX: &str = ".tar.gz";

#[derive(Debug, Error)]
enum SnapshotError {
    #[error("Could not dup stdout")]
    DupStdout(#[source] OsError),
    #[error("Could not open file")]
    OpenFile(#[source] OsError),
    #[error("File must end in {SUFFIX}")]
    WrongSuffix,
    #[error("File name must not consist only of {SUFFIX}")]
    EmptyPrefix,
    #[error(transparent)]
    JarToTar(JarToTarError),
}

impl Debugfs {
    async fn run(self, global: &GlobalArgs, args: DebugfsArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let debugfs = tc.id();
        tc.send(jay_compositor::GetDebugfs {
            self_id: comp,
            id: debugfs,
        });
        match args.command {
            DebugfsCmd::Snapshot(args) => {
                if let Err(e) = self.snapshot(debugfs, global, args).await {
                    fatal!("{}", ErrorFmt(e));
                }
            }
            DebugfsCmd::Mount => self.mount(debugfs).await,
            DebugfsCmd::Unmount => {
                tc.send(jay_debugfs::Unmount { self_id: debugfs });
            }
        }
        tc.round_trip().await;
    }

    async fn snapshot(
        &self,
        debugfs: JayDebugfsId,
        global: &GlobalArgs,
        args: SnapshotArgs,
    ) -> Result<(), SnapshotError> {
        let tc = &self.tc;
        if let Err(e) = tc.ensure_same_exe().await {
            fatal!(
                "Could not ensure that compositor uses same executable: {}",
                ErrorFmt(e)
            );
        }
        let root_name = LazyCell::new(|| {
            let now = SystemTime::now();
            let mut s = format!("jay-debugfs-{}", humantime::format_rfc3339_millis(now));
            s.retain(|c| c != ':');
            s
        });
        let root;
        let out;
        let path = args
            .path
            .unwrap_or_else(|| format!("{}{SUFFIX}", *root_name));
        if path == "-" {
            root = root_name.as_str();
            out = uapi::fcntl_dupfd_cloexec(1, 0).map_os_err(SnapshotError::DupStdout)?;
        } else {
            let Some(prefix) = path.strip_suffix(SUFFIX) else {
                return Err(SnapshotError::WrongSuffix);
            };
            root = prefix.rsplit_once("/").map(|v| v.1).unwrap_or(prefix);
            if root.is_empty() {
                return Err(SnapshotError::EmptyPrefix);
            }
            out = uapi::open(
                &*path,
                c::O_WRONLY | c::O_CREAT | c::O_TRUNC | c::O_CLOEXEC,
                0o644,
            )
            .map_os_err(SnapshotError::OpenFile)?;
        }
        let id = tc.id();
        tc.send(jay_debugfs::CreateSnapshot {
            self_id: debugfs,
            id,
            root,
            json: global.json,
        });
        let fd = Rc::new(Cell::new(None));
        jay_debugfs_snapshot::Success::handle(tc, id, fd.clone(), |v, msg| {
            v.set(Some(msg.fd));
        });
        jay_debugfs_snapshot::Failure::handle(tc, id, (), |_, msg| {
            fatal!("Could not create snapshot: {}", msg.msg);
        });
        tc.round_trip().await;
        let fd = fd.take().unwrap();
        jar_to_tar(root, &fd, &out).map_err(SnapshotError::JarToTar)?;
        Ok(())
    }

    async fn mount(&self, debugfs: JayDebugfsId) {
        let tc = &self.tc;
        let id = tc.id();
        tc.send(jay_debugfs::Mount {
            self_id: debugfs,
            id,
        });
        use jay_debugfs_mount_request::*;
        Success::handle(tc, id, (), |_, msg| {
            println!("{}", msg.path);
            std::process::exit(0);
        });
        Failure::handle(tc, id, (), |_, _msg| {
            fatal!("Could not mount filesystem");
        });
        pending::<()>().await;
    }
}
