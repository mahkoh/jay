use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{JayClientQueryId, jay_client_query, jay_compositor},
    },
    ahash::AHashMap,
    clap::{Args, Subcommand},
    std::{cell::RefCell, mem, rc::Rc},
    uapi::c,
};

#[derive(Args, Debug)]
pub struct ClientsArgs {
    #[clap(subcommand)]
    cmd: Option<ClientsCmd>,
}

#[derive(Subcommand, Debug)]
enum ClientsCmd {
    /// Show information about clients.
    Show(ShowArgs),
    /// Disconnect a client.
    Kill(KillArgs),
}

#[derive(Args, Debug)]
struct ShowArgs {
    #[clap(subcommand)]
    cmd: ShowCmd,
}

#[derive(Subcommand, Debug)]
enum ShowCmd {
    /// Show all clients.
    All,
    /// Show a client with a given ID.
    Id(ShowIdArgs),
    /// Interactively select a window and show information about its client.
    SelectWindow,
}

#[derive(Args, Debug)]
struct ShowIdArgs {
    /// The ID of the client.
    id: u64,
}

#[derive(Args, Debug)]
struct KillArgs {
    #[clap(subcommand)]
    cmd: KillCmd,
}

#[derive(Subcommand, Debug)]
enum KillCmd {
    /// Kill the client with a given ID.
    Id(KillIdArgs),
    /// Interactively select a window and kill its client.
    SelectWindow,
}

#[derive(Args, Debug)]
struct KillIdArgs {
    /// The ID of the client.
    id: u64,
}

pub fn main(global: GlobalArgs, clients_args: ClientsArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let clients = Rc::new(Clients { tc: tc.clone() });
        clients.run(clients_args).await;
    });
}

struct Clients {
    tc: Rc<ToolClient>,
}

impl Clients {
    async fn run(&self, args: ClientsArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let cmd = args
            .cmd
            .unwrap_or(ClientsCmd::Show(ShowArgs { cmd: ShowCmd::All }));
        match cmd {
            ClientsCmd::Show(a) => {
                let id = tc.id();
                tc.send(jay_compositor::CreateClientQuery { self_id: comp, id });
                match a.cmd {
                    ShowCmd::All => {
                        tc.send(jay_client_query::AddAll { self_id: id });
                    }
                    ShowCmd::Id(a) => {
                        tc.send(jay_client_query::AddId {
                            self_id: id,
                            id: a.id,
                        });
                    }
                    ShowCmd::SelectWindow => {
                        let client_id = tc.select_toplevel_client().await;
                        if client_id == 0 {
                            fatal!("Did not select a window");
                        }
                        tc.send(jay_client_query::AddId {
                            self_id: id,
                            id: client_id,
                        });
                    }
                }
                tc.send(jay_client_query::Execute { self_id: id });
                let clients = handle_client_query(tc, id).await;
                let mut clients = clients.values().collect::<Vec<_>>();
                clients.sort_by_key(|c| c.id);
                let mut prefix = "    ".to_string();
                let mut printer = ClientPrinter {
                    prefix: &mut prefix,
                };
                for client in clients {
                    println!("- client:");
                    printer.print_client(client);
                }
            }
            ClientsCmd::Kill(a) => match a.cmd {
                KillCmd::Id(id) => {
                    tc.send(jay_compositor::KillClient {
                        self_id: comp,
                        id: id.id,
                    });
                }
                KillCmd::SelectWindow => {
                    let client_id = tc.select_toplevel_client().await;
                    if client_id == 0 {
                        fatal!("Did not select a window");
                    }
                    tc.send(jay_compositor::KillClient {
                        self_id: comp,
                        id: client_id,
                    });
                }
            },
        }
        tc.round_trip().await;
    }
}

#[derive(Default)]
pub struct Client {
    pub id: u64,
    pub sandboxed: bool,
    pub sandbox_engine: Option<String>,
    pub sandbox_app_id: Option<String>,
    pub sandbox_instance_id: Option<String>,
    pub uid: Option<c::uid_t>,
    pub pid: Option<c::pid_t>,
    pub is_xwayland: bool,
    pub comm: Option<String>,
    pub exe: Option<String>,
}

pub async fn handle_client_query(
    tl: &Rc<ToolClient>,
    id: JayClientQueryId,
) -> AHashMap<u64, Client> {
    use jay_client_query::*;
    let c = Rc::new(RefCell::new(Vec::<Client>::new()));
    macro_rules! last {
        ($c:ident) => {
            $c.borrow_mut().last_mut().unwrap()
        };
    }
    Start::handle(tl, id, c.clone(), |c, event| {
        c.borrow_mut().push(Client::default());
        last!(c).id = event.id;
    });
    Sandboxed::handle(tl, id, c.clone(), |c, _event| {
        last!(c).sandboxed = true;
    });
    SandboxEngine::handle(tl, id, c.clone(), |c, event| {
        last!(c).sandbox_engine = Some(event.engine.to_string());
    });
    SandboxAppId::handle(tl, id, c.clone(), |c, event| {
        last!(c).sandbox_app_id = Some(event.app_id.to_string());
    });
    SandboxInstanceId::handle(tl, id, c.clone(), |c, event| {
        last!(c).sandbox_instance_id = Some(event.instance_id.to_string());
    });
    Uid::handle(tl, id, c.clone(), |c, event| {
        last!(c).uid = Some(event.uid);
    });
    Pid::handle(tl, id, c.clone(), |c, event| {
        last!(c).pid = Some(event.pid);
    });
    IsXwayland::handle(tl, id, c.clone(), |c, _event| {
        last!(c).is_xwayland = true;
    });
    Comm::handle(tl, id, c.clone(), |c, event| {
        last!(c).comm = Some(event.comm.to_string());
    });
    Exe::handle(tl, id, c.clone(), |c, event| {
        last!(c).exe = Some(event.exe.to_string());
    });
    tl.round_trip().await;
    mem::take(&mut *c.borrow_mut())
        .into_iter()
        .map(|c| (c.id, c))
        .collect()
}

pub struct ClientPrinter<'a> {
    pub prefix: &'a mut String,
}

impl ClientPrinter<'_> {
    pub fn print_client(&mut self, c: &Client) {
        let p = &self.prefix;
        macro_rules! opt {
            ($field:ident, $pretty:expr) => {
                if let Some(v) = &c.$field {
                    println!("{p}{}: {}", $pretty, v);
                }
            };
        }
        macro_rules! bol {
            ($field:ident, $pretty:expr) => {
                if c.$field {
                    println!("{p}{}", $pretty);
                }
            };
        }
        println!("{p}id: {}", c.id);
        bol!(sandboxed, "sandboxed");
        opt!(sandbox_engine, "sandbox engine");
        opt!(sandbox_app_id, "sandbox app id");
        opt!(sandbox_instance_id, "sandbox instance id");
        opt!(uid, "uid");
        opt!(pid, "pid");
        bol!(is_xwayland, "xwayland");
        opt!(comm, "comm");
        opt!(exe, "exe");
    }
}
