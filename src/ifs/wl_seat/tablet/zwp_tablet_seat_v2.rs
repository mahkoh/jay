use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_seat::{
            WlSeatGlobal,
            tablet::{
                Tablet, TabletPad, TabletTool, zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2,
                zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
                zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2,
                zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2, zwp_tablet_pad_v2::ZwpTabletPadV2,
                zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpTabletSeatV2Id, zwp_tablet_seat_v2::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

const BUSTYPE_SINCE: Version = Version(2);
const DIALS_SINCE: Version = Version(2);

pub struct ZwpTabletSeatV2 {
    pub id: ZwpTabletSeatV2Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpTabletSeatV2 {
    pub fn detach(&self) {
        self.seat.tablet.seats.remove(&self.client, self);
    }

    pub fn announce_tablet(self: &Rc<Self>, tablet: &Rc<Tablet>) {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let obj = Rc::new(ZwpTabletV2 {
            id,
            client: self.client.clone(),
            seat: self.clone(),
            tracker: Default::default(),
            version: self.version,
            tablet: tablet.clone(),
        });
        track!(self.client, obj);
        self.client.add_server_obj(&obj);
        self.send_tablet_added(&obj);
        obj.send_name(&tablet.name);
        obj.send_id(tablet.vid, tablet.pid);
        obj.send_path(&tablet.path);
        if obj.version >= BUSTYPE_SINCE {
            if let Some(bustype) = tablet.bustype {
                obj.send_bustype(bustype);
            }
        }
        obj.send_done();
        tablet.bindings.add(self, &obj);
    }

    pub fn announce_tool(self: &Rc<Self>, tool: &Rc<TabletTool>) {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let obj = Rc::new(ZwpTabletToolV2 {
            id,
            client: self.client.clone(),
            seat: self.clone(),
            tool: tool.opt.clone(),
            tracker: Default::default(),
            version: self.version,
            entered: Cell::new(false),
        });
        track!(self.client, obj);
        self.client.add_server_obj(&obj);
        self.send_tool_added(&obj);
        obj.send_type(tool.type_);
        obj.send_hardware_serial(tool.hardware_serial);
        obj.send_hardware_id_wacom(tool.hardware_id_wacom);
        for cap in &tool.capabilities {
            obj.send_capability(*cap);
        }
        obj.send_done();
        tool.bindings.add(self, &obj);
    }

    pub fn announce_pad(self: &Rc<Self>, pad: &Rc<TabletPad>) {
        macro_rules! id {
            () => {
                match self.client.new_id() {
                    Ok(id) => id,
                    Err(e) => {
                        self.client.error(e);
                        return;
                    }
                }
            };
        }
        let obj = Rc::new(ZwpTabletPadV2 {
            id: id!(),
            client: self.client.clone(),
            seat: self.clone(),
            tracker: Default::default(),
            version: self.version,
            pad: pad.clone(),
            entered: Cell::new(false),
        });
        track!(self.client, obj);
        self.client.add_server_obj(&obj);
        self.send_pad_added(&obj);
        obj.send_path(&pad.path);
        obj.send_buttons(pad.buttons);
        for group in &pad.groups {
            let group_obj = Rc::new(ZwpTabletPadGroupV2 {
                id: id!(),
                client: self.client.clone(),
                seat: self.clone(),
                tracker: Default::default(),
                version: self.version,
                group: group.clone(),
            });
            track!(self.client, group_obj);
            self.client.add_server_obj(&group_obj);
            obj.send_group(&group_obj);
            group_obj.send_buttons(&group.buttons);
            group_obj.send_modes(group.modes);
            for ring in &group.rings {
                let Some(ring) = pad.rings.get(*ring as usize) else {
                    continue;
                };
                let ring_obj = Rc::new(ZwpTabletPadRingV2 {
                    id: id!(),
                    client: self.client.clone(),
                    seat: self.clone(),
                    tracker: Default::default(),
                    version: self.version,
                    ring: ring.clone(),
                });
                track!(self.client, ring_obj);
                self.client.add_server_obj(&ring_obj);
                group_obj.send_ring(&ring_obj);
                ring.bindings.add(self, &ring_obj);
            }
            for strip in &group.strips {
                let Some(strip) = pad.strips.get(*strip as usize) else {
                    continue;
                };
                let strip_obj = Rc::new(ZwpTabletPadStripV2 {
                    id: id!(),
                    client: self.client.clone(),
                    seat: self.clone(),
                    tracker: Default::default(),
                    version: self.version,
                    strip: strip.clone(),
                });
                track!(self.client, strip_obj);
                self.client.add_server_obj(&strip_obj);
                group_obj.send_strip(&strip_obj);
                strip.bindings.add(self, &strip_obj);
            }
            if self.version >= DIALS_SINCE {
                for dial in &group.dials {
                    let Some(dial) = pad.dials.get(*dial as usize) else {
                        continue;
                    };
                    let dial_obj = Rc::new(ZwpTabletPadDialV2 {
                        id: id!(),
                        client: self.client.clone(),
                        seat: self.clone(),
                        tracker: Default::default(),
                        version: self.version,
                        dial: dial.clone(),
                    });
                    track!(self.client, dial_obj);
                    self.client.add_server_obj(&dial_obj);
                    group_obj.send_dial(&dial_obj);
                    dial.bindings.add(self, &dial_obj);
                }
            }
            group_obj.send_done();
        }
        obj.send_done();
        pad.bindings.add(self, &obj);
    }

    fn send_tablet_added(&self, tablet: &ZwpTabletV2) {
        self.client.event(TabletAdded {
            self_id: self.id,
            id: tablet.id,
        });
    }

    fn send_tool_added(&self, tool: &ZwpTabletToolV2) {
        self.client.event(ToolAdded {
            self_id: self.id,
            id: tool.id,
        });
    }

    fn send_pad_added(&self, pad: &ZwpTabletPadV2) {
        self.client.event(PadAdded {
            self_id: self.id,
            id: pad.id,
        });
    }
}

impl ZwpTabletSeatV2RequestHandler for ZwpTabletSeatV2 {
    type Error = ZwpTabletSeatV2Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpTabletSeatV2;
    version = self.version;
}

impl Object for ZwpTabletSeatV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpTabletSeatV2);

#[derive(Debug, Error)]
pub enum ZwpTabletSeatV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTabletSeatV2Error, ClientError);
