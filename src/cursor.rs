use {
    crate::{
        format::ARGB8888,
        rect::Rect,
        render::{RenderContext, RenderError, Renderer, Texture},
        utils::{errorfmt::ErrorFmt, numcell::NumCell},
    },
    ahash::AHashSet,
    bstr::{BStr, BString, ByteSlice, ByteVec},
    byteorder::{LittleEndian, ReadBytesExt},
    isnt::std_1::primitive::IsntSliceExt,
    std::{
        cell::Cell,
        convert::TryInto,
        env,
        fmt::{Debug, Formatter},
        fs::File,
        io::{self, BufRead, BufReader, Seek, SeekFrom},
        mem::MaybeUninit,
        rc::Rc,
        slice, str,
    },
    thiserror::Error,
    uapi::c,
};

const XCURSOR_MAGIC: u32 = 0x72756358;
const XCURSOR_IMAGE_TYPE: u32 = 0xfffd0002;
const XCURSOR_PATH_DEFAULT: &[u8] =
    b"~/.icons:/usr/share/icons:/usr/share/pixmaps:/usr/X11R6/lib/X11/icons";
const XCURSOR_PATH: &str = "XCURSOR_PATH";
const HOME: &str = "HOME";

const HEADER_SIZE: u32 = 16;

pub trait Cursor {
    fn set_position(&self, x: i32, y: i32);
    fn render(&self, renderer: &mut Renderer, x: i32, y: i32);
    fn get_hotspot(&self) -> (i32, i32);
    fn extents(&self) -> Rect;
    fn handle_unset(&self) {}
    fn tick(&self) {}
}

pub struct ServerCursors {
    pub default: ServerCursorTemplate,
    pub resize_right: ServerCursorTemplate,
    pub resize_left: ServerCursorTemplate,
    pub resize_top: ServerCursorTemplate,
    pub resize_bottom: ServerCursorTemplate,
    pub resize_top_bottom: ServerCursorTemplate,
    pub resize_left_right: ServerCursorTemplate,
    pub resize_top_left: ServerCursorTemplate,
    pub resize_top_right: ServerCursorTemplate,
    pub resize_bottom_left: ServerCursorTemplate,
    pub resize_bottom_right: ServerCursorTemplate,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum KnownCursor {
    Default,
    ResizeLeftRight,
    ResizeTopBottom,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
}

impl ServerCursors {
    pub fn load(ctx: &Rc<RenderContext>) -> Result<Self, CursorError> {
        let paths = find_cursor_paths();
        log::debug!("Trying to load cursors from paths {:?}", paths);
        let load = |name: &str| ServerCursorTemplate::load(name, None, 16, &paths, ctx);
        Ok(Self {
            default: load("left_ptr")?,
            // default: load("left_ptr_watch")?,
            resize_right: load("right_side")?,
            resize_left: load("left_side")?,
            resize_top: load("top_side")?,
            resize_bottom: load("bottom_side")?,
            resize_top_bottom: load("v_double_arrow")?,
            resize_left_right: load("h_double_arrow")?,
            resize_top_left: load("top_left_corner")?,
            resize_top_right: load("top_right_corner")?,
            resize_bottom_left: load("bottom_left_corner")?,
            resize_bottom_right: load("bottom_right_corner")?,
        })
    }
}

pub struct ServerCursorTemplate {
    var: ServerCursorTemplateVariant,
    pub xcursor: Vec<XCursorImage>,
}

enum ServerCursorTemplateVariant {
    Static(Rc<CursorImage>),
    Animated(Rc<Vec<CursorImage>>),
}

impl ServerCursorTemplate {
    fn load(
        name: &str,
        theme: Option<&BStr>,
        size: u32,
        paths: &[BString],
        ctx: &Rc<RenderContext>,
    ) -> Result<Self, CursorError> {
        match open_cursor(name, theme, size, paths) {
            Ok(cs) => {
                if cs.len() == 1 {
                    let c = &cs[0];
                    let cursor = CursorImage::from_bytes(
                        ctx, &c.pixels, 0, c.width, c.height, c.xhot, c.yhot,
                    )?;
                    Ok(ServerCursorTemplate {
                        var: ServerCursorTemplateVariant::Static(Rc::new(cursor)),
                        xcursor: cs,
                    })
                } else {
                    let mut images = vec![];
                    for c in &cs {
                        let img = CursorImage::from_bytes(
                            ctx,
                            &c.pixels,
                            c.delay as _,
                            c.width,
                            c.height,
                            c.xhot,
                            c.yhot,
                        )?;
                        images.push(img);
                    }
                    Ok(ServerCursorTemplate {
                        var: ServerCursorTemplateVariant::Animated(Rc::new(images)),
                        xcursor: cs,
                    })
                }
            }
            Err(e) => {
                log::warn!("Could not load cursor {}: {}", name, ErrorFmt(e));
                let empty: [Cell<u8>; 4] = unsafe { MaybeUninit::zeroed().assume_init() };
                let cursor = CursorImage::from_bytes(ctx, &empty, 0, 1, 1, 0, 0)?;
                Ok(ServerCursorTemplate {
                    var: ServerCursorTemplateVariant::Static(Rc::new(cursor)),
                    xcursor: Default::default(),
                })
            }
        }
    }

