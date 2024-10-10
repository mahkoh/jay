use {
    crate::{
        dbus::{prelude::Variant, DbusObject, DictEntry, DynamicType, PendingReply, FALSE},
        pipewire::pw_con::PwCon,
        portal::{
            ptl_remote_desktop::{DeviceTypes, RemoteDesktopPhase},
            ptl_screencast::{ScreencastPhase, ScreencastTarget},
            PortalState, PORTAL_SUCCESS,
        },
        utils::{clonecell::CloneCell, hash_map_ext::HashMapExt},
        wire_dbus::org::freedesktop::impl_::portal::{
            remote_desktop::StartReply as RdStartReply, screen_cast::StartReply as ScStartReply,
            session::Closed,
        },
    },
    std::{borrow::Cow, cell::Cell, ops::Deref, rc::Rc},
};

shared_ids!(SessionId);
pub struct PortalSession {
    pub _id: SessionId,
    pub state: Rc<PortalState>,
    pub pw_con: Option<Rc<PwCon>>,
    pub app: String,
    pub session_obj: DbusObject,
    pub sc_phase: CloneCell<ScreencastPhase>,
    pub rd_phase: CloneCell<RemoteDesktopPhase>,
    pub start_reply: Cell<Option<PortalSessionReply>>,
}

pub enum PortalSessionReply {
    RemoteDesktop(PendingReply<RdStartReply<'static>>),
    ScreenCast(PendingReply<ScStartReply<'static>>),
}

impl PortalSession {
    pub(super) fn kill(&self) {
        self.session_obj.emit_signal(&Closed);
        self.state.sessions.remove(self.session_obj.path());
        self.reply_err("Session has been terminated");
        match self.rd_phase.set(RemoteDesktopPhase::Terminated) {
            RemoteDesktopPhase::Init => {}
            RemoteDesktopPhase::DevicesSelected => {}
            RemoteDesktopPhase::Terminated => {}
            RemoteDesktopPhase::Selecting(s) => {
                for gui in s.guis.lock().drain_values() {
                    gui.kill(false);
                }
            }
            RemoteDesktopPhase::Starting(s) => {
                s.ei_session.con.remove_obj(s.ei_session.deref());
                s.dpy.sessions.remove(self.session_obj.path());
            }
            RemoteDesktopPhase::Started(s) => {
                s.ei_session.con.remove_obj(s.ei_session.deref());
                s.dpy.sessions.remove(self.session_obj.path());
            }
        }
        match self.sc_phase.set(ScreencastPhase::Terminated) {
            ScreencastPhase::Init => {}
            ScreencastPhase::SourcesSelected(_) => {}
            ScreencastPhase::Terminated => {}
            ScreencastPhase::Selecting(s) => {
                for gui in s.guis.lock().drain_values() {
                    gui.kill(false);
                }
            }
            ScreencastPhase::SelectingWindow(s) => {
                s.dpy.con.remove_obj(&*s.selector);
            }
            ScreencastPhase::SelectingWorkspace(s) => {
                s.dpy.con.remove_obj(&*s.selector);
            }
            ScreencastPhase::Starting(s) => {
                s.node.con.destroy_obj(s.node.deref());
                s.dpy.sessions.remove(self.session_obj.path());
                match &s.target {
                    ScreencastTarget::Output(_) => {}
                    ScreencastTarget::Workspace(_, w, true) => {
                        s.dpy.con.remove_obj(&**w);
                    }
                    ScreencastTarget::Workspace(_, _, false) => {}
                    ScreencastTarget::Toplevel(t) => {
                        s.dpy.con.remove_obj(&**t);
                    }
                }
            }
            ScreencastPhase::Started(s) => {
                s.jay_screencast.con.remove_obj(s.jay_screencast.deref());
                s.node.con.destroy_obj(s.node.deref());
                s.dpy.sessions.remove(self.session_obj.path());
                for buffer in s.pending_buffers.borrow_mut().drain(..) {
                    s.dpy.con.remove_obj(&*buffer);
                }
            }
        }
    }

    pub(super) fn send_start_reply(
        &self,
        pw_node_id: Option<u32>,
        restore_data: Option<Variant<'static>>,
    ) {
        let inner_type = DynamicType::DictEntry(
            Box::new(DynamicType::String),
            Box::new(DynamicType::Variant),
        );
        let kt = DynamicType::Struct(vec![
            DynamicType::U32,
            DynamicType::Array(Box::new(inner_type.clone())),
        ]);
        let mut streams = vec![];
        if let Some(node_id) = pw_node_id {
            streams = vec![Variant::U32(node_id), Variant::Array(inner_type, vec![])];
        }
        let mut variants = vec![
            DictEntry {
                key: "devices".into(),
                value: Variant::U32(DeviceTypes::all().0),
            },
            DictEntry {
                key: "clipboard_enabled".into(),
                value: Variant::Bool(FALSE),
            },
            DictEntry {
                key: "streams".into(),
                value: Variant::Array(kt, streams),
            },
        ];
        if let Some(rd) = restore_data {
            variants.push(DictEntry {
                key: "restore_data".into(),
                value: rd,
            });
        }
        if let Some(reply) = self.start_reply.take() {
            match reply {
                PortalSessionReply::RemoteDesktop(reply) => {
                    reply.ok(&RdStartReply {
                        response: PORTAL_SUCCESS,
                        results: Cow::Borrowed(&variants),
                    });
                }
                PortalSessionReply::ScreenCast(reply) => {
                    reply.ok(&ScStartReply {
                        response: PORTAL_SUCCESS,
                        results: Cow::Borrowed(&variants),
                    });
                }
            }
        }
    }

    pub(super) fn reply_err(&self, err: &str) {
        if let Some(reply) = self.start_reply.take() {
            match reply {
                PortalSessionReply::RemoteDesktop(r) => r.err(err),
                PortalSessionReply::ScreenCast(r) => r.err(err),
            }
        }
    }
}
