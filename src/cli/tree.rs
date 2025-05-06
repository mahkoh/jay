use {
    crate::{
        cli::{
            GlobalArgs,
            clients::{Client, ClientPrinter, handle_client_query},
        },
        ifs::jay_tree_query::{
            TREE_TY_CONTAINER, TREE_TY_DISPLAY, TREE_TY_FLOAT, TREE_TY_LAYER_SURFACE,
            TREE_TY_LOCK_SURFACE, TREE_TY_OUTPUT, TREE_TY_PLACEHOLDER, TREE_TY_WORKSPACE,
            TREE_TY_X_WINDOW, TREE_TY_XDG_POPUP, TREE_TY_XDG_TOPLEVEL,
        },
        rect::Rect,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{JayCompositorId, JayTreeQueryId, jay_client_query, jay_compositor, jay_tree_query},
    },
    ahash::{AHashMap, AHashSet},
    clap::{Args, Subcommand},
    isnt::std_1::primitive::IsntSliceExt,
    std::{cell::RefCell, rc::Rc},
};

#[derive(Args, Debug)]
pub struct TreeArgs {
    #[clap(subcommand)]
    cmd: TreeCmd,
}

#[derive(Subcommand, Debug)]
enum TreeCmd {
    /// Query the tree.
    Query(QueryArgs),
}

#[derive(Args, Debug)]
struct QueryArgs {
    /// Whether to perform a recursive query.
    #[arg(short, long)]
    recursive: bool,
    /// Whether to repeatedly print details of the same client.
    #[arg(long)]
    all_clients: bool,
    #[clap(subcommand)]
    cmd: QueryCmd,
}

#[derive(Subcommand, Debug)]
enum QueryCmd {
    /// Query the entire display.
    Root,
    /// Query a workspace by name.
    WorkspaceName(QueryWorkspaceNameArgs),
    /// Interactively select a workspace to query.
    SelectWorkspace,
    /// Interactively select a window to query.
    SelectWindow,
}

#[derive(Args, Debug)]
struct QueryWorkspaceNameArgs {
    /// The name of the workspace.
    name: String,
}

pub fn main(global: GlobalArgs, tree_args: TreeArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let comp = tc.jay_compositor().await;
        let tree = Rc::new(Tree {
            tc: tc.clone(),
            comp,
        });
        tree.run(tree_args).await;
    });
}

struct Tree {
    tc: Rc<ToolClient>,
    comp: JayCompositorId,
}

impl Tree {
    async fn run(&self, args: TreeArgs) {
        match &args.cmd {
            TreeCmd::Query(a) => self.query(a).await,
        }
    }

    async fn query(&self, args: &QueryArgs) {
        let id = self.tc.id();
        self.tc.send(jay_compositor::CreateTreeQuery {
            self_id: self.comp,
            id,
        });
        let mut query = Query {
            tree: self,
            tc: &self.tc,
            id,
        };
        query.run(args).await;
    }
}

struct Query<'a> {
    tree: &'a Tree,
    tc: &'a Rc<ToolClient>,
    id: JayTreeQueryId,
}

#[derive(Debug, Default)]
struct Queried {
    not_found: bool,
    roots: Vec<Node>,
    stack: Vec<Node>,
    client_ids: AHashSet<u64>,
}

#[derive(Debug, Default)]
struct Node {
    ty: u32,
    children: Vec<Node>,
    position: Option<Rect>,
    toplevel_id: Option<String>,
    client: Option<u64>,
    title: Option<String>,
    app_id: Option<String>,
    tag: Option<String>,
    x_class: Option<String>,
    x_instance: Option<String>,
    x_role: Option<String>,
    workspace: Option<String>,
    placeholder_for: Option<String>,
    floating: bool,
    visible: bool,
    urgent: bool,
    fullscreen: bool,
    output: Option<String>,
}

