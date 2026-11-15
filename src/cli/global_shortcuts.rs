use crate::cli::GlobalArgs;
use crate::cli::json::JsonConfiguredGlobalTrigger;
use crate::cli::json::JsonGlobalShortcut;
use crate::cli::json::JsonGlobalShortcuts;
use crate::cli::json::jsonl;
use crate::object::Version;
use crate::tools::tool_client::Handle;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::utils::stack::Stack;
use crate::wire::JayGlobalShortcutsId;
use crate::wire::jay_compositor;
use crate::wire::jay_global_shortcuts;
use clap::Args;
use clap::Subcommand;
use std::rc::Rc;

#[derive(Args, Debug)]
pub struct GlobalShortcutsArgs {
    #[clap(subcommand)]
    command: Option<GlobalShortcutsCmd>,
}

#[derive(Subcommand, Debug, Default)]
pub enum GlobalShortcutsCmd {
    /// List registered global shortcuts and configured triggers.
    #[default]
    Status,
}

pub fn main(global: GlobalArgs, args: GlobalShortcutsArgs) {
    with_tool_client(async move |tc| {
        let gs = GlobalShortcuts { tc: tc.clone() };
        gs.run(&global, args).await;
    });
}

struct GlobalShortcuts {
    tc: Rc<ToolClient>,
}

struct RegisteredShortcut {
    app_id: String,
    shortcut_id: String,
    description: String,
    preferred_trigger: String,
    configured_trigger: String,
}

struct ConfiguredTrigger {
    app_id: String,
    shortcut_id: String,
    trigger: String,
}

fn format_config_action(app_id: &str, shortcut_id: &str) -> String {
    format!("global-shortcut:{app_id}:{shortcut_id}")
}

fn non_empty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

impl GlobalShortcuts {
    async fn run(self, global: &GlobalArgs, args: GlobalShortcutsArgs) {
        let tc = &self.tc;
        if tc.jay_compositor_version().await < Version(46) {
            fatal!("Compositor does not support inspecting global shortcuts");
        }
        let comp = tc.jay_compositor().await;
        let id = tc.id();
        tc.send(jay_compositor::GetGlobalShortcuts { self_id: comp, id });
        match args.command.unwrap_or_default() {
            GlobalShortcutsCmd::Status => self.status(global, id).await,
        }
        tc.send(jay_global_shortcuts::Destroy { self_id: id });
        tc.round_trip().await;
    }

    async fn status(&self, global: &GlobalArgs, id: JayGlobalShortcutsId) {
        let tc = &self.tc;
        tc.send(jay_global_shortcuts::Get { self_id: id });
        let shortcuts = Rc::new(Stack::default());
        jay_global_shortcuts::Shortcut::handle(tc, id, shortcuts.clone(), |v, msg| {
            v.push(RegisteredShortcut {
                app_id: msg.app_id.to_string(),
                shortcut_id: msg.shortcut_id.to_string(),
                description: msg.description.to_string(),
                preferred_trigger: msg.preferred_trigger.to_string(),
                configured_trigger: msg.configured_trigger.to_string(),
            });
        });
        let configured = Rc::new(Stack::default());
        jay_global_shortcuts::ConfiguredTrigger::handle(tc, id, configured.clone(), |v, msg| {
            v.push(ConfiguredTrigger {
                app_id: msg.app_id.to_string(),
                shortcut_id: msg.shortcut_id.to_string(),
                trigger: msg.trigger.to_string(),
            });
        });
        tc.round_trip().await;
        let shortcuts = shortcuts.take();
        let configured = configured.take();
        if global.json {
            let json = JsonGlobalShortcuts {
                shortcuts: shortcuts
                    .iter()
                    .map(|sc| JsonGlobalShortcut {
                        app_id: &sc.app_id,
                        shortcut_id: &sc.shortcut_id,
                        description: non_empty(&sc.description),
                        preferred_trigger: non_empty(&sc.preferred_trigger),
                        configured_trigger: non_empty(&sc.configured_trigger),
                        config_action: format_config_action(&sc.app_id, &sc.shortcut_id),
                    })
                    .collect(),
                configured_triggers: configured
                    .iter()
                    .map(|ct| JsonConfiguredGlobalTrigger {
                        app_id: &ct.app_id,
                        shortcut_id: &ct.shortcut_id,
                        trigger: &ct.trigger,
                        config_action: format_config_action(&ct.app_id, &ct.shortcut_id),
                    })
                    .collect(),
            };
            jsonl(&json);
            return;
        }
        if shortcuts.is_empty() && configured.is_empty() {
            println!("No global shortcuts registered or configured.");
            return;
        }
        if !shortcuts.is_empty() {
            println!("Registered shortcuts:");
            for sc in &shortcuts {
                let trigger = non_empty(&sc.configured_trigger)
                    .map(|t| format!("configured `{t}`"))
                    .or_else(|| {
                        non_empty(&sc.preferred_trigger).map(|t| format!("preferred `{t}`"))
                    })
                    .unwrap_or_else(|| "unbound".to_string());
                let action = format_config_action(&sc.app_id, &sc.shortcut_id);
                if sc.description.is_empty() {
                    println!("  {action} [{trigger}]");
                } else {
                    println!("  {action} ({}) [{trigger}]", sc.description);
                }
            }
        }
        if !configured.is_empty() {
            println!("Configured triggers:");
            for ct in &configured {
                println!(
                    "  {} -> {}",
                    ct.trigger,
                    format_config_action(&ct.app_id, &ct.shortcut_id),
                );
            }
        }
    }
}
