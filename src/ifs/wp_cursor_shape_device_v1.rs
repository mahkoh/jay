use {
    crate::{
        client::{Client, ClientError},
        cursor::KnownCursor,
        ifs::wl_seat::{tablet::TabletToolOpt, WlSeatGlobal},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_cursor_shape_device_v1::*, WpCursorShapeDeviceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

const DEFAULT: u32 = 1;
const CONTEXT_MENU: u32 = 2;
const HELP: u32 = 3;
const POINTER: u32 = 4;
const PROGRESS: u32 = 5;
const WAIT: u32 = 6;
const CELL: u32 = 7;
const CROSSHAIR: u32 = 8;
const TEXT: u32 = 9;
const VERTICAL_TEXT: u32 = 10;
const ALIAS: u32 = 11;
const COPY: u32 = 12;
const MOVE: u32 = 13;
const NO_DROP: u32 = 14;
const NOT_ALLOWED: u32 = 15;
const GRAB: u32 = 16;
const GRABBING: u32 = 17;
const E_RESIZE: u32 = 18;
const N_RESIZE: u32 = 19;
const NE_RESIZE: u32 = 20;
const NW_RESIZE: u32 = 21;
const S_RESIZE: u32 = 22;
const SE_RESIZE: u32 = 23;
const SW_RESIZE: u32 = 24;
const W_RESIZE: u32 = 25;
const EW_RESIZE: u32 = 26;
const NS_RESIZE: u32 = 27;
const NESW_RESIZE: u32 = 28;
const NWSE_RESIZE: u32 = 29;
const COL_RESIZE: u32 = 30;
const ROW_RESIZE: u32 = 31;
const ALL_SCROLL: u32 = 32;
const ZOOM_IN: u32 = 33;
const ZOOM_OUT: u32 = 34;
const DND_ASK: u32 = 35;
const ALL_RESIZE: u32 = 36;

const V2: Version = Version(2);

pub enum CursorShapeCursorUser {
    Seat(Rc<WlSeatGlobal>),
    TabletTool(Rc<TabletToolOpt>),
}

pub struct WpCursorShapeDeviceV1 {
    pub id: WpCursorShapeDeviceV1Id,
    pub client: Rc<Client>,
    pub cursor_user: CursorShapeCursorUser,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpCursorShapeDeviceV1RequestHandler for WpCursorShapeDeviceV1 {
    type Error = WpCursorShapeDeviceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_shape(&self, req: SetShape, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let cursor = match req.shape {
            DEFAULT => KnownCursor::Default,
            CONTEXT_MENU => KnownCursor::ContextMenu,
            HELP => KnownCursor::Help,
            POINTER => KnownCursor::Pointer,
            PROGRESS => KnownCursor::Progress,
            WAIT => KnownCursor::Wait,
            CELL => KnownCursor::Cell,
            CROSSHAIR => KnownCursor::Crosshair,
            TEXT => KnownCursor::Text,
            VERTICAL_TEXT => KnownCursor::VerticalText,
            ALIAS => KnownCursor::Alias,
            COPY => KnownCursor::Copy,
            MOVE => KnownCursor::Move,
            NO_DROP => KnownCursor::NoDrop,
            NOT_ALLOWED => KnownCursor::NotAllowed,
            GRAB => KnownCursor::Grab,
            GRABBING => KnownCursor::Grabbing,
            E_RESIZE => KnownCursor::EResize,
            N_RESIZE => KnownCursor::NResize,
            NE_RESIZE => KnownCursor::NeResize,
            NW_RESIZE => KnownCursor::NwResize,
            S_RESIZE => KnownCursor::SResize,
            SE_RESIZE => KnownCursor::SeResize,
            SW_RESIZE => KnownCursor::SwResize,
            W_RESIZE => KnownCursor::WResize,
            EW_RESIZE => KnownCursor::EwResize,
            NS_RESIZE => KnownCursor::NsResize,
            NESW_RESIZE => KnownCursor::NeswResize,
            NWSE_RESIZE => KnownCursor::NwseResize,
            COL_RESIZE => KnownCursor::ColResize,
            ROW_RESIZE => KnownCursor::RowResize,
            ALL_SCROLL => KnownCursor::AllScroll,
            ZOOM_IN => KnownCursor::ZoomIn,
            ZOOM_OUT => KnownCursor::ZoomOut,
            DND_ASK if self.version >= V2 => KnownCursor::DndAsk,
            ALL_RESIZE if self.version >= V2 => KnownCursor::AllResize,
            _ => return Err(WpCursorShapeDeviceV1Error::UnknownShape(req.shape)),
        };
        let tablet_tool;
        let (node_client_id, user) = match &self.cursor_user {
            CursorShapeCursorUser::Seat(s) => match s.pointer_node() {
                Some(n) => (n.node_client_id(), s.pointer_cursor()),
                _ => return Ok(()),
            },
            CursorShapeCursorUser::TabletTool(t) => match t.get() {
                Some(t) => {
                    tablet_tool = t;
                    (tablet_tool.node().node_client_id(), tablet_tool.cursor())
                }
                _ => return Ok(()),
            },
        };
        if node_client_id != Some(self.client.id) {
            return Ok(());
        }
        user.set_known(cursor);
        Ok(())
    }
}

object_base! {
    self = WpCursorShapeDeviceV1;
    version = self.version;
}

impl Object for WpCursorShapeDeviceV1 {}

simple_add_obj!(WpCursorShapeDeviceV1);

#[derive(Debug, Error)]
pub enum WpCursorShapeDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Shape {0} is unknown")]
    UnknownShape(u32),
}
efrom!(WpCursorShapeDeviceV1Error, ClientError);
