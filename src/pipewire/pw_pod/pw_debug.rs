use {
    crate::{
        pipewire::{
            pw_parser::PwParser,
            pw_pod::{
                PW_COMMAND_Node, PW_OBJECT_Format, PW_OBJECT_ParamBuffers, PW_OBJECT_ParamIO,
                PW_OBJECT_ParamLatency, PW_OBJECT_ParamMeta, PW_OBJECT_ParamPortConfig,
                PW_OBJECT_ParamProcessLatency, PW_OBJECT_ParamProfile, PW_OBJECT_ParamRoute,
                PW_OBJECT_Profiler, PW_OBJECT_PropInfo, PW_OBJECT_Props, PW_TYPE_Id, PwPod,
                PwPodArray, PwPodObject, PwPodObjectType, PwPodSequence, PwPodStruct, PwPodType,
                PwProp, SPA_FORMAT_AUDIO_bitorder, SPA_FORMAT_AUDIO_format,
                SPA_FORMAT_AUDIO_iec958Codec, SPA_FORMAT_AUDIO_position,
                SPA_FORMAT_VIDEO_H264_alignment, SPA_FORMAT_VIDEO_H264_streamFormat,
                SPA_FORMAT_VIDEO_chromaSite, SPA_FORMAT_VIDEO_colorMatrix,
                SPA_FORMAT_VIDEO_colorPrimaries, SPA_FORMAT_VIDEO_colorRange,
                SPA_FORMAT_VIDEO_format, SPA_FORMAT_VIDEO_interlaceMode,
                SPA_FORMAT_VIDEO_multiviewFlags, SPA_FORMAT_VIDEO_multiviewMode,
                SPA_FORMAT_VIDEO_transferFunction, SPA_FORMAT_mediaSubtype, SPA_FORMAT_mediaType,
                SPA_PARAM_BUFFERS_dataType, SPA_PARAM_IO_id, SPA_PARAM_META_type,
                SPA_PARAM_PORT_CONFIG_direction, SPA_PARAM_PORT_CONFIG_mode,
                SPA_PARAM_PROFILE_available, SPA_PARAM_ROUTE_available, SPA_PARAM_ROUTE_direction,
                SPA_PROP_channelMap, SPA_PROP_iec958Codecs, SpaAudioChannel, SpaAudioFormat,
                SpaAudioIec958Codec, SpaDataTypes, SpaDirection, SpaFormat, SpaH264Alignment,
                SpaH264StreamFormat, SpaIoType, SpaMediaSubtype, SpaMediaType, SpaMetaType,
                SpaNodeCommand, SpaParamAvailability, SpaParamBitorder, SpaParamBuffers,
                SpaParamIo, SpaParamLatency, SpaParamMeta, SpaParamPortConfig,
                SpaParamPortConfigMode, SpaParamProcessLatency, SpaParamProfile, SpaParamRoute,
                SpaParamType, SpaProfiler, SpaProp, SpaPropInfo, SpaVideoChromaSite,
                SpaVideoColorMatrix, SpaVideoColorPrimaries, SpaVideoColorRange, SpaVideoFormat,
                SpaVideoInterlaceMode, SpaVideoMultiviewFlags, SpaVideoMultiviewMode,
                SpaVideoTransferFunction,
            },
        },
        utils::{debug_fn::debug_fn, errorfmt::ErrorFmt},
    },
    std::fmt::{Debug, DebugList, Formatter, Write},
};

trait PwPodObjectDebugger: Sync {
    fn debug_property(&self, fmt: &mut Formatter<'_>, value: PwProp<'_>) -> std::fmt::Result;
    fn id_name(&self, id: u32) -> Option<&'static str>;
}

struct PwPodObjectDebuggerSimple<F, G, H> {
    key_name: F,
    debug_pod: G,
    id_name: H,
}

