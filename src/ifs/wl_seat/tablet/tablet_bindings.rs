use {
    crate::{
        client::ClientId,
        ifs::wl_seat::tablet::zwp_tablet_seat_v2::ZwpTabletSeatV2,
        utils::copyhashmap::{CopyHashMap, Locked},
        wire::ZwpTabletSeatV2Id,
    },
    std::rc::Rc,
};

pub struct TabletBindings<T> {
    bindings: CopyHashMap<(ClientId, ZwpTabletSeatV2Id), Rc<T>>,
}

impl<T> Default for TabletBindings<T> {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
        }
    }
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
