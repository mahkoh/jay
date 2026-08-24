use crate::async_engine::SpawnedFuture;
use crate::cli::GlobalArgs;
use crate::cli::MatchSource;
use crate::forker::ForkerError;
use crate::ifs::jay_client_trace::ClientTraceInfo;
use crate::io_uring::WriteVecCache;
use crate::tools::tool_client::Handle;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::client_trace::ClientTraceRead;
use crate::utils::client_trace::ClientTraceReadAvailable;
use crate::utils::clone3::Forked;
use crate::utils::clone3::fork_with_pidfd;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::pipe::Pipe;
use crate::utils::pipe::pipe;
use crate::utils::pread::Preader;
use crate::utils::queue::AsyncQueue;
use crate::utils::read_ext::ReadExt;
use crate::utils::rwf_flags::supports_rwf_nosignal;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::str_fmt::StrFmtFmt;
use crate::wire::JayClientTraceId;
use crate::wire::jay_client_trace;
use crate::wire::jay_compositor;
use crate::wire::jay_global_tracer;
use bincode::Options;
use clap::Args;
use clap::Subcommand;
use futures_util::future::Either;
use futures_util::future::select;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt;
use jay_algorithms::oserror::OsErrorExt2;
use jay_config::_private::bincode_ops;
use run_on_drop::on_drop;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::pending;
use std::os::unix::prelude::CommandExt;
use std::pin::pin;
use std::rc::Rc;
use std::sync::LazyLock;
use std::sync::OnceLock;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;
use uapi::isatty;
use uapi::raise;

/// Traces wayland messages.
///
/// This subcommand is documented in the book in the section titled
/// "Tracing Wayland Messages".
#[derive(Args, Debug)]
pub struct TraceArgs {
    /// The file prefix to store the output in, or a command to pipe the output to.
    #[clap(short, long)]
    output: Option<String>,
    /// Combine all clients into a single output stream, if the output parameter is used.
    #[clap(short, long, requires = "output")]
    combine: bool,
    /// Use raw wayland IDs instead of unique IDs.
    #[clap(long)]
    raw_ids: bool,
    #[clap(subcommand)]
    cmd: TraceCmd,
}

#[derive(Subcommand, Debug)]
enum TraceCmd {
    /// Trace all clients.
    All,
    /// Trace the client with a given ID.
    Id(TraceIdArgs),
    /// Interactively select a window and trace its client.
    SelectWindow,
    /// Trace clients matching a toml client match.
    Match(MatchSource),
}

#[derive(Args, Debug)]
struct TraceIdArgs {
    /// The ID of the client.
    id: u64,
}

pub fn main(global: GlobalArgs, trace_args: TraceArgs) {
    with_tool_client(|tc| async move {
        let trace = Rc::new(CliTrace { tc: tc.clone() });
        trace.run(&global, trace_args).await;
    });
}

struct CliTrace {
    tc: Rc<ToolClient>,
}

struct Tracer {
    tc: Rc<ToolClient>,
    format: Format,
    model: Model,
    cache: WriteVecCache<String>,
    futures: CopyHashMap<JayClientTraceId, SpawnedFuture<()>>,
    todo: AsyncQueue<Todo>,
    disconnected: AsyncEvent,
    raw_ids: bool,
    single_client: bool,
}

#[derive(Debug)]
enum Model {
    Combined(Rc<OwnedFd>),
    Separate(String),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Format {
    Jsonl,
    Text,
}

impl Format {
    fn suffix(self) -> &'static str {
        match self {
            Format::Jsonl => "jsonl",
            Format::Text => "txt",
        }
    }
}

struct SingleTrace {
    tracer: Rc<Tracer>,
    trace: JayClientTraceId,
    client_id: u64,
    src: RefCell<ClientTraceRead>,
    available: ClientTraceReadAvailable,
}