impl<F, G, H> PwPodObjectDebugger for PwPodObjectDebuggerSimple<F, G, H>
where
    F: Fn(u32) -> Option<&'static str> + Sync,
    G: Fn(u32, &mut Formatter<'_>, PwPod<'_>) -> std::fmt::Result + Sync,
    H: Fn(u32) -> Option<&'static str> + Sync,
{
    fn debug_property(&self, fmt: &mut Formatter<'_>, value: PwProp<'_>) -> std::fmt::Result {
        let mut s = fmt.debug_struct("PwProp");
        match (self.key_name)(value.key) {
            Some(n) => s.field("key", &n),
            _ => s.field("key", &value.key),
        };
        s.field("flags", &value.flags)
            .field(
                "pod",
                &debug_fn(|f| (self.debug_pod)(value.key, f, value.pod)),
            )
            .finish()
    }

    fn id_name(&self, id: u32) -> Option<&'static str> {
        (self.id_name)(id)
    }
}

fn choice_debug<F>(fmt: &mut Formatter<'_>, p: PwPod<'_>, ty: PwPodType, f: F) -> std::fmt::Result
where
    F: Fn(&mut Formatter<'_>, PwPod<'_>) -> std::fmt::Result,
{
    match p {
        PwPod::Choice(c) if c.elements.ty == ty => fmt
            .debug_struct("choice")
            .field("ty", &c.ty)
            .field("flags", &c.flags)
            .field(
                "elements",
                &debug_fn(|fmt| {
                    array_body_debug(fmt, c.elements, |l, p| {
                        match p.read_pod_body_packed(ty, c.elements.child_len) {
                            Ok(p) => {
                                l.entry(&debug_fn(|fmt| f(fmt, p)));
                                true
                            }
                            Err(e) => {
                                let e = ErrorFmt(e);
                                l.entry(&debug_fn(|fmt| {
                                    write!(fmt, "Could not read choice element: {}", e)
                                }));
                                false
                            }
                        }
                    })
                }),
            )
            .finish(),
        _ if p.ty() == ty => f(fmt, p),
        _ => p.fmt(fmt),
    }
}

fn id_debug<F>(fmt: &mut Formatter<'_>, p: PwPod<'_>, f: F) -> std::fmt::Result
where
    F: Fn(&mut Formatter<'_>, u32) -> std::fmt::Result,
{
    choice_debug(fmt, p, PW_TYPE_Id, |fmt, p| match p {
        PwPod::Id(id) => f(fmt, id),
        _ => p.fmt(fmt),
    })
}

fn array_body_debug<F>(fmt: &mut Formatter<'_>, mut a: PwPodArray<'_>, f: F) -> std::fmt::Result
where
    F: Fn(&mut DebugList, &mut PwParser<'_>) -> bool,
{
    let mut l = fmt.debug_list();
    for _ in 0..a.n_elements {
        if !f(&mut l, &mut a.elements) {
            break;
        }
    }
    l.finish()
}

fn array_debug<F>(fmt: &mut Formatter<'_>, p: PwPod<'_>, ty: PwPodType, f: F) -> std::fmt::Result
where
    F: Fn(&mut DebugList, &mut PwParser<'_>) -> bool,
{
    match p {
        PwPod::Array(a) if a.ty == ty => array_body_debug(fmt, a, f),
        _ => p.fmt(fmt),
    }
}

fn array_id_debug<F, T>(fmt: &mut Formatter<'_>, p: PwPod<'_>, f: F) -> std::fmt::Result
where
    F: Fn(&mut DebugList, u32) -> T,
{
    array_debug(fmt, p, PW_TYPE_Id, |l, p| match p.read_id() {
        Ok(a) => {
            f(l, a);
            true
        }
        Err(e) => {
            let e = ErrorFmt(e);
            l.entry(&debug_fn(|f| write!(f, "Could not read id: {}", e)));
            false
        }
    })
}

fn object_id_name(id: u32) -> Option<&'static str> {
    SpaParamType(id).name()
}

fn command_id_name(id: u32) -> Option<&'static str> {
    SpaNodeCommand(id).name()
}

static PROP_INFO_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaPropInfo(key).name(),
    debug_pod: |_, f: &mut Formatter<'_>, p: PwPod<'_>| p.fmt(f),
    id_name: object_id_name,
};