    pub fn instantiate(&self) -> Rc<dyn Cursor> {
        match &self.var {
            ServerCursorTemplateVariant::Static(s) => Rc::new(StaticCursor {
                x: Cell::new(0),
                y: Cell::new(0),
                extents: Cell::new(s.extents),
                image: s.clone(),
            }),
            ServerCursorTemplateVariant::Animated(a) => {
                let mut start = c::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                };
                uapi::clock_gettime(c::CLOCK_MONOTONIC, &mut start).unwrap();
                Rc::new(AnimatedCursor {
                    start,
                    next: NumCell::new(a[0].delay_ns),
                    idx: Cell::new(0),
                    images: a.clone(),
                    x: Cell::new(0),
                    y: Cell::new(0),
                    extents: Cell::new(a[0].extents),
                })
            }
        }
    }
}

struct CursorImage {
    extents: Rect,
    xhot: i32,
    yhot: i32,
    delay_ns: u64,
    tex: Rc<Texture>,
}

impl CursorImage {
    fn from_bytes(
        ctx: &Rc<RenderContext>,
        data: &[Cell<u8>],
        delay_ms: u64,
        width: i32,
        height: i32,
        xhot: i32,
        yhot: i32,
    ) -> Result<Self, CursorError> {
        Ok(Self {
            extents: Rect::new_sized(-xhot, -yhot, width, height).unwrap(),
            xhot,
            yhot,
            delay_ns: delay_ms * 1_000_000,
            tex: ctx.shmem_texture(data, ARGB8888, width, height, width * 4)?,
        })
    }
}

struct StaticCursor {
    x: Cell<i32>,
    y: Cell<i32>,
    extents: Cell<Rect>,
    image: Rc<CursorImage>,
}

impl Cursor for StaticCursor {
    fn set_position(&self, x: i32, y: i32) {
        let dx = x - self.x.replace(x);
        let dy = y - self.y.replace(y);
        self.extents.set(self.extents.get().move_(dx, dy));
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_texture(&self.image.tex, x, y, ARGB8888, None, None);
    }

    fn get_hotspot(&self) -> (i32, i32) {
        (self.image.xhot, self.image.yhot)
    }

    fn extents(&self) -> Rect {
        self.extents.get()
    }
}

struct AnimatedCursor {
    start: c::timespec,
    next: NumCell<u64>,
    idx: Cell<usize>,
    images: Rc<Vec<CursorImage>>,
    x: Cell<i32>,
    y: Cell<i32>,
    extents: Cell<Rect>,
}

impl Cursor for AnimatedCursor {
    fn set_position(&self, x: i32, y: i32) {
        let dx = x - self.x.replace(x);
        let dy = y - self.y.replace(y);
        self.extents.set(self.extents.get().move_(dx, dy));
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        let img = &self.images[self.idx.get()];
        renderer.render_texture(&img.tex, x, y, ARGB8888, None, None);
    }

    fn get_hotspot(&self) -> (i32, i32) {
        let img = &self.images[self.idx.get()];
        (img.xhot, img.yhot)
    }

    fn extents(&self) -> Rect {
        self.extents.get()
    }

