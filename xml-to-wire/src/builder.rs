use {
    crate::{
        ast::{ArgType, Message, MessageType},
        parser::{ParserError, parse},
    },
    std::{
        fs::File,
        io::{self, BufWriter, Write},
        mem,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("Could not process {0}")]
    File(String, #[source] Box<BuilderError>),
    #[error("Could not read file")]
    ReadFile(#[source] io::Error),
    #[error("Could not parse file")]
    ParseFile(#[source] ParserError),
    #[error("Could not format file")]
    FormatFile(#[source] io::Error),
    #[error("Could not open {0} for writing")]
    OpenFile(String, #[source] io::Error),
}

pub fn handle_file(path: &str) -> Result<(), BuilderError> {
    handle_file_(path).map_err(|e| BuilderError::File(path.to_string(), Box::new(e)))
}

fn handle_file_(path: &str) -> Result<(), BuilderError> {
    let data = std::fs::read(path).map_err(BuilderError::ReadFile)?;
    let protocols = parse(&data).map_err(BuilderError::ParseFile)?;
    for protocol in protocols {
        for i in protocol.interfaces {
            if matches!(&*i.name, "wl_shell" | "wl_shell_surface") {
                continue;
            }
            let path = format!("wire/{}.txt", i.name);
            let mut file =
                BufWriter::new(File::create(&path).map_err(|e| BuilderError::OpenFile(path, e))?);
            let mut not_first = false;
            let mut handle_msg = |msg: &Message| -> Result<(), io::Error> {
                if not_first {
                    writeln!(file)?;
                }
                not_first = true;
                let ty = match msg.request {
                    true => "request",
                    false => "event",
                };
                write!(file, "{ty} {} ", msg.name)?;
                if msg.ty.is_some() || msg.since.is_some() {
                    write!(file, "(")?;
                    let mut needs_comma = false;
                    let handle_comma =
                        |needs_comma: &mut bool, file: &mut BufWriter<File>| -> io::Result<()> {
                            if mem::take(needs_comma) {
                                write!(file, ", ")?;
                            }
                            Ok(())
                        };
                    if let Some(ty) = msg.ty {
                        match ty {
                            MessageType::Destructor => {
                                handle_comma(&mut needs_comma, &mut file)?;
                                write!(file, "destructor")?;
                                needs_comma = true;
                            }
                        }
                    }
                    if let Some(s) = msg.since {
                        handle_comma(&mut needs_comma, &mut file)?;
                        write!(file, "since = {}", s)?;
                        needs_comma = true;
                    }
                    let _ = needs_comma;
                    write!(file, ") ")?;
                }
                writeln!(file, "{{")?;
                let mut args = msg.args.iter().peekable();
                while let Some(arg) = args.next() {
                    if arg.ty == ArgType::Uint
                        && arg.enum_.is_none()
                        && let Some(prefix) = arg.name.strip_suffix("_hi")
                        && let Some(next) = args.peek()
                        && next.ty == ArgType::Uint
                        && next.enum_.is_none()
                        && next.name.strip_prefix(prefix) == Some("_lo")
                    {
                        writeln!(file, "    {prefix}: u64,")?;
                        args.next();
                        continue;
                    }
                    if arg.ty == ArgType::Uint
                        && arg.enum_.is_none()
                        && let Some(prefix) = arg.name.strip_suffix("_lo")
                        && let Some(next) = args.peek()
                        && next.ty == ArgType::Uint
                        && next.enum_.is_none()
                        && next.name.strip_prefix(prefix) == Some("_hi")
                    {
                        writeln!(file, "    {prefix}: u64_rev,")?;
                        args.next();
                        continue;
                    }
                    if arg.ty == ArgType::NewId && arg.interface.is_none() {
                        writeln!(file, "    interface: str,")?;
                        writeln!(file, "    version: u32,")?;
                    }
                    write!(file, "    {}: ", arg.name)?;
                    'ty: {
                        let ty = match arg.ty {
                            ArgType::NewId | ArgType::Object => {
                                write!(
                                    file,
                                    "id({})",
                                    arg.interface.as_deref().unwrap_or("object")
                                )?;
                                if arg.ty == ArgType::NewId {
                                    write!(file, " (new)")?;
                                }
                                break 'ty;
                            }
                            ArgType::Int => "i32",
                            ArgType::Uint => "u32",
                            ArgType::Fixed => "fixed",
                            ArgType::String => match arg.allow_null {
                                true => "optstr",
                                false => "str",
                            },
                            ArgType::Array => match (&*i.name, &*msg.name, &*arg.name) {
                                ("wl_keyboard", "enter", "keys") => "array(u32)",
                                (
                                    "ext_image_copy_capture_session_v1",
                                    "dmabuf_device",
                                    "device",
                                ) => "pod(uapi::c::dev_t)",
                                ("ext_workspace_handle_v1", "coordinates", "coordinates") => {
                                    "array(u32)"
                                }
                                ("xdg_toplevel", "configure", "states") => "array(u32)",
                                ("xdg_toplevel", "wm_capabilities", "capabilities") => "array(u32)",
                                ("zwp_linux_dmabuf_feedback_v1", "main_device", "device") => {
                                    "pod(uapi::c::dev_t)"
                                }
                                (
                                    "zwp_linux_dmabuf_feedback_v1",
                                    "tranche_target_device",
                                    "device",
                                ) => "pod(uapi::c::dev_t)",
                                ("zwp_linux_dmabuf_feedback_v1", "tranche_formats", "indices") => {
                                    "array(pod(u16))"
                                }
                                ("zwp_tablet_pad_group_v2", "buttons", "buttons") => "array(u32)",
                                _ => "array(pod(u8))",
                            },
                            ArgType::Fd => "fd",
                        };
                        write!(file, "{}", ty)?;
                    }
                    writeln!(file, ",")?;
                }
                writeln!(file, "}}")?;
                Ok(())
            };
            for m in &i.messages {
                handle_msg(m).map_err(BuilderError::FormatFile)?;
            }
        }
    }
    Ok(())
}