static PROPS_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaProp(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaProp(key) {
        SPA_PROP_channelMap => array_id_debug(f, p, |l, a| {
            l.entry(&SpaAudioChannel(a));
        }),
        SPA_PROP_iec958Codecs => array_id_debug(f, p, |l, a| {
            l.entry(&SpaAudioIec958Codec(a));
        }),
        _ => p.fmt(f),
    },
};

static FORMAT_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaFormat(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaFormat(key) {
        SPA_FORMAT_mediaType => id_debug(f, p, |f, a| SpaMediaType(a).fmt(f)),
        SPA_FORMAT_mediaSubtype => id_debug(f, p, |f, a| SpaMediaSubtype(a).fmt(f)),
        SPA_FORMAT_AUDIO_format => id_debug(f, p, |f, a| SpaAudioFormat(a).fmt(f)),
        SPA_FORMAT_AUDIO_position => array_id_debug(f, p, |l, a| {
            l.entry(&SpaAudioChannel(a));
        }),
        SPA_FORMAT_AUDIO_iec958Codec => id_debug(f, p, |f, a| SpaAudioIec958Codec(a).fmt(f)),
        SPA_FORMAT_AUDIO_bitorder => id_debug(f, p, |f, a| SpaParamBitorder(a).fmt(f)),
        SPA_FORMAT_VIDEO_format => id_debug(f, p, |f, a| SpaVideoFormat(a).fmt(f)),
        SPA_FORMAT_VIDEO_interlaceMode => id_debug(f, p, |f, a| SpaVideoInterlaceMode(a).fmt(f)),
        SPA_FORMAT_VIDEO_multiviewMode => id_debug(f, p, |f, a| SpaVideoMultiviewMode(a).fmt(f)),
        SPA_FORMAT_VIDEO_multiviewFlags => id_debug(f, p, |f, a| SpaVideoMultiviewFlags(a).fmt(f)),
        SPA_FORMAT_VIDEO_chromaSite => id_debug(f, p, |f, a| SpaVideoChromaSite(a).fmt(f)),
        SPA_FORMAT_VIDEO_colorRange => id_debug(f, p, |f, a| SpaVideoColorRange(a).fmt(f)),
        SPA_FORMAT_VIDEO_colorMatrix => id_debug(f, p, |f, a| SpaVideoColorMatrix(a).fmt(f)),
        SPA_FORMAT_VIDEO_transferFunction => {
            id_debug(f, p, |f, a| SpaVideoTransferFunction(a).fmt(f))
        }
        SPA_FORMAT_VIDEO_colorPrimaries => id_debug(f, p, |f, a| SpaVideoColorPrimaries(a).fmt(f)),
        SPA_FORMAT_VIDEO_H264_streamFormat => id_debug(f, p, |f, a| SpaH264StreamFormat(a).fmt(f)),
        SPA_FORMAT_VIDEO_H264_alignment => id_debug(f, p, |f, a| SpaH264Alignment(a).fmt(f)),
        _ => p.fmt(f),
    },
};

static PARAM_BUFFERS_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamBuffers(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamBuffers(key) {
        SPA_PARAM_BUFFERS_dataType => match p {
            PwPod::Int(v) => SpaDataTypes(v as _).fmt(f),
            _ => p.fmt(f),
        },
        _ => p.fmt(f),
    },
};

static PARAM_META_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamMeta(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamMeta(key) {
        SPA_PARAM_META_type => id_debug(f, p, |f, b| SpaMetaType(b).fmt(f)),
        _ => p.fmt(f),
    },
};

static PARAM_IO_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamIo(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamIo(key) {
        SPA_PARAM_IO_id => id_debug(f, p, |f, b| SpaIoType(b).fmt(f)),
        _ => p.fmt(f),
    },
};