enum Todo {
    Connect {
        info: ClientTraceInfo<'static>,
    },
    Disconnect {
        client_id: u64,
    },
    Trace {
        trace: Rc<SingleTrace>,
        wake: Rc<AsyncEvent>,
        disconnected: bool,
    },
}

impl CliTrace {
    async fn run(&self, global: &GlobalArgs, args: TraceArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        if let Err(e) = tc.ensure_same_exe().await {
            fatal!(
                "Could not ensure that compositor uses same executable: {}",
                ErrorFmt(e)
            );
        }
        let format = global.json.then_some(Format::Jsonl).unwrap_or(Format::Text);
        let model = match args.output {
            None => {
                let stdout = uapi::fcntl_dupfd_cloexec(1, 0).map(Rc::new).to_os_error();
                let stdout = match stdout {
                    Ok(s) => s,
                    Err(e) => {
                        fatal!("Could not dup stdout: {}", ErrorFmt(e));
                    }
                };
                Model::Combined(stdout)
            }
            Some(p) => match args.combine {
                false => Model::Separate(p),
                true => {
                    let fd = match create_out_fd(format, None, &p) {
                        Ok(r) => r,
                        Err(e) => {
                            fatal!("{}", ErrorFmt(e));
                        }
                    };
                    Model::Combined(fd)
                }
            },
        };
        let tracer = Rc::new(Tracer {
            tc: tc.clone(),
            format,
            model,
            futures: Default::default(),
            cache: Default::default(),
            todo: Default::default(),
            disconnected: Default::default(),
            raw_ids: args.raw_ids,
            single_client: matches!(args.cmd, TraceCmd::Id(_) | TraceCmd::SelectWindow),
        });
        let handle_single = |client_id| {
            let trace = tc.id();
            tc.send(jay_compositor::TraceClient {
                self_id: comp,
                trace,
                client_id,
            });
            tracer.handle_trace(trace);
        };
        let handle_match = |m: MatchSource| {
            let m = m.parse_client_match(tc, false);
            let m = tc.create_client_match(comp, &m);
            let trace = tc.id();
            tc.send(jay_compositor::TraceClients {
                self_id: comp,
                clients: trace,
                client_match: m,
            });
            jay_global_tracer::ClientTrace::handle(tc, trace, tracer.clone(), |tracer, msg| {
                tracer.handle_trace(msg.id);
            });
        };
        match args.cmd {
            TraceCmd::Id(id) => handle_single(id.id),
            TraceCmd::SelectWindow => {
                let id = tc.select_toplevel_client().await;
                if id == 0 {
                    fatal!("Could not select a client");
                }
                handle_single(id);
            }
            TraceCmd::Match(m) => handle_match(m),
            TraceCmd::All => handle_match(MatchSource {
                expr: Some(String::new()),
                file: None,
            }),
        };
        let eng = &self.tc.eng;
        let _f1 = eng.spawn("todos", tracer.clone().handle_todos());
        let _f2 = eng.spawn("disco", tracer.clone().handle_disconnected());
        pending().await
    }
}

impl Tracer {
    async fn handle_todos(self: Rc<Self>) {
        if let Model::Combined(fd) = &self.model {
            let color = self.format == Format::Text && isatty(fd.raw()).is_ok();
            self.flush_todos(fd, color).await
        }
    }

    async fn handle_disconnected(self: Rc<Self>) {
        if self.single_client {
            self.disconnected.triggered().await;
            std::process::exit(0);
        }
    }