impl Query<'_> {
    async fn run(&mut self, args: &QueryArgs) {
        match &args.cmd {
            QueryCmd::Root => {
                self.tc.send(SetRootDisplay { self_id: self.id });
            }
            QueryCmd::WorkspaceName(a) => {
                self.tc.send(SetRootWorkspaceName {
                    self_id: self.id,
                    workspace: &a.name,
                });
            }
            QueryCmd::SelectWorkspace => {
                let id = self.tc.select_workspace().await;
                if id.is_none() {
                    fatal!("Workspace selection failed");
                }
                self.tc.send(SetRootWorkspace {
                    self_id: self.id,
                    workspace: id,
                });
            }
            QueryCmd::SelectWindow => {
                let id = self.tc.select_toplevel().await;
                if id.is_none() {
                    fatal!("Window selection failed");
                }
                self.tc.send(SetRootToplevel {
                    self_id: self.id,
                    toplevel: id,
                });
            }
        }
        let tl = self.tc;
        let id = self.id;
        let d = Rc::new(RefCell::new(Queried::default()));
        use jay_tree_query::*;
        macro_rules! last {
            ($d:ident, $n:ident) => {
                let $d = &mut *$d.borrow_mut();
                let $n = $d.stack.last_mut().unwrap();
            };
        }
        NotFound::handle(tl, id, d.clone(), |d, _event| {
            d.borrow_mut().not_found = true;
        });
        End::handle(tl, id, d.clone(), |d, _event| {
            let d = &mut *d.borrow_mut();
            let n = d.stack.pop().unwrap();
            if let Some(p) = d.stack.last_mut() {
                p.children.push(n);
            } else {
                d.roots.push(n);
            }
        });
        Position::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.position = Rect::new_sized(event.x, event.y, event.w, event.h);
        });
        Start::handle(tl, id, d.clone(), |d, event| {
            let d = &mut *d.borrow_mut();
            let node = Node {
                ty: event.ty,
                ..Default::default()
            };
            d.stack.push(node);
        });
        OutputName::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.output = Some(event.name.to_string());
        });
        WorkspaceName::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.workspace = Some(event.name.to_string());
        });
        ToplevelId::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.toplevel_id = Some(event.id.to_string());
        });
        ClientId::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.client = Some(event.id);
            d.client_ids.insert(event.id);
        });
        Title::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.title = Some(event.title.to_string());
        });
        AppId::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.app_id = Some(event.app_id.to_string());
        });
        Floating::handle(tl, id, d.clone(), |d, _event| {
            last!(d, n);
            n.floating = true;
        });
        Visible::handle(tl, id, d.clone(), |d, _event| {
            last!(d, n);
            n.visible = true;
        });
        Urgent::handle(tl, id, d.clone(), |d, _event| {
            last!(d, n);
            n.urgent = true;
        });
        Fullscreen::handle(tl, id, d.clone(), |d, _event| {
            last!(d, n);
            n.fullscreen = true;
        });
        Tag::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.tag = Some(event.tag.to_string());
        });
        XClass::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.x_class = Some(event.class.to_string());
        });
        XInstance::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.x_instance = Some(event.instance.to_string());
        });
        XRole::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.x_role = Some(event.role.to_string());
        });
        Workspace::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.workspace = Some(event.name.to_string());
        });
        PlaceholderFor::handle(tl, id, d.clone(), |d, event| {
            last!(d, n);
            n.placeholder_for = Some(event.id.to_string());
        });
        if args.recursive {
            tl.send(SetRecursive {
                self_id: id,
                recursive: 1,
            });
        }
        tl.send(Execute { self_id: id });
        tl.round_trip().await;
        let clients = {
            let id = tl.id();
            tl.send(jay_compositor::CreateClientQuery {
                self_id: self.tree.comp,
                id,
            });
            use jay_client_query::*;
            for &client in &d.borrow().client_ids {
                tl.send(AddId {
                    self_id: id,
                    id: client,
                });
            }
            tl.send(Execute { self_id: id });
            handle_client_query(tl, id).await
        };
        let mut printer = Printer {
            clients,
            printed_clients: Default::default(),
            verbose: args.all_clients,
            prefix: "".to_string(),
            output_depth: 0,
            workspace_depth: 0,
        };
        for node in &d.borrow().roots {
            printer.print(node);
        }
    }
}

