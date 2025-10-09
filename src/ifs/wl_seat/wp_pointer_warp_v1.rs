use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::PositionHintRequest,
        leaks::Tracker,
        object::{Object, Version},
        tree::Node,
        wire::{
            WpPointerWarpV1Id,
            wp_pointer_warp_v1::{Destroy, WarpPointer, WpPointerWarpV1RequestHandler},
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

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
    ) -> Result<(), WpPointerWarpV1Error> {
        let obj = Rc::new(WpPointerWarpV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(WpPointerWarpV1Global, WpPointerWarpV1, WpPointerWarpV1Error);

impl Global for WpPointerWarpV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpPointerWarpV1Global);

pub struct WpPointerWarpV1 {
    pub id: WpPointerWarpV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpPointerWarpV1RequestHandler for WpPointerWarpV1 {
    type Error = WpPointerWarpV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
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
        let buffer = surface.node_mapped_position();
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

object_base! {
    self = WpPointerWarpV1;
    version = self.version;
}

impl Object for WpPointerWarpV1 {}

simple_add_obj!(WpPointerWarpV1);

#[derive(Debug, Error)]
pub enum WpPointerWarpV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpPointerWarpV1Error, ClientError);