    fn handle_trace(self: &Rc<Self>, trace: JayClientTraceId) {
        let err = Rc::new(Cell::new(None));
        jay_client_trace::Failed::handle(&self.tc, trace, err.clone(), |err, msg| {
            err.set(Some(format!("Could not create a tracer: {}", msg.msg)));
        });
        let disconnected = Rc::new(AsyncEvent::default());
        jay_client_trace::Disconnected::handle(&self.tc, trace, disconnected.clone(), |d, _| {
            d.trigger();
        });
        let info = Rc::new(Cell::new(None));
        jay_client_trace::Info::handle(&self.tc, trace, info.clone(), |info, msg| {
            info.set(Some(msg.fd));
        });
        let storage = Rc::new(Cell::new(None));
        jay_client_trace::Storage::handle(&self.tc, trace, storage.clone(), |storage, msg| {
            storage.set(Some(msg.fd));
        });
        let destroy = on_drop({
            let slf = self.clone();
            move || {
                slf.futures.remove(&trace);
                slf.tc.send(jay_client_trace::Destroy { self_id: trace });
            }
        });
        let slf = self.clone();
        let fut = async move {
            let _destroy = destroy;
            slf.tc.round_trip().await;
            macro_rules! error {
                ($($tt:tt)*) => {
                    if slf.single_client {
                        fatal!($($tt)*);
                    } else {
                        log::error!($($tt)*);
                        return;
                    }
                };
            }
            if let Some(e) = err.take() {
                error!("{}", e);
            }
            let info = info.take().unwrap();
            let info = match Preader::new(info).read_to_vec() {
                Ok(i) => i,
                Err(e) => {
                    error!("Could not read info fd: {}", ErrorFmt(e));
                }
            };
            let info = match bincode_ops().deserialize::<ClientTraceInfo<'static>>(&info) {
                Ok(i) => i,
                Err(e) => {
                    error!("Could not deserialize client trace info: {}", ErrorFmt(e));
                }
            };
            let memfd = storage.take().unwrap();
            let src = unsafe { ClientTraceRead::new(&slf.tc.ring, &memfd, slf.raw_ids) };
            let src = match src {
                Ok(src) => src,
                Err(e) => {
                    error!("Could not create ClientTraceRead: {}", ErrorFmt(e));
                }
            };
            let client_id = info.id;
            log::info!("Tracing client {client_id}");
            let trace = SingleTrace {
                tracer: slf.clone(),
                trace,
                client_id,
                available: src.available(),
                src: RefCell::new(src),
            };
            match &slf.model {
                Model::Combined(_) => {
                    slf.todo.push(Todo::Connect { info });
                    let trace = Rc::new(trace);
                    trace.handle_shared(&disconnected).await;
                    slf.todo.push(Todo::Disconnect { client_id });
                }
                Model::Separate(p) => {
                    let fd = match create_out_fd(slf.format, Some(info.id), p) {
                        Ok(fd) => fd,
                        Err(e) => error!("{}", ErrorFmt(e)),
                    };
                    let mut buf = String::new();
                    slf.fmt_connected(&mut buf, &info);
                    slf.flush(&fd, &mut buf).await;
                    trace.handle_dedicated(&disconnected, &mut buf, &fd).await;
                    slf.fmt_disconnected(&mut buf, client_id);
                    slf.flush(&fd, &mut buf).await;
                    slf.disconnected.trigger();
                }
            }
            log::info!("Detaching client {client_id}");
        };
        let fut = self.tc.eng.spawn("client-trace", fut);
        self.futures.set(trace, fut);
    }

    async fn flush_todos(&self, sink: &Rc<OwnedFd>, color: bool) {
        let mut buf = String::new();
        let mut todos = VecDeque::new();
        loop {
            todos.clear();
            self.todo.non_empty().await;
            self.todo.swap(&mut todos);
            let mut disconnected = false;
            for todo in &todos {
                match todo {
                    Todo::Connect { info } => {
                        self.fmt_connected(&mut buf, info);
                    }
                    Todo::Disconnect { client_id } => {
                        self.fmt_disconnected(&mut buf, *client_id);
                        disconnected = true;
                    }
                    Todo::Trace {
                        trace,
                        wake,
                        disconnected,
                    } => {
                        let limit = !disconnected;
                        trace.fmt_messages(&mut buf, color, limit);
                        wake.trigger();
                    }
                }
            }
            self.flush(&sink, &mut buf).await;
            if disconnected {
                self.disconnected.trigger();
            }
        }
    }

    async fn flush(&self, sink: &Rc<OwnedFd>, buf: &mut String) -> bool {
        let is_combined = matches!(self.model, Model::Combined(_));
        let want_sigpipe = self.single_client || is_combined;
        let mask_sigpipe = !want_sigpipe && supports_rwf_nosignal();
        let res = self
            .tc
            .ring
            .write_vec_all(sink, mask_sigpipe, &self.cache, buf)
            .await;
        if let Err(e) = &res {
            log::warn!("Could not write to tracer: {}", ErrorFmt(e));
        }
        buf.clear();
        if res.is_err() && is_combined {
            let _ = raise(c::SIGPIPE);
        }
        res.is_err()
    }

    fn fmt_connected(&self, buf: &mut String, info: &ClientTraceInfo) {
        match self.format {
            Format::Jsonl => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Jsonl,
                    ..Default::default()
                };
                buf.push_str(r#"{"t":"n","cl":"#);
                info.id.str_fmt(buf, &ctx);
                buf.push_str(r#","info":"#);
                info.str_fmt(buf, &ctx);
                buf.push_str("}\n");
            }
            Format::Text => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Human,
                    prefix: "> ",
                    ..Default::default()
                };
                buf.push_str("Tracing client ");
                info.id.str_fmt(buf, &ctx);
                buf.push_str("\n");
                buf.push_str(ctx.prefix);
                info.str_fmt(buf, &ctx);
                buf.push_str("\n");
            }
        }
    }

    fn fmt_disconnected(&self, buf: &mut String, client_id: u64) {
        match self.format {
            Format::Jsonl => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Jsonl,
                    ..Default::default()
                };
                buf.push_str(r#"{"t":"d","cl":"#);
                client_id.str_fmt(buf, &ctx);
                buf.push_str("}\n");
            }
            Format::Text => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Human,
                    prefix: "> ",
                    ..Default::default()
                };
                buf.push_str("Detaching client ");
                client_id.str_fmt(buf, &ctx);
                buf.push_str("\n");
            }
        }
    }

    fn fmt_missed(&self, buf: &mut String, client_id: u64, missed: u64) {
        match self.format {
            Format::Jsonl => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Jsonl,
                    ..Default::default()
                };
                buf.push_str(r#"{"t":"x","cl":"#);
                client_id.str_fmt(buf, &ctx);
                buf.push_str(r#","n":"#);
                missed.str_fmt(buf, &ctx);
                buf.push_str("}\n");
            }
            Format::Text => {
                let ctx = StrCtx {
                    fmt: StrFmtFmt::Human,
                    prefix: "> ",
                    ..Default::default()
                };
                buf.push_str("Missed ");
                missed.str_fmt(buf, &ctx);
                buf.push_str(" messages from client ");
                client_id.str_fmt(buf, &ctx);
                buf.push_str("\n");
            }
        }
    }
}