struct Printer {
    clients: AHashMap<u64, Client>,
    printed_clients: AHashSet<u64>,
    verbose: bool,
    prefix: String,
    output_depth: u32,
    workspace_depth: u32,
}

impl Printer {
    fn print(&mut self, node: &Node) {
        let p = &self.prefix;
        'ty: {
            let n = match node.ty {
                TREE_TY_DISPLAY => "display",
                TREE_TY_OUTPUT => "output",
                TREE_TY_WORKSPACE => "workspace",
                TREE_TY_FLOAT => "float",
                TREE_TY_CONTAINER => "container",
                TREE_TY_PLACEHOLDER => "placeholder",
                TREE_TY_XDG_TOPLEVEL => "xdg-toplevel",
                TREE_TY_X_WINDOW => "x-window",
                TREE_TY_XDG_POPUP => "xdg-popup",
                TREE_TY_LAYER_SURFACE => "layer-surface",
                TREE_TY_LOCK_SURFACE => "lock-surface",
                _ => {
                    println!("{p}- unknown ({}):", node.ty);
                    break 'ty;
                }
            };
            println!("{p}- {n}:");
        }
        macro_rules! opt {
            ($field:ident, $pretty:expr) => {
                if let Some(v) = &node.$field {
                    println!("{p}    {}: {}", $pretty, v);
                }
            };
        }
        macro_rules! bol {
            ($field:ident, $pretty:expr) => {
                if node.$field {
                    println!("{p}    {}", $pretty);
                }
            };
        }
        if node.ty == TREE_TY_OUTPUT {
            opt!(output, "name");
        }
        if node.ty == TREE_TY_WORKSPACE {
            opt!(workspace, "name");
        }
        opt!(toplevel_id, "id");
        opt!(placeholder_for, "placeholder-for");
        if let Some(r) = node.position {
            println!(
                "{p}    pos: {}x{} + {}x{}",
                r.x1(),
                r.y1(),
                r.width(),
                r.height()
            );
        }
        if let Some(client_id) = node.client {
            let client = self.clients.get(&client_id);
            if client.is_some() && (self.printed_clients.insert(client_id) || self.verbose) {
                println!("{p}    client:");
                let mut prefix = format!("{}      ", p);
                let mut cp = ClientPrinter {
                    prefix: &mut prefix,
                };
                cp.print_client(client.unwrap());
            } else {
                println!("{p}    client: {client_id}");
            }
        }
        opt!(title, "title");
        opt!(app_id, "app-id");
        opt!(tag, "tag");
        opt!(x_class, "x-class");
        opt!(x_instance, "x-instance");
        opt!(x_role, "x-role");
        if self.workspace_depth == 0 && node.ty != TREE_TY_WORKSPACE {
            opt!(workspace, "workspace");
        }
        if self.workspace_depth == 0 && self.output_depth == 0 && node.ty != TREE_TY_OUTPUT {
            opt!(output, "output");
        }
        bol!(floating, "floating");
        bol!(visible, "visible");
        bol!(urgent, "urgent");
        bol!(fullscreen, "fullscreen");
        if node.children.is_not_empty() {
            let (od, wd) = match node.ty {
                TREE_TY_OUTPUT => (1, 0),
                TREE_TY_WORKSPACE => (0, 1),
                _ => (0, 0),
            };
            self.output_depth += od;
            self.workspace_depth += wd;
            println!("{p}    children:");
            let len = self.prefix.len();
            self.prefix.push_str("      ");
            for child in &node.children {
                self.print(child);
            }
            self.prefix.truncate(len);
            self.output_depth -= od;
            self.workspace_depth -= wd;
        }
    }
}
