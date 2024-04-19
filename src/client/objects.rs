use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                wl_data_source::WlDataSource, zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
                zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
            },
            jay_output::JayOutput,
            jay_screencast::JayScreencast,
            jay_toplevel::JayToplevel,
            jay_workspace::JayWorkspace,
            wl_buffer::WlBuffer,
            wl_display::WlDisplay,
            wl_output::WlOutput,
            wl_region::WlRegion,
            wl_registry::WlRegistry,
            wl_seat::{wl_pointer::WlPointer, WlSeat},
            wl_surface::{
                xdg_surface::{xdg_toplevel::XdgToplevel, XdgSurface},
                WlSurface,
            },
            wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1,
            xdg_positioner::XdgPositioner,
            xdg_wm_base::XdgWmBase,
        },
        object::{Object, ObjectId},
        utils::{
            clonecell::CloneCell,
            copyhashmap::{CopyHashMap, Locked},
        },
        wire::{
            JayOutputId, JayScreencastId, JayToplevelId, JayWorkspaceId, WlBufferId,
            WlDataSourceId, WlOutputId, WlPointerId, WlRegionId, WlRegistryId, WlSeatId,
            WlSurfaceId, WpLinuxDrmSyncobjTimelineV1Id, XdgPositionerId, XdgSurfaceId,
            XdgToplevelId, XdgWmBaseId, ZwlrDataControlSourceV1Id, ZwpPrimarySelectionSourceV1Id,
        },
    },
    std::{cell::RefCell, mem, rc::Rc},
};

pub struct Objects {
    pub display: CloneCell<Option<Rc<WlDisplay>>>,
    registry: CopyHashMap<ObjectId, Rc<dyn Object>>,
    pub registries: CopyHashMap<WlRegistryId, Rc<WlRegistry>>,
    pub outputs: CopyHashMap<WlOutputId, Rc<WlOutput>>,
    pub surfaces: CopyHashMap<WlSurfaceId, Rc<WlSurface>>,
    pub xdg_surfaces: CopyHashMap<XdgSurfaceId, Rc<XdgSurface>>,
    pub xdg_toplevel: CopyHashMap<XdgToplevelId, Rc<XdgToplevel>>,
    pub wl_data_source: CopyHashMap<WlDataSourceId, Rc<WlDataSource>>,
    pub zwp_primary_selection_source:
        CopyHashMap<ZwpPrimarySelectionSourceV1Id, Rc<ZwpPrimarySelectionSourceV1>>,
    pub xdg_positioners: CopyHashMap<XdgPositionerId, Rc<XdgPositioner>>,
    pub regions: CopyHashMap<WlRegionId, Rc<WlRegion>>,
    pub buffers: CopyHashMap<WlBufferId, Rc<WlBuffer>>,
    pub jay_outputs: CopyHashMap<JayOutputId, Rc<JayOutput>>,
    pub jay_workspaces: CopyHashMap<JayWorkspaceId, Rc<JayWorkspace>>,
    pub pointers: CopyHashMap<WlPointerId, Rc<WlPointer>>,
    pub xdg_wm_bases: CopyHashMap<XdgWmBaseId, Rc<XdgWmBase>>,
    pub seats: CopyHashMap<WlSeatId, Rc<WlSeat>>,
    pub screencasts: CopyHashMap<JayScreencastId, Rc<JayScreencast>>,
    pub timelines: CopyHashMap<WpLinuxDrmSyncobjTimelineV1Id, Rc<WpLinuxDrmSyncobjTimelineV1>>,
    pub zwlr_data_sources: CopyHashMap<ZwlrDataControlSourceV1Id, Rc<ZwlrDataControlSourceV1>>,
    pub jay_toplevels: CopyHashMap<JayToplevelId, Rc<JayToplevel>>,
    ids: RefCell<Vec<usize>>,
}

pub const MIN_SERVER_ID: u32 = 0xff000000;
const SEG_SIZE: usize = 8 * mem::size_of::<usize>();

impl Objects {
    pub fn new() -> Self {
        Self {
            display: CloneCell::new(None),
            registry: Default::default(),
            registries: Default::default(),
            outputs: Default::default(),
            surfaces: Default::default(),
            xdg_surfaces: Default::default(),
            xdg_toplevel: Default::default(),
            wl_data_source: Default::default(),
            zwp_primary_selection_source: Default::default(),
            xdg_positioners: Default::default(),
            regions: Default::default(),
            buffers: Default::default(),
            jay_outputs: Default::default(),
            jay_workspaces: Default::default(),
            pointers: Default::default(),
            xdg_wm_bases: Default::default(),
            seats: Default::default(),
            screencasts: Default::default(),
            timelines: Default::default(),
            zwlr_data_sources: Default::default(),
            jay_toplevels: Default::default(),
            ids: RefCell::new(vec![]),
        }
    }