#[derive(Debug, Error)]
enum OutFdError {
    #[error("$SHELL is not set")]
    Shell,
    #[error("Could not create a pipe")]
    CreatePipe(#[source] OsError),
    #[error("Could not fork")]
    Fork(#[source] ForkerError),
    #[error("Could not open file for writing")]
    OpenWrite(#[source] OsError),
}

fn create_out_fd(format: Format, id: Option<u64>, p: &str) -> Result<Rc<OwnedFd>, OutFdError> {
    if let Some(cmd) = p.strip_prefix("|") {
        static SHELL: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("SHELL").ok());
        let Some(shell) = &*SHELL else {
            return Err(OutFdError::Shell);
        };
        let Pipe { read, write } = pipe().map_err(OutFdError::CreatePipe)?;
        static IGNORE_SIGCHLD: OnceLock<()> = OnceLock::new();
        IGNORE_SIGCHLD.get_or_init(|| unsafe {
            c::signal(c::SIGCHLD, c::SIG_IGN);
        });
        match fork_with_pidfd(false).map_err(OutFdError::Fork)? {
            Forked::Parent { .. } => return Ok(Rc::new(write)),
            Forked::Child { .. } => {
                unsafe {
                    c::signal(c::SIGCHLD, c::SIG_DFL);
                }
                let mut command = std::process::Command::new(shell);
                command.arg("-c").arg(cmd).stdin(read);
                const JAY_CLIENT_ID: &str = "JAY_CLIENT_ID";
                if let Some(id) = id {
                    command.env(JAY_CLIENT_ID, id.to_string());
                } else {
                    command.env_remove(JAY_CLIENT_ID);
                }
                let res = command.exec();
                log::error!("Could not exec {shell}: {}", ErrorFmt(res));
                std::process::exit(1);
            }
        }
    } else {
        let suffix = format.suffix();
        let path = match id {
            None => format!("{p}.{suffix}"),
            Some(id) => format!("{p}.{id}.{suffix}"),
        };
        uapi::open(
            path.as_str(),
            c::O_WRONLY | c::O_CREAT | c::O_TRUNC | c::O_CLOEXEC,
            0o644,
        )
        .map(Rc::new)
        .map_os_err(OutFdError::OpenWrite)
    }
}

impl SingleTrace {
    async fn await_messages(
        &self,
        disconnected: &Rc<AsyncEvent>,
        mut on_messages: impl AsyncFnMut(bool),
    ) {
        let mut disconnected = pin!(disconnected.triggered());
        loop {
            let disconnected = {
                let available = pin!(self.available.available());
                let res = select(disconnected.as_mut(), available).await;
                match res {
                    Either::Left(_) => true,
                    Either::Right((Err(e), _)) => {
                        log::error!("futex_wait failed: {}", ErrorFmt(e));
                        true
                    }
                    Either::Right(_) => false,
                }
            };
            on_messages(disconnected).await;
            if disconnected {
                break;
            }
        }
    }

    fn fmt_messages(&self, string: &mut String, color: bool, limit: bool) {
        const MAX_MSGS: usize = 1024;
        let mut n = 0;
        let src = &mut *self.src.borrow_mut();
        let ctx = Default::default();
        let format = self.tracer.format;
        while let Some(msg) = src.try_read() {
            if msg.missed > 0 {
                self.tracer.fmt_missed(string, self.client_id, msg.missed);
            }
            if let Some(msg) = &msg.msg {
                match format {
                    Format::Jsonl => {
                        msg.fmt_jsonl(string, &ctx, self.client_id);
                    }
                    Format::Text => {
                        msg.fmt_text(string, &ctx, color, self.client_id);
                    }
                }
            }
            n += 1;
            if n >= MAX_MSGS && limit {
                break;
            }
        }
    }

    async fn handle_shared(self: &Rc<Self>, disconnected: &Rc<AsyncEvent>) {
        let wake = Rc::new(AsyncEvent::default());
        self.await_messages(disconnected, async |disconnected| {
            self.tracer.todo.push(Todo::Trace {
                trace: self.clone(),
                wake: wake.clone(),
                disconnected,
            });
            if !disconnected {
                wake.triggered().await;
            }
        })
        .await;
    }

    async fn handle_dedicated(
        &self,
        disconnected: &Rc<AsyncEvent>,
        buf: &mut String,
        sink: &Rc<OwnedFd>,
    ) {
        self.await_messages(disconnected, async |disconnected| {
            let limit = !disconnected;
            self.fmt_messages(buf, false, limit);
            let err = self.tracer.flush(sink, buf).await;
            if err {
                self.tracer.futures.remove(&self.trace);
                self.tracer.tc.eng.yield_now().await;
            }
        })
        .await;
    }
}
