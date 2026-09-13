use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::PositionHintRequest;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::NodeBase;
use crate::tree::TreeTimeline::LiveTL;
use crate::wire::WpPointerWarpV1Id;
use crate::wire::wp_pointer_warp_v1::Destroy;
use crate::wire::wp_pointer_warp_v1::WarpPointer;
use crate::wire::wp_pointer_warp_v1::WpPointerWarpV1RequestHandler;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct WpPointerWarpV1Global {
    name: GlobalName,
}

impl WpPointerWarpV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpPointerWarpV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(WpPointerWarpV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for WpPointerWarpV1Global {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct WpPointerWarpV1 {
    id: WpPointerWarpV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl WpPointerWarpV1RequestHandler for WpPointerWarpV1 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn warp_pointer(&self, req: WarpPointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(serial) = self.client.map_serial(req.serial) else {
            return Ok(());
        };
        if Some(serial) != self.client.last_enter_serial.get() {
            return Ok(());
        }
        let pointer = self.client.lookup(req.pointer)?;
        let seat = &pointer.seat.global;
        let Some(pointer_node) = seat.pointer_node() else {
            return Ok(());
        };
        if pointer_node.node_client_id() != Some(self.client.id) {
            return Ok(());
        }
        let (x, y) = (req.x, req.y);
        let surface = self.client.lookup(req.surface)?;
        let buffer = surface.node_absolute_position(LiveTL);
        let (x_int, y_int) = buffer.translate_inv(x.round_down(), y.round_down());
        self.client
            .state
            .position_hint_requests
            .push(PositionHintRequest {
                seat: seat.clone(),
                client_id: surface.client.id,
                old_pos: seat.pointer_cursor.position(),
                new_pos: (x.apply_fract(x_int), y.apply_fract(y_int)),
            });
        Ok(())
    }
}