    pub fn destroy(&self) {
        for surface in self.surfaces.lock().values() {
            if let Some(tl) = surface.get_toplevel() {
                tl.tl_destroy();
            }
        }
        for obj in self.registry.lock().values_mut() {
            obj.break_loops();
        }
        self.display.set(None);
        self.registry.clear();
        self.registries.clear();
        self.outputs.clear();
        self.surfaces.clear();
        self.xdg_surfaces.clear();
        self.xdg_toplevel.clear();
        self.wl_data_source.clear();
        self.zwp_primary_selection_source.clear();
        self.xdg_positioners.clear();
        self.regions.clear();
        self.buffers.clear();
        self.jay_outputs.clear();
        self.jay_workspaces.clear();
        self.xdg_wm_bases.clear();
        self.seats.clear();
        self.pointers.clear();
        self.screencasts.clear();
        self.timelines.clear();
        self.zwlr_data_sources.clear();
        self.jay_toplevels.clear();
    }

    pub fn id<T>(&self, client_data: &Client) -> Result<T, ClientError>
    where
        ObjectId: Into<T>,
    {
        const MAX_ID_OFFSET: u32 = u32::MAX - MIN_SERVER_ID;
        let offset = self.id_offset();
        if offset > MAX_ID_OFFSET {
            log::error!(
                "Client {} caused the server to allocate more than 0x{:x} ids",
                client_data.id,
                MAX_ID_OFFSET + 1
            );
            return Err(ClientError::TooManyIds);
        }
        Ok(ObjectId::from_raw(MIN_SERVER_ID + offset).into())
    }

    pub fn get_obj(&self, id: ObjectId) -> Result<Rc<dyn Object>, ClientError> {
        match self.registry.get(&id) {
            Some(o) => Ok(o),
            _ => Err(ClientError::UnknownId),
        }
    }

    pub fn add_server_object(&self, obj: Rc<dyn Object>) {
        let id = obj.id();
        assert!(id.raw() >= MIN_SERVER_ID);
        assert!(!self.registry.contains(&id));
        self.registry.set(id, obj.clone());
    }

    pub fn add_client_object(&self, obj: Rc<dyn Object>) -> Result<(), ClientError> {
        let id = obj.id();
        let res = (|| {
            if id.raw() == 0 || id.raw() >= MIN_SERVER_ID {
                return Err(ClientError::ClientIdOutOfBounds);
            }
            if self.registry.contains(&id) {
                return Err(ClientError::IdAlreadyInUse);
            }
            self.registry.set(id, obj.clone());
            Ok(())
        })();
        if let Err(e) = res {
            return Err(ClientError::AddObjectError(id, Box::new(e)));
        }
        Ok(())
    }

    pub fn remove_obj(&self, client_data: &Rc<Client>, id: ObjectId) -> Result<(), ClientError> {
        let _obj = match self.registry.remove(&id) {
            Some(o) => o,
            _ => return Err(ClientError::UnknownId),
        };
        let mut send_delete = true;
        if id.raw() >= MIN_SERVER_ID {
            let offset = (id.raw() - MIN_SERVER_ID) as usize;
            let pos = offset / SEG_SIZE;
            let seg_offset = offset % SEG_SIZE;
            let mut ids = self.ids.borrow_mut();
            if ids.len() <= pos {
                return Err(ClientError::ServerIdOutOfBounds);
            }
            ids[pos] |= 1 << seg_offset;
            send_delete = client_data.symmetric_delete.get();
        }
        if send_delete {
            client_data.display()?.send_delete_id(id);
        }
        Ok(())
    }

    pub fn registries(&self) -> Locked<WlRegistryId, Rc<WlRegistry>> {
        self.registries.lock()
    }

    fn id_offset(&self) -> u32 {
        let mut ids = self.ids.borrow_mut();
        for (pos, seg) in ids.iter_mut().enumerate() {
            if *seg != 0 {
                let offset = seg.trailing_zeros();
                *seg &= !(1 << offset);
                return (pos * SEG_SIZE) as u32 + offset;
            }
        }
        ids.push(!1);
        ((ids.len() - 1) * SEG_SIZE) as u32
    }
}