    fn tick(&self) {
        let mut now = c::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        uapi::clock_gettime(c::CLOCK_MONOTONIC, &mut now).unwrap();
        let dist = (now.tv_sec.wrapping_sub(self.start.tv_sec)) as i64 * 1_000_000_000
            + now.tv_nsec.wrapping_sub(self.start.tv_nsec) as i64;
        if (dist as u64) < self.next.get() {
            return;
        }
        let idx = (self.idx.get() + 1) % self.images.len();
        self.idx.set(idx);
        let image = &self.images[idx];
        self.extents.set(
            Rect::new_sized(
                self.x.get() - image.xhot,
                self.y.get() - image.yhot,
                image.extents.width(),
                image.extents.height(),
            )
            .unwrap(),
        );
        self.next.fetch_add(image.delay_ns);
    }
}

fn open_cursor(
    name: &str,
    theme: Option<&BStr>,
    size: u32,
    paths: &[BString],
) -> Result<Vec<XCursorImage>, CursorError> {
    let name = name.as_bytes().as_bstr();
    let mut file = None;
    let mut themes_tested = AHashSet::new();
    if let Some(theme) = theme {
        file = open_cursor_file(&mut themes_tested, paths, theme, name);
    }
    if file.is_none() {
        file = open_cursor_file(&mut themes_tested, paths, b"default".as_bstr(), name);
    }
    let file = match file {
        Some(f) => f,
        _ => return Err(CursorError::NotFound),
    };
    let mut file = BufReader::new(file);
    let images = parser_cursor_file(&mut file, size)?;
    if images.is_empty() {
        return Err(CursorError::EmptyXcursorFile);
    }
    Ok(images)
}

fn open_cursor_file(
    themes_tested: &mut AHashSet<BString>,
    paths: &[BString],
    theme: &BStr,
    name: &BStr,
) -> Option<File> {
    if !themes_tested.insert(theme.to_owned()) {
        return None;
    }
    if paths.is_empty() {
        return None;
    }
    let mut parents = None;
    for cursor_path in paths {
        let mut theme_dir = cursor_path.to_vec();
        theme_dir.push(b'/');
        theme_dir.extend_from_slice(theme.as_bytes());
        let mut cursor_file = theme_dir.clone();
        cursor_file.extend_from_slice(b"/cursors/");
        cursor_file.extend_from_slice(name.as_bytes());
        if let Ok(f) = File::open(cursor_file.to_os_str().unwrap()) {
            return Some(f);
        }
        if parents.is_none() {
            let mut index_file = theme_dir.clone();
            index_file.extend_from_slice(b"/index.theme");
            parents = find_parent_themes(&index_file);
        }
    }
    if let Some(parents) = parents {
        for parent in parents {
            if let Some(file) = open_cursor_file(themes_tested, paths, parent.as_bstr(), name) {
                return Some(file);
            }
        }
    }
    None
}

fn find_cursor_paths() -> Vec<BString> {
    let home = env::var_os(HOME).map(|h| Vec::from_os_string(h).unwrap());
    let cursor_paths = env::var_os(XCURSOR_PATH);
    let cursor_paths = cursor_paths
        .as_ref()
        .map(|c| <[u8]>::from_os_str(c).unwrap())
        .unwrap_or(XCURSOR_PATH_DEFAULT);
    let mut paths = vec![];
    for path in <[u8]>::split(cursor_paths, |b| *b == b':') {
        if path.first() == Some(&b'~') {
            if let Some(home) = home.as_ref() {
                let mut full_path = home.clone();
                full_path.extend_from_slice(&path[1..]);
                paths.push(full_path.into());
            } else {
                log::warn!(
                    "`HOME` is not set. Cannot expand {}. Ignoring.",
                    path.as_bstr()
                );
            }
        } else {
            paths.push(path.as_bstr().to_owned());
        }
    }
    paths
}

