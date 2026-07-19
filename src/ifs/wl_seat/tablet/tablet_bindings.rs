use crate::client::ClientId;
use crate::ifs::wl_seat::tablet::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::copyhashmap::Locked;
use crate::wire::ZwpTabletSeatV2Id;
use derivative::Derivative;
use std::rc::Rc;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct TabletBindings<T> {
    bindings: CopyHashMap<(ClientId, ZwpTabletSeatV2Id), Rc<T>>,
}

impl<T> TabletBindings<T> {
    pub fn add(&self, seat: &ZwpTabletSeatV2, t: &Rc<T>) {
        self.bindings.set((seat.client.id, seat.id), t.clone());
    }

    pub fn get(&self, seat: &ZwpTabletSeatV2) -> Option<Rc<T>> {
        self.bindings.get(&(seat.client.id, seat.id))
    }

    pub fn remove(&self, seat: &ZwpTabletSeatV2) -> Option<Rc<T>> {
        self.bindings.remove(&(seat.client.id, seat.id))
    }

    pub fn lock(&self) -> Locked<'_, (ClientId, ZwpTabletSeatV2Id), Rc<T>> {
        self.bindings.lock()
    }

    pub fn clear(&self) {
        self.bindings.clear();
    }
}
