use {
    crate::{
        client::{Client, ClientError},
        fixed::Fixed,
        globals::{Global, GlobalName},
        ifs::{
            wl_seat::{
                zwp_pointer_constraints_v1::zwp_confined_pointer_v1::ZwpConfinedPointerV1,
                WlSeatGlobal,
            },
            wl_surface::WlSurface,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::{Rect, Region},
        utils::clonecell::CloneCell,
        wire::{
            zwp_pointer_constraints_v1::*, WlPointerId, WlRegionId, WlSurfaceId,
            ZwpPointerConstraintsV1Id,
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    zwp_locked_pointer_v1::ZwpLockedPointerV1,
};

pub mod zwp_confined_pointer_v1;
pub mod zwp_locked_pointer_v1;

pub struct ZwpPointerConstraintsV1Global {
    pub name: GlobalName,
}

pub struct ZwpPointerConstraintsV1 {
    pub id: ZwpPointerConstraintsV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ConstraintType {
    Lock,
    Confine,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SeatConstraintStatus {
    Active,
    ActivatableOnFocus,
    Inactive,
    TerminallyDisabled,
}

pub struct SeatConstraint {
    pub owner: CloneCell<Option<Rc<dyn ConstraintOwner>>>,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub surface: Rc<WlSurface>,
    pub region: CloneCell<Option<Rc<Region>>>,
    pub one_shot: bool,
    pub status: Cell<SeatConstraintStatus>,
    pub ty: ConstraintType,
}

impl SeatConstraint {
    pub fn deactivate(&self) {
        if self.status.get() == SeatConstraintStatus::Active {
            self.seat.constraint.take();
            if let Some(owner) = self.owner.get() {
                owner.send_disabled();
            }
            if self.one_shot {
                self.status.set(SeatConstraintStatus::TerminallyDisabled);
            } else {
                self.status.set(SeatConstraintStatus::Inactive);
            }
        }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        let region = self.region.get();
        if let Some(region) = region {
            return region.contains(x, y);
        }
        true
    }

    pub fn warp(&self, mut x: Fixed, mut y: Fixed) -> (Fixed, Fixed) {
        let (x_int, y_int) = (x.round_down(), y.round_down());
        let mut best_rect;
        if let Some(region) = self.region.get() {
            if region.is_empty() {
                return (x, y);
            }
            best_rect = region[0];
            let mut best_dist = region[0].dist_squared(x_int, y_int);
            for rect in &region[1..] {
                let dist = rect.dist_squared(x_int, y_int);
                if dist < best_dist {
                    best_dist = dist;
                    best_rect = *rect;
                }
            }
        } else {
            best_rect = self.surface.buffer_abs_pos.get().at_point(0, 0);
        }
        if x_int < best_rect.x1() {
            x = Fixed::from_int(best_rect.x1());
        } else if x_int >= best_rect.x2() {
            x = Fixed::from_int(best_rect.x2()) - Fixed::EPSILON;
        }
        if y_int < best_rect.y1() {
            y = Fixed::from_int(best_rect.y1());
        } else if y_int >= best_rect.y2() {
            y = Fixed::from_int(best_rect.y2()) - Fixed::EPSILON;
        }
        (x, y)
    }

    fn detach(&self) {
        self.deactivate();
        self.owner.take();
        self.surface.constraints.remove(&self.seat.id);
    }

    fn set_region(&self, region: WlRegionId) -> Result<(), ZwpPointerConstraintsV1Error> {
        let region = get_region(&self.client, region)?;
        self.region.set(region);
        Ok(())
    }
}

pub trait ConstraintOwner {
    fn send_enabled(&self);
    fn send_disabled(&self);
}

const LT_ONESHOT: u32 = 1;
const LT_PERSISTENT: u32 = 2;

impl ZwpPointerConstraintsV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpPointerConstraintsV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpPointerConstraintsV1Error> {
        let cs = Rc::new(ZwpPointerConstraintsV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, cs);
        client.add_client_obj(&cs)?;
        Ok(())
    }
}

fn get_region(
    client: &Client,
    region: WlRegionId,
) -> Result<Option<Rc<Region>>, ZwpPointerConstraintsV1Error> {
    let region = if region.is_some() {
        let mut region = client.lookup(region)?.region();
        if let Some(scale) = client.wire_scale.get() {
            let rects: Vec<_> = region
                .rects()
                .iter()
                .map(|r| {
                    Rect::new_sized(
                        r.x1() / scale,
                        r.y1() / scale,
                        r.width() / scale,
                        r.height() / scale,
                    )
                    .unwrap()
                })
                .collect();
            region = Region::from_rects(&rects);
        }
        Some(region)
    } else {
        None
    };
    Ok(region)
}

impl ZwpPointerConstraintsV1 {
    fn create_constraint(
        &self,
        ty: ConstraintType,
        pointer: WlPointerId,
        surface: WlSurfaceId,
        region: WlRegionId,
        lifetime: u32,
    ) -> Result<Rc<SeatConstraint>, ZwpPointerConstraintsV1Error> {
        let pointer = self.client.lookup(pointer)?;
        let seat = &pointer.seat.global;
        let surface = self.client.lookup(surface)?;
        if surface.constraints.contains(&seat.id) {
            return Err(ZwpPointerConstraintsV1Error::AlreadyConstrained);
        }
        let region = get_region(&self.client, region)?;
        let one_shot = match lifetime {
            LT_ONESHOT => true,
            LT_PERSISTENT => false,
            _ => return Err(ZwpPointerConstraintsV1Error::UnknownLifetime(lifetime)),
        };
        Ok(Rc::new(SeatConstraint {
            owner: Default::default(),
            client: self.client.clone(),
            seat: seat.clone(),
            surface,
            region: CloneCell::new(region),
            one_shot,
            status: Cell::new(SeatConstraintStatus::Inactive),
            ty,
        }))
    }
}

impl ZwpPointerConstraintsV1RequestHandler for ZwpPointerConstraintsV1 {
    type Error = ZwpPointerConstraintsV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn lock_pointer(&self, req: LockPointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let constraint = self.create_constraint(
            ConstraintType::Lock,
            req.pointer,
            req.surface,
            req.region,
            req.lifetime,
        )?;
        let lp = Rc::new(ZwpLockedPointerV1 {
            id: req.id,
            tracker: Default::default(),
            constraint,
            version: self.version,
        });
        track!(self.client, lp);
        self.client.add_client_obj(&lp)?;
        lp.constraint.owner.set(Some(lp.clone()));
        lp.constraint
            .surface
            .constraints
            .insert(lp.constraint.seat.id, lp.constraint.clone());
        lp.constraint.seat.maybe_constrain_pointer_node();
        Ok(())
    }

    fn confine_pointer(&self, req: ConfinePointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let constraint = self.create_constraint(
            ConstraintType::Confine,
            req.pointer,
            req.surface,
            req.region,
            req.lifetime,
        )?;
        let lp = Rc::new(ZwpConfinedPointerV1 {
            id: req.id,
            tracker: Default::default(),
            constraint,
            version: self.version,
        });
        track!(self.client, lp);
        self.client.add_client_obj(&lp)?;
        lp.constraint.owner.set(Some(lp.clone()));
        lp.constraint
            .surface
            .constraints
            .insert(lp.constraint.seat.id, lp.constraint.clone());
        lp.constraint.seat.maybe_constrain_pointer_node();
        Ok(())
    }
}

global_base!(
    ZwpPointerConstraintsV1Global,
    ZwpPointerConstraintsV1,
    ZwpPointerConstraintsV1Error
);

impl Global for ZwpPointerConstraintsV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpPointerConstraintsV1Global);

object_base! {
    self = ZwpPointerConstraintsV1;
    version = self.version;
}

impl Object for ZwpPointerConstraintsV1 {}

simple_add_obj!(ZwpPointerConstraintsV1);

#[derive(Debug, Error)]
pub enum ZwpPointerConstraintsV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a constraint attached for the seat")]
    AlreadyConstrained,
    #[error("The constraint lifetime {0} is unknown")]
    UnknownLifetime(u32),
}
efrom!(ZwpPointerConstraintsV1Error, ClientError);