fn find_parent_themes(path: &[u8]) -> Option<Vec<BString>> {
    // NOTE: The files we're reading here are really INI files with a hierarchy. This
    // algorithm treats it as a flat list and is inherited from libxcursor.
    let file = match File::open(path.to_os_str().unwrap()) {
        Ok(f) => f,
        _ => return None,
    };
    let mut buf_reader = BufReader::new(file);
    let mut buf = vec![];
    loop {
        buf.clear();
        match buf_reader.read_until(b'\n', &mut buf) {
            Ok(n) if n > 0 => {}
            _ => return None,
        }
        let mut suffix = match buf.strip_prefix(b"Inherits") {
            Some(s) => s,
            _ => continue,
        };
        while suffix.first() == Some(&b' ') {
            suffix = &suffix[1..];
        }
        if suffix.first() != Some(&b'=') {
            continue;
        }
        suffix = &suffix[1..];
        let parents = suffix
            .split(|b| matches!(*b, b' ' | b'\t' | b'\n' | b';' | b','))
            .filter(|v| v.is_not_empty())
            .map(|v| v.as_bstr().to_owned())
            .collect();
        return Some(parents);
    }
}

#[derive(Debug, Error)]
pub enum CursorError {
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
    #[error("The file is not an Xcursor file")]
    NotAnXcursorFile,
    #[error("The Xcursor file contains more than 0x10000 images")]
    OversizedXcursorFile,
    #[error("The Xcursor file is empty")]
    EmptyXcursorFile,
    #[error("The Xcursor file is corrupt")]
    CorruptXcursorFile,
    #[error("The requested cursor could not be found")]
    NotFound,
    #[error("Could not import the cursor as a texture")]
    ImportError(#[from] RenderError),
}

#[derive(Default, Clone)]
pub struct XCursorImage {
    pub width: i32,
    pub height: i32,
    pub xhot: i32,
    pub yhot: i32,
    pub delay: u32,
    pub pixels: Vec<Cell<u8>>,
}

impl Debug for XCursorImage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XcbCursorImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("xhot", &self.xhot)
            .field("yhot", &self.yhot)
            .field("delay", &self.delay)
            .finish_non_exhaustive()
    }
}

fn parser_cursor_file<R: BufRead + Seek>(
    r: &mut R,
    target: u32,
) -> Result<Vec<XCursorImage>, CursorError> {
    let [magic, header] = read_u32_n(r)?;
    if magic != XCURSOR_MAGIC || header < HEADER_SIZE {
        return Err(CursorError::NotAnXcursorFile);
    }
    let [_version, ntoc] = read_u32_n(r)?;
    r.seek(SeekFrom::Current((HEADER_SIZE - header) as i64))?;
    if ntoc > 0x10000 {
        return Err(CursorError::OversizedXcursorFile);
    }
    let mut images_positions = vec![];
    let mut best_fit = i64::MAX;
    for _ in 0..ntoc {
        let [type_, size, position] = read_u32_n(r)?;
        if type_ != XCURSOR_IMAGE_TYPE {
            continue;
        }
        let fit = (size as i64 - target as i64).abs();
        if fit < best_fit {
            best_fit = fit;
            images_positions.clear();
        }
        if fit == best_fit {
            images_positions.push(position);
        }
    }
    let mut images = Vec::with_capacity(images_positions.len());
    for position in images_positions {
        r.seek(SeekFrom::Start(position as u64))?;
        let [_chunk_header, _type_, _size, _version, width, height, xhot, yhot, delay] =
            read_u32_n(r)?;
        let [width, height, xhot, yhot] = u32_to_i32([width, height, xhot, yhot])?;
        let mut image = XCursorImage {
            width,
            height,
            xhot,
            yhot,
            delay,
            pixels: vec![],
        };
        let num_bytes = width as usize * height as usize * 4;
        unsafe {
            image.pixels.reserve_exact(num_bytes as usize);
            image.pixels.set_len(num_bytes as usize);
            r.read_exact(slice::from_raw_parts_mut(
                image.pixels.as_mut_ptr() as _,
                num_bytes,
            ))?;
        }
        images.push(image);
    }
    Ok(images)
}

fn read_u32_n<R: BufRead, const N: usize>(r: &mut R) -> Result<[u32; N], io::Error> {
    let mut res = [0; N];
    r.read_u32_into::<LittleEndian>(&mut res)?;
    Ok(res)
}

fn u32_to_i32<const N: usize>(n: [u32; N]) -> Result<[i32; N], CursorError> {
    let mut res = [0; N];
    for i in 0..N {
        res[i] = n[i]
            .try_into()
            .map_err(|_| CursorError::CorruptXcursorFile)?;
    }
    Ok(res)
}