static PARAM_PROFILE_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamProfile(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamProfile(key) {
        SPA_PARAM_PROFILE_available => id_debug(f, p, |f, b| SpaParamAvailability(b).fmt(f)),
        _ => p.fmt(f),
    },
};

static PARAM_PORT_CONFIG_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamPortConfig(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamPortConfig(key) {
        SPA_PARAM_PORT_CONFIG_direction => id_debug(f, p, |f, b| SpaDirection(b).fmt(f)),
        SPA_PARAM_PORT_CONFIG_mode => id_debug(f, p, |f, b| SpaParamPortConfigMode(b).fmt(f)),
        _ => p.fmt(f),
    },
};

static PARAM_ROUTE_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamRoute(key).name(),
    id_name: object_id_name,
    debug_pod: |key, f: &mut Formatter<'_>, p: PwPod<'_>| match SpaParamRoute(key) {
        SPA_PARAM_ROUTE_direction => id_debug(f, p, |f, b| SpaDirection(b).fmt(f)),
        SPA_PARAM_ROUTE_available => id_debug(f, p, |f, b| SpaParamAvailability(b).fmt(f)),
        _ => p.fmt(f),
    },
};

static PROFILER_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaProfiler(key).name(),
    id_name: object_id_name,
    debug_pod: |_, f: &mut Formatter<'_>, p: PwPod<'_>| p.fmt(f),
};

static PARAM_LATENCY_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaParamLatency(key).name(),
    id_name: object_id_name,
    debug_pod: |_, f: &mut Formatter<'_>, p: PwPod<'_>| p.fmt(f),
};

static PARAM_PROCESS_LATENCY_DEBUGGER: &'static dyn PwPodObjectDebugger =
    &PwPodObjectDebuggerSimple {
        key_name: |key| SpaParamProcessLatency(key).name(),
        id_name: object_id_name,
        debug_pod: |_, f: &mut Formatter<'_>, p: PwPod<'_>| p.fmt(f),
    };

static COMMAND_NODE_DEBUGGER: &'static dyn PwPodObjectDebugger = &PwPodObjectDebuggerSimple {
    key_name: |key| SpaNodeCommand(key).name(),
    id_name: command_id_name,
    debug_pod: |_, f: &mut Formatter<'_>, p: PwPod<'_>| p.fmt(f),
};

fn object_debugger(obj: PwPodObjectType) -> Option<&'static dyn PwPodObjectDebugger> {
    let res: &dyn PwPodObjectDebugger = match obj {
        PW_OBJECT_PropInfo => PROP_INFO_DEBUGGER,
        PW_OBJECT_Props => PROPS_DEBUGGER,
        PW_OBJECT_Format => FORMAT_DEBUGGER,
        PW_OBJECT_ParamBuffers => PARAM_BUFFERS_DEBUGGER,
        PW_OBJECT_ParamMeta => PARAM_META_DEBUGGER,
        PW_OBJECT_ParamIO => PARAM_IO_DEBUGGER,
        PW_OBJECT_ParamProfile => PARAM_PROFILE_DEBUGGER,
        PW_OBJECT_ParamPortConfig => PARAM_PORT_CONFIG_DEBUGGER,
        PW_OBJECT_ParamRoute => PARAM_ROUTE_DEBUGGER,
        PW_OBJECT_Profiler => PROFILER_DEBUGGER,
        PW_OBJECT_ParamLatency => PARAM_LATENCY_DEBUGGER,
        PW_OBJECT_ParamProcessLatency => PARAM_PROCESS_LATENCY_DEBUGGER,
        PW_COMMAND_Node => COMMAND_NODE_DEBUGGER,
        _ => return None,
    };
    Some(res)
}

