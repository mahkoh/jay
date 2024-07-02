use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::{
            zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1,
            zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
            zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_pointer_gestures_v1::*, ZwpPointerGesturesV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPointerGesturesV1Global {
    pub name: GlobalName,
}

pub struct ZwpPointerGesturesV1 {
    pub id: ZwpPointerGesturesV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpPointerGesturesV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpPointerGesturesV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpPointerGesturesV1Error> {
        let obj = Rc::new(ZwpPointerGesturesV1 {
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

global_base!(
    ZwpPointerGesturesV1Global,
    ZwpPointerGesturesV1,
    ZwpPointerGesturesV1Error
);

impl Global for ZwpPointerGesturesV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }
}

simple_add_global!(ZwpPointerGesturesV1Global);

impl ZwpPointerGesturesV1RequestHandler for ZwpPointerGesturesV1 {
    type Error = ZwpPointerGesturesV1Error;

    fn get_swipe_gesture(&self, req: GetSwipeGesture, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.pointer)?.seat.global.clone();
        let obj = Rc::new(ZwpPointerGestureSwipeV1 {
            id: req.id,
            client: self.client.clone(),
            seat: seat.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        seat.swipe_bindings.add(&self.client, &obj);
        Ok(())
    }

    fn get_pinch_gesture(&self, req: GetPinchGesture, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.pointer)?.seat.global.clone();
        let obj = Rc::new(ZwpPointerGesturePinchV1 {
            id: req.id,
            client: self.client.clone(),
            seat: seat.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        seat.pinch_bindings.add(&self.client, &obj);
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_hold_gesture(&self, req: GetHoldGesture, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.pointer)?.seat.global.clone();
        let obj = Rc::new(ZwpPointerGestureHoldV1 {
            id: req.id,
            client: self.client.clone(),
            seat: seat.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        seat.hold_bindings.add(&self.client, &obj);
        Ok(())
    }
}

object_base! {
    self = ZwpPointerGesturesV1;
    version = self.version;
}

impl Object for ZwpPointerGesturesV1 {}

simple_add_obj!(ZwpPointerGesturesV1);

#[derive(Debug, Error)]
pub enum ZwpPointerGesturesV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPointerGesturesV1Error, ClientError);
