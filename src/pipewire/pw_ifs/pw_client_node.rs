use std::cell::RefCell;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Weak;
use {
    crate::{
        async_engine::SpawnedFuture,
        pipewire::{
            pw_con::PwConData,
            pw_mem::{PwMemMap, PwMemTyped},
            pw_object::{PwObject, PwObjectData},
            pw_parser::{PwParser, PwParserError},
            pw_pod::{
                pw_node_activation, PW_CHOICE_Enum, PW_OBJECT_Format, PwIoType, PwPropFlag,
                SPA_FORMAT_VIDEO_format, SPA_FORMAT_VIDEO_size, SPA_FORMAT_mediaSubtype,
                SPA_FORMAT_mediaType, SPA_MEDIA_SUBTYPE_raw, SPA_MEDIA_TYPE_video,
                SPA_PARAM_EnumFormat, SPA_PARAM_Format, SPA_VIDEO_FORMAT_RGBx,
                PW_NODE_ACTIVATION_TRIGGERED, SPA_PARAM_INFO_READ, SPA_PARAM_INFO_WRITE,
                SPA_PORT_FLAG_CAN_ALLOC_BUFFERS, SPA_VIDEO_FORMAT_BGR, SPA_VIDEO_FORMAT_I420,
                SPA_VIDEO_FORMAT_RGB, SPA_VIDEO_FORMAT_RGBA, SPA_VIDEO_FORMAT_YUY2,
            },
        },
        time::Time,
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};
use crate::format::{Format, pw_formats};
use crate::pipewire::pw_mem::PwMemError;
use crate::pipewire::pw_pod::{PW_CHOICE_Flags, PW_OBJECT_ParamBuffers, PW_OBJECT_ParamMeta, PwPodFraction, PwPodObject, PwPodRectangle, SPA_DATA_MemFd, SPA_DATA_MemPtr, SPA_DIRECTION_INPUT, SPA_DIRECTION_OUTPUT, SPA_FORMAT_VIDEO_framerate, spa_fraction, SPA_IO_Buffers, spa_io_buffers, SPA_META_Bitmap, spa_meta_bitmap, SPA_META_Busy, spa_meta_busy, SPA_META_Control, SPA_META_Cursor, spa_meta_cursor, SPA_META_Header, spa_meta_header, SPA_META_Invalid, spa_meta_region, SPA_META_VideoCrop, SPA_META_VideoDamage, SPA_NODE_COMMAND_Start, SPA_PARAM_Buffers, SPA_PARAM_BUFFERS_align, SPA_PARAM_BUFFERS_blocks, SPA_PARAM_BUFFERS_buffers, SPA_PARAM_BUFFERS_dataType, SPA_PARAM_BUFFERS_size, SPA_PARAM_BUFFERS_stride, SPA_PARAM_Meta, SPA_PARAM_META_size, SPA_PARAM_META_type, SPA_PORT_FLAG, spa_rectangle, SPA_STATUS_HAVE_DATA, SPA_VIDEO_FORMAT_BGRA, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_NV12, SpaDataFlags, SpaDataType, SpaDataTypes, SpaDirection, SpaIoType, SpaMediaSubtype, SpaMediaType, SpaMetaHeaderFlags, SpaMetaType, SpaNodeBuffersFlags, SpaNodeCommand, SpaParamType, SpaVideoFormat};
use crate::utils::bitfield::Bitfield;
use crate::utils::bitflags::BitflagsExt;

pw_opcodes! {
    PwClientNodeMethods;

    GetNode     = 1,
    Update      = 2,
    PortUpdate  = 3,
    SetActive   = 4,
    Event       = 5,
    PortBuffers = 6,
}

pw_opcodes! {
    PwClientNodeEvents;

    Transport      = 0,
    SetParam       = 1,
    SetIo          = 2,
    Event          = 3,
    Command        = 4,
    AddPort        = 5,
    RemovePort     = 6,
    PortSetParam   = 7,
    PortUseBuffers = 8,
    PortSetIo      = 9,
    SetActivation  = 10,
    PortSetMixInfo = 11,
}

pub trait PwClientNodeOwner {
    fn port_format_changed(&self, port: &Rc<PwClientNodePort>, format: PwClientNodePortFormat);
}

pub struct PwClientNodeOwnerDummy;

impl PwClientNodeOwner for PwClientNodeOwnerDummy {
    fn port_format_changed(&self, _port: &Rc<PwClientNodePort>, _format: PwClientNodePortFormat) {
        // nothing
    }
}

bitflags! {
    PwClientNodePortChanges: u32;

    CHANGED_SUPPORTED_PARAMS = 1 << 0,
}

bitflags! {
    PwClientNodePortSupportedMetas: u32;

    SUPPORTED_META_HEADER = 1 << 0,
    SUPPORTED_META_BUSY = 1 << 1,
}

pub struct PwClientNodePort {
    pub node: Rc<PwClientNode>,

    pub direction: SpaDirection,
    pub id: u32,

    pub destroyed: Cell<bool>,

    pub effective_format: Cell<PwClientNodePortFormat>,
    pub supported_formats: RefCell<Option<PwClientNodePortSupportedFormats>>,
    pub supported_metas: Cell<PwClientNodePortSupportedMetas>,
    pub can_alloc_buffers: Cell<bool>,

    pub buffers: RefCell<Vec<Rc<PwClientNodeBuffer>>>,

    pub buffer_config: Cell<Option<PwClientNodeBufferConfig>>,

    pub io_buffers: CloneCell<Option<Rc<PwMemTyped<spa_io_buffers>>>>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PwClientNodeBufferConfig {
    pub num_buffers: usize,
    pub planes: usize,
    pub size: Option<u32>,
    pub stride: Option<u32>,
    pub align: usize,
    pub data_type: SpaDataType,
}

pub struct PwClientNodeBuffer {
    pub meta_header: CloneCell<Option<Rc<PwMemTyped<spa_meta_header>>>>,
    pub meta_busy: CloneCell<Option<Rc<PwMemTyped<spa_meta_busy>>>>,
}

#[derive(Clone, Debug, Default)]
pub struct PwClientNodePortSupportedFormats {
    pub media_type: Option<SpaMediaType>,
    pub media_sub_type: Option<SpaMediaSubtype>,
    pub video_size: Option<PwPodRectangle>,
    pub formats: Vec<&'static Format>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PwClientNodePortFormat {
    pub media_type: Option<SpaMediaType>,
    pub media_sub_type: Option<SpaMediaSubtype>,
    pub video_size: Option<PwPodRectangle>,
    pub format: Option<&'static Format>,
    pub framerate: Option<PwPodFraction>,
}

pub struct PwClientNode {
    pub data: PwObjectData,
    pub con: Rc<PwConData>,
    pub ios: CopyHashMap<PwIoType, Rc<PwMemMap>>,

    pub owner: CloneCell<Weak<dyn PwClientNodeOwner>>,

    pub ports: CopyHashMap<(SpaDirection, u32), Rc<PwClientNodePort>>,

    pub check_buffers: Cell<Option<SpawnedFuture<()>>>,

    pub port_out_free: RefCell<Bitfield>,
    pub port_in_free: RefCell<Bitfield>,

    pub activation: CloneCell<Option<Rc<PwMemTyped<pw_node_activation>>>>,
    pub transport_in: Cell<Option<SpawnedFuture<()>>>,
    pub transport_out: CloneCell<Option<Rc<OwnedFd>>>,

    pub activations: CopyHashMap<u32, Rc<PwNodeActivation>>,
}

pub struct PwNodeActivation {
    pub activation: Rc<PwMemTyped<pw_node_activation>>,
    pub fd: Rc<OwnedFd>,
}

pub const PW_CLIENT_NODE_FACTORY: &str = "client-node";
pub const PW_CLIENT_NODE_INTERFACE: &str = "PipeWire:Interface:ClientNode";
pub const PW_CLIENT_NODE_VERSION: i32 = 4;

const PW_CLIENT_NODE_UPDATE_PARAMS: u32 = 1 << 0;
const PW_CLIENT_NODE_UPDATE_INFO: u32 = 1 << 1;

const SPA_NODE_CHANGE_MASK_FLAGS: u64 = 1 << 0;
const SPA_NODE_CHANGE_MASK_PROPS: u64 = 1 << 1;
const SPA_NODE_CHANGE_MASK_PARAMS: u64 = 1 << 2;

const PW_CLIENT_NODE_PORT_UPDATE_PARAMS: u32 = 1 << 0;
const PW_CLIENT_NODE_PORT_UPDATE_INFO: u32 = 1 << 1;

const SPA_PORT_CHANGE_MASK_FLAGS: u64 = 1 << 0;
const SPA_PORT_CHANGE_MASK_RATE: u64 = 1 << 1;
const SPA_PORT_CHANGE_MASK_PROPS: u64 = 1 << 2;
const SPA_PORT_CHANGE_MASK_PARAMS: u64 = 1 << 3;

impl PwClientNode {
    pub fn send_update(&self) {
        self.con.send(self, PwClientNodeMethods::Update, |f| {
            f.write_struct(|f| {
                f.write_uint(PW_CLIENT_NODE_UPDATE_INFO);
                f.write_uint(0);
                f.write_struct(|f| {
                    f.write_uint(0);
                    f.write_uint(1);
                    f.write_ulong(SPA_NODE_CHANGE_MASK_PARAMS | SPA_NODE_CHANGE_MASK_FLAGS);
                    f.write_ulong(0);
                    f.write_uint(0);
                    f.write_uint(0);
                });
            });
        });
    }

    pub fn send_active(&self, active: bool) {
        self.con.send(self, PwClientNodeMethods::SetActive, |f| {
            f.write_struct(|f| {
                f.write_bool(active);
            });
        });
    }

    pub fn create_port(self: &Rc<Self>, output: bool) -> Rc<PwClientNodePort> {
        let (ids, direction) = match output {
            true => (&self.port_out_free, SPA_DIRECTION_OUTPUT),
            false => (&self.port_in_free, SPA_DIRECTION_INPUT),
        };
        let port = Rc::new(PwClientNodePort {
            node: self.clone(),
            direction,
            id: ids.borrow_mut().acquire(),
            destroyed: Cell::new(false),
            effective_format: Cell::new(Default::default()),
            supported_formats: RefCell::new(None),
            supported_metas: Cell::new(PwClientNodePortSupportedMetas::none()),
            can_alloc_buffers: Cell::new(false),
            buffers: RefCell::new(vec![]),
            buffer_config: Cell::new(None),
            io_buffers: Default::default(),
        });
        self.ports.set((direction, port.id), port.clone());
        port
    }

    pub fn send_port_update(&self, port: &PwClientNodePort) {
        self.con.send(self, PwClientNodeMethods::PortUpdate, |f| {
            f.write_struct(|f| {
                // direction
                f.write_uint(port.direction.0);
                // id
                f.write_uint(port.id);
                // change flags
                f.write_uint(PW_CLIENT_NODE_PORT_UPDATE_PARAMS | PW_CLIENT_NODE_PORT_UPDATE_INFO);
                let sm = port.supported_metas.get();
                let mut metas = vec![];
                if sm.contains(SUPPORTED_META_HEADER) {
                    metas.push((SPA_META_Header, mem::size_of::<spa_meta_header>()));
                }
                if sm.contains(SUPPORTED_META_BUSY) {
                    metas.push((SPA_META_Busy, mem::size_of::<spa_meta_busy>()));
                }
                let sf = port.supported_formats.borrow_mut();
                let bc = port.buffer_config.get();
                let mut num_params = metas.len() as u32;
                if sf.is_some() {
                    num_params += 1;
                }
                if bc.is_some() {
                    num_params += 1;
                }
                // num params
                f.write_uint(num_params);
                if let Some(sf) = sf.deref() {
                    f.write_object(PW_OBJECT_Format, SPA_PARAM_EnumFormat.0, |f| {
                        if let Some(mt) = sf.media_type {
                            f.write_property(SPA_FORMAT_mediaType.0, PwPropFlag::none(), |f| {
                                f.write_id(mt.0);
                            });
                        }
                        if let Some(mst) = sf.media_sub_type {
                            f.write_property(SPA_FORMAT_mediaSubtype.0, PwPropFlag::none(), |f| {
                                f.write_id(mst.0);
                            });
                        }
                        if sf.formats.len() > 0 {
                            f.write_property(SPA_FORMAT_VIDEO_format.0, PwPropFlag::none(), |f| {
                                f.write_choice(PW_CHOICE_Enum, 0, |f| {
                                    f.write_id(sf.formats[0].pipewire.0);
                                    for format in &sf.formats {
                                        f.write_id(format.pipewire.0);
                                    }
                                });
                            });
                        }
                        if let Some(vs) = sf.video_size {
                            f.write_property(SPA_FORMAT_VIDEO_size.0, PwPropFlag::none(), |f| {
                                f.write_choice(PW_CHOICE_Enum, 0, |f| {
                                    f.write_rectangle(vs.width, vs.height);
                                    f.write_rectangle(vs.width, vs.height);
                                });
                            });
                        }
                    });
                }
                if let Some(bc) = &bc {
                    f.write_object(PW_OBJECT_ParamBuffers, SPA_PARAM_Buffers.0, |f| {
                        f.write_property(SPA_PARAM_BUFFERS_buffers.0, PwPropFlag::none(), |f| {
                            f.write_uint(bc.num_buffers as _);
                        });
                        f.write_property(SPA_PARAM_BUFFERS_blocks.0, PwPropFlag::none(), |f| {
                            f.write_uint(bc.planes as _);
                        });
                        if let Some(size) = bc.size {
                            f.write_property(SPA_PARAM_BUFFERS_size.0, PwPropFlag::none(), |f| {
                                f.write_uint(size as _);
                            });
                        }
                        if let Some(stride) = bc.stride {
                            f.write_property(SPA_PARAM_BUFFERS_stride.0, PwPropFlag::none(), |f| {
                                f.write_uint(stride as _);
                            });
                        }
                        f.write_property(SPA_PARAM_BUFFERS_align.0, PwPropFlag::none(), |f| {
                            f.write_uint(bc.align as _);
                        });
                        f.write_property(SPA_PARAM_BUFFERS_dataType.0, PwPropFlag::none(), |f| {
                            f.write_choice(PW_CHOICE_Flags, 0, |f| {
                                f.write_uint(1 << bc.data_type.0);
                            });
                        });
                    });
                }
                for (key, size) in metas {
                    f.write_object(PW_OBJECT_ParamMeta, SPA_PARAM_Meta.0, |f| {
                        f.write_property(SPA_PARAM_META_type.0, PwPropFlag::none(), |f| {
                            f.write_id(key.0);
                        });
                        f.write_property(SPA_PARAM_META_size.0, PwPropFlag::none(), |f| {
                            f.write_uint(size as u32);
                        });
                    });
                }
                f.write_struct(|f| {
                    // change mask
                    f.write_ulong(
                        SPA_PORT_CHANGE_MASK_FLAGS
                            | SPA_PORT_CHANGE_MASK_PROPS
                            | SPA_PORT_CHANGE_MASK_PARAMS
                            | SPA_PORT_CHANGE_MASK_RATE,
                    );
                    let mut flags = SPA_PORT_FLAG::none();
                    if port.can_alloc_buffers.get() {
                        flags = SPA_PORT_FLAG_CAN_ALLOC_BUFFERS;
                    }
                    // flags
                    f.write_ulong(flags.0);
                    // rate num
                    f.write_int(0);
                    // rate denom
                    f.write_int(1);
                    // num props
                    f.write_int(0);
                    let mut num_params = 1;
                    if sf.is_some() {
                        num_params += 1;
                    }
                    if bc.is_some() {
                        num_params += 1;
                    }
                    // num params
                    f.write_uint(num_params);
                    if sf.is_some() {
                        f.write_id(SPA_PARAM_EnumFormat.0);
                        f.write_uint(SPA_PARAM_INFO_READ.0);
                    }
                    if bc.is_some() {
                        f.write_id(SPA_PARAM_Buffers.0);
                        f.write_uint(SPA_PARAM_INFO_READ.0);
                    }
                    f.write_id(SPA_PARAM_Meta.0);
                    f.write_uint(SPA_PARAM_INFO_READ.0);
                });
            });
        });
    }

    fn handle_set_param(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        Ok(())
    }

    fn handle_set_io(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let s = p.read_struct()?;
        let mut p2 = s.fields;
        let id = PwIoType(p2.read_id()?);
        let memid = p2.read_uint()?;
        let offset = p2.read_uint()?;
        let size = p2.read_uint()?;
        log::info!("set io {:?}", id);
        if memid == !0 {
            self.ios.remove(&id);
        } else {
            let map = match self.con.mem.map(memid, offset, size) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Could not map memory from the pool: {}", ErrorFmt(e));
                    return Ok(());
                }
            };
            self.ios.set(id, map);
        }
        Ok(())
    }

    fn handle_event(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        Ok(())
    }

    fn handle_command(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let s1 = p.read_struct()?;
        let mut p1 = s1.fields;
        let obj = p1.read_object()?;
        match SpaNodeCommand(obj.id) {
            SPA_NODE_COMMAND_Start => {
                for port in self.ports.lock().values() {
                    if let Some(header) = port.buffers.borrow_mut()[1].meta_header.get() {
                        unsafe {
                            let header = header.write();
                            header.pts = Time::now_unchecked().nsec() as _;
                            header.seq = 1000000000;
                            header.flags = SpaMetaHeaderFlags::none();
                            header.dts_offset = 0;
                        }
                    }
                    if let Some(io) = port.io_buffers.get() {
                        unsafe {
                            io.write().status = SPA_STATUS_HAVE_DATA;
                            io.write().buffer_id = 1;
                        }
                    }
                }
                if let Some(wfd) = self.transport_out.get() {
                    let _ = uapi::eventfd_write(wfd.raw(), 1);
                }
                // for activation in self.activations.lock().values() {
                //     unsafe {
                //         activation.activation.write().status = PW_NODE_ACTIVATION_TRIGGERED;
                //         activation.activation.write().signal_time = Time::now_unchecked().nsec();
                //     }
                //     uapi::eventfd_write(activation.fd.raw(), 1).unwrap();
                // }
            },
            v => {
                log::error!("Unhandled node command {:?}", v);
            }
        }
        Ok(())
    }

    fn handle_add_port(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        Ok(())
    }

    fn handle_remove_port(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        Ok(())
    }

    fn port_set_format(&self, port: &Rc<PwClientNodePort>, obj: Option<PwPodObject<'_>>) -> Result<(), PwClientNodeError> {
        let mut obj = match obj {
            Some(obj) => obj,
            _ => {
                port.effective_format.take();
                return Ok(());
            }
        };
        let mut format = PwClientNodePortFormat::default();
        if let Some(mt) = obj.get_param(SPA_FORMAT_mediaType.0)? {
            format.media_type = Some(SpaMediaType(mt.pod.get_id()?));
        }
        if let Some(mt) = obj.get_param(SPA_FORMAT_mediaSubtype.0)? {
            format.media_sub_type = Some(SpaMediaSubtype(mt.pod.get_id()?));
        }
        if let Some(mt) = obj.get_param(SPA_FORMAT_VIDEO_size.0)? {
            format.video_size = Some(mt.pod.get_rectangle()?);
        }
        if let Some(mt) = obj.get_param(SPA_FORMAT_VIDEO_format.0)? {
            if let Some(fmt) = pw_formats().get(&SpaVideoFormat(mt.pod.get_id()?)) {
                format.format = Some(*fmt);
            }
        }
        if let Some(mt) = obj.get_param(SPA_FORMAT_VIDEO_framerate.0)? {
            format.framerate = Some(mt.pod.get_fraction()?);
        }
        port.effective_format.set(format);
        if let Some(owner) = self.owner.get().upgrade() {
            owner.port_format_changed(&port, format);
        }
        let mut bc = PwClientNodeBufferConfig::default();
        bc.num_buffers = 3;
        bc.planes = 1;
        if let (Some(size), Some(format)) = (format.video_size, format.format) {
            let stride = size.width * format.bpp;
            bc.stride = Some(stride);
            bc.size = Some(stride * size.height)
        }
        bc.align = 16;
        bc.data_type = SPA_DATA_MemFd;
        port.buffer_config.set(Some(bc));
        self.send_port_update(port);
        Ok(())
    }

    fn handle_port_set_param(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let mut s1 = p.read_struct()?;
        let mut p1 = s1.fields;
        let direction = SpaDirection(p1.read_uint()?);
        let port_id = p1.read_uint()?;
        let id = SpaParamType(p1.read_id()?);
        let flags = p1.read_int()?;
        let mut obj = p1.read_object_opt()?;
        let port = self.get_port(direction, port_id)?;
        match id {
            SPA_PARAM_Format => self.port_set_format(&port, obj)?,
            _ => {
                log::warn!("port_set_param: Ignoring unexpected port parameter {:?}", id);
            }
        }
        Ok(())
    }

    fn handle_port_use_buffers(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let s1 = p.read_struct()?;
        let mut p1 = s1.fields;
        let direction = SpaDirection(p1.read_uint()?);
        let port_id = p1.read_uint()?;
        let mix_id = p1.read_int()?;
        let flags = SpaNodeBuffersFlags(p1.read_uint()?);
        let n_buffers = p1.read_uint()?;
        let port = self.get_port(direction, port_id)?;

        log::info!("port_use_buffers: mix_id={}, flags={:?}, n_buffers={}", mix_id, flags, n_buffers);

        let mut res = vec!();

        for _ in 0..n_buffers {
            let mem_id = p1.read_uint()?;
            let offset = p1.read_uint()?;
            let size = p1.read_uint()?;
            let n_metas = p1.read_uint()?;

            let mut buf = PwClientNodeBuffer {
                meta_header: Default::default(),
                meta_busy: Default::default(),
            };

            let mem = self.con.mem.map(mem_id, offset, size)?;

            log::info!("  mem_id={}, offset={}, size={}", mem_id, offset, size);
            log::info!("  n_metas={}", n_metas);

            let mut offset = 0;

            for _ in 0..n_metas {
                let ty = SpaMetaType(p1.read_id()?);
                let size = p1.read_uint()? as usize;

                unsafe {
                    match ty {
                        SPA_META_Header      => {
                            let header = mem.typed_at::<spa_meta_header>(offset);
                            log::info!("    SPA_META_Header = {:?}", header.read());
                            buf.meta_header.set(Some(header));
                        }
                        SPA_META_VideoCrop   => {
                            let crop = mem.typed_at::<spa_meta_region>(offset);
                            log::info!("    SPA_META_VideoCrop = {:?}", crop.read());
                        }
                        SPA_META_VideoDamage => {
                            let video_damage = mem.typed_at::<spa_meta_region>(offset);
                            log::info!("    SPA_META_VideoDamage = {:?}", video_damage.read());
                        }
                        SPA_META_Bitmap      => {
                            let bitmap = mem.typed_at::<spa_meta_bitmap>(offset);
                            log::info!("    SPA_META_Bitmap = {:?}", bitmap.read());
                        }
                        SPA_META_Cursor      => {
                            let cursor = mem.typed_at::<spa_meta_cursor>(offset);
                            log::info!("    SPA_META_Cursor = {:?}", cursor.read());
                        },
                        SPA_META_Control     => { },
                        SPA_META_Busy        => {
                            let busy = mem.typed_at::<spa_meta_busy>(offset);
                            log::info!("    SPA_META_Busy = {:?}", busy.read());
                            buf.meta_busy.set(Some(busy));
                        },
                        _ => { },
                    }
                }

                offset += size;
            }

            let n_datas = p1.read_uint()?;

            log::info!("  offset = {}, n_datas={}", offset, n_datas);

            for _ in 0..n_datas {
                let ty = SpaDataType(p1.read_id()?);
                let data_id = p1.read_uint()?;
                let flags = SpaDataFlags(p1.read_uint()?);
                let mapoffset = p1.read_uint()?;
                let maxsize = p1.read_uint()?;

                if ty == SPA_DATA_MemPtr {
                    unsafe {
                        let offset = data_id as usize;
                        for i in 0..maxsize as usize {
                            mem.bytes_mut()[offset + i] = i as _;
                        }
                        // for i in 0..maxsize as usize / 4 {
                        //     mem.bytes_mut()[offset + 4 * i + 0] = 255;
                        //     mem.bytes_mut()[offset + 4 * i + 1] = 255;
                        //     mem.bytes_mut()[offset + 4 * i + 2] = 255;
                        //     mem.bytes_mut()[offset + 4 * i + 3] = 255;
                        // }
                    }
                }
                log::info!("    ty={:?}, data_id={}, flags={:?}, mapoffset={}, maxsize={}", ty, data_id, flags, mapoffset, maxsize);
            }

            res.push(Rc::new(buf));
        }

        *port.buffers.borrow_mut() = res;

        Ok(())
    }

    fn handle_port_set_io(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let s = p.read_struct()?;
        let mut p2 = s.fields;
        let direction = SpaDirection(p2.read_uint()?);
        let port_id = p2.read_uint()?;
        let mix_id = p2.read_uint()?;
        let id = SpaIoType(p2.read_id()?);
        let mem_id = p2.read_uint()?;
        let offset = p2.read_uint()?;
        let size = p2.read_uint()?;
        let port = self.get_port(direction, port_id)?;
        match id {
            SPA_IO_Buffers => {
                port.io_buffers.set(Some(self.con.mem.map(mem_id, offset, size)?.typed()));
            },
            _ => { },
        }
        Ok(())
    }

    fn handle_transport(self: &Rc<Self>, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let s = p.read_struct()?;
        let mut p2 = s.fields;
        let readfd = p2.read_fd()?;
        let writefd = p2.read_fd()?;
        let memid = p2.read_uint()?;
        let offset = p2.read_uint()?;
        let size = p2.read_uint()?;
        let map = match self.con.mem.map(memid, offset, size) {
            Ok(m) => m,
            Err(e) => {
                log::error!("Could not map memory from the pool: {}", ErrorFmt(e));
                return Ok(());
            }
        };
        let typed = map.typed::<pw_node_activation>();
        self.activation.set(Some(typed.clone()));
        self.transport_in.set(Some(
            self.con.eng.spawn(self.clone().transport_in(typed, readfd)),
        ));
        self.transport_out.set(Some(writefd));
        Ok(())
    }

    fn handle_set_activation(
        self: &Rc<Self>,
        mut p: PwParser<'_>,
    ) -> Result<(), PwClientNodeError> {
        let s = p.read_struct()?;
        let mut p2 = s.fields;
        let node = p2.read_uint()?;
        let signalfd = p2.read_fd_opt()?;
        if let Some(signalfd) = signalfd {
            let memid = p2.read_uint()?;
            let offset = p2.read_uint()?;
            let size = p2.read_uint()?;
            let map = match self.con.mem.map(memid, offset, size) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Could not map memory from the pool: {}", ErrorFmt(e));
                    return Ok(());
                }
            };
            let typed = map.typed::<pw_node_activation>();
            self.activations.set(node, Rc::new(PwNodeActivation {
                activation: typed,
                fd: signalfd,
            }));
        } else {
            self.activations.remove(&node);
        }
        Ok(())
    }

    fn get_port(&self, direction: SpaDirection, port_id: u32) -> Result<Rc<PwClientNodePort>, PwClientNodeError> {
        match self.ports.get(&(direction, port_id)) {
            Some(p) => Ok(p),
            _ => Err(PwClientNodeError::UnknownPort(direction, port_id)),
        }
    }

    fn handle_port_set_mix_info(&self, mut p: PwParser<'_>) -> Result<(), PwClientNodeError> {
        let mut s1 = p.read_struct()?;
        let mut p1 = s1.fields;
        let direction = SpaDirection(p1.read_uint()?);
        let port_id = p1.read_uint()?;
        let mix_id = p1.read_int()?;
        let peer_id = p1.read_int()?;
        let dict = p1.read_dict_struct()?;
        let port = self.get_port(direction, port_id)?;
        Ok(())
    }

    async fn transport_in(
        self: Rc<Self>,
        activation: Rc<PwMemTyped<pw_node_activation>>,
        fd: Rc<OwnedFd>,
    ) {
        loop {
            // unsafe {
            //     log::info!("transport = {:#?}", activation.read());
            // }
            if let Err(e) = self.con.ring.readable(&fd).await {
                log::error!(
                    "Could not wait for transport to become readable: {}",
                    ErrorFmt(e)
                );
                return;
            }
            let mut n = 0u64;
            if let Err(e) = uapi::read(fd.raw(), &mut n) {
                log::error!("Could not read from eventfd: {}", ErrorFmt(e));
                return;
            }
            if n > 1 {
                log::warn!("Missed {} transport changes", n - 1);
            }
        }
    }
}

pw_object_base! {
    PwClientNode, "client-node", PwClientNodeEvents;

    Transport      => handle_transport,
    SetParam       => handle_set_param,
    SetIo          => handle_set_io,
    Event          => handle_event,
    Command        => handle_command,
    AddPort        => handle_add_port,
    RemovePort     => handle_remove_port,
    PortSetParam   => handle_port_set_param,
    PortUseBuffers => handle_port_use_buffers,
    PortSetIo      => handle_port_set_io,
    SetActivation  => handle_set_activation,
    PortSetMixInfo => handle_port_set_mix_info,
}

impl PwObject for PwClientNode {}

#[derive(Debug, Error)]
pub enum PwClientNodeError {
    #[error(transparent)]
    PwParserError(#[from] PwParserError),
    #[error(transparent)]
    PwMemError(#[from] PwMemError),
    #[error("Unknown port direction {0:?}")]
    UnknownPortDirection(SpaDirection),
    #[error("Unknown port {0:?}@{1}")]
    UnknownPort(SpaDirection, u32),
}