impl<'a> Debug for PwPodObject<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let debugger = object_debugger(self.ty);
        let mut s = f.debug_struct("object");
        s.field("type", &self.ty);
        let name;
        let mut id: &dyn Debug = &self.id;
        if let Some(d) = debugger
            && let Some(n) = d.id_name(self.id)
        {
            name = n;
            id = &name;
        }
        s.field("id", id);
        s.field(
            "props",
            &debug_fn(|f| {
                let mut l = f.debug_list();
                let mut parser = self.probs;
                while parser.len() > 0 {
                    match parser.read_prop() {
                        Ok(p) => match debugger {
                            Some(d) => l.entry(&debug_fn(|fmt| d.debug_property(fmt, p))),
                            _ => l.entry(&p),
                        },
                        Err(e) => {
                            let e = ErrorFmt(e);
                            l.entry(&debug_fn(|f| {
                                write!(f, "Could not read object property: {}", &e)
                            }));
                            break;
                        }
                    };
                }
                l.finish()
            }),
        );
        s.finish()
    }
}

impl<'a> Debug for PwPodSequence<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("sequence");
        s.field("unit", &self.unit);
        s.field(
            "controls",
            &debug_fn(|f| {
                let mut l = f.debug_list();
                let mut parser = self.controls;
                while parser.len() > 0 {
                    match parser.read_control() {
                        Ok(c) => l.entry(&c),
                        Err(e) => {
                            let e = ErrorFmt(e);
                            l.entry(&debug_fn(|f| {
                                write!(f, "Could not read control element: {}", &e)
                            }));
                            break;
                        }
                    };
                }
                l.finish()
            }),
        );
        s.finish()
    }
}

impl<'a> Debug for PwPodStruct<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut parser = self.fields;
        let mut s = f.debug_struct("struct");
        let mut field = String::new();
        for i in 0.. {
            if parser.len() == 0 {
                break;
            }
            field.clear();
            let _ = write!(&mut field, "\"{}\"", i);
            match parser.read_pod() {
                Ok(p) => s.field(&field, &p),
                Err(e) => {
                    let e = ErrorFmt(e);
                    s.field(
                        &field,
                        &debug_fn(|f| write!(f, "Could not parse struct field: {}", &e)),
                    );
                    break;
                }
            };
        }
        s.finish()
    }
}

impl<'a> Debug for PwPodArray<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();
        let mut parser = self.elements;
        for _ in 0..self.n_elements {
            match parser.read_pod_body_packed(self.ty, self.child_len) {
                Ok(e) => list.entry(&e),
                Err(e) => {
                    let e = ErrorFmt(e);
                    list.entry(&debug_fn(|f| {
                        write!(f, "Could not parse array element: {}", &e)
                    }));
                    break;
                }
            };
        }
        list.finish()
    }
}

impl<'a> Debug for PwPod<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PwPod::None => write!(f, "None"),
            PwPod::Bool(b) => write!(f, "{}", b),
            PwPod::Id(id) => write!(f, "id({})", id),
            PwPod::Int(i) => write!(f, "int({})", i),
            PwPod::Long(l) => write!(f, "long({})", l),
            PwPod::Float(v) => write!(f, "float({})", v),
            PwPod::Double(d) => write!(f, "double({})", d),
            PwPod::String(s) => write!(f, "string({:?})", s),
            PwPod::Bytes(b) => write!(f, "bytes(len = {})", b.len()),
            PwPod::Rectangle(r) => write!(f, "rectangle({}x{})", r.width, r.height),
            PwPod::Fraction(v) => write!(f, "fraction({}/{})", v.num, v.denom),
            PwPod::Bitmap(b) => write!(f, "bitmap(len = {})", b.len()),
            PwPod::Array(a) => a.fmt(f),
            PwPod::Struct(s) => s.fmt(f),
            PwPod::Object(o) => o.fmt(f),
            PwPod::Sequence(s) => s.fmt(f),
            PwPod::Pointer(p) => p.fmt(f),
            PwPod::Fd(v) => write!(f, "fd({})", v),
            PwPod::Choice(c) => c.fmt(f),
        }
    }
}
