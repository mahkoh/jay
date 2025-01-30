use {
    crate::{
        async_engine::AsyncEngine,
        fixed::Fixed,
        format::ARGB8888,
        gfx_api::{AcquireSync, GfxContext, GfxError, GfxTexture, ReleaseSync},
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        state::State,
        time::Time,
        tree::OutputNode,
        utils::{errorfmt::ErrorFmt, numcell::NumCell, smallmap::SmallMapMut},
    },
    ahash::{AHashMap, AHashSet},
    bstr::{BStr, BString, ByteSlice, ByteVec},
    byteorder::{LittleEndian, ReadBytesExt},
    isnt::std_1::primitive::IsntSliceExt,
    num_derive::FromPrimitive,
    once_cell::sync::Lazy,
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
        time::Duration,
    },
    thiserror::Error,
    uapi::Bytes,
};

const XCURSOR_MAGIC: u32 = 0x72756358;
const XCURSOR_IMAGE_TYPE: u32 = 0xfffd0002;
const XCURSOR_PATH_DEFAULT: &[u8] =
    b"~/.icons:/usr/share/icons:/usr/share/pixmaps:/usr/X11R6/lib/X11/icons";
const XCURSOR_PATH: &str = "XCURSOR_PATH";
const XCURSOR_THEME: &str = "XCURSOR_THEME";
const XCURSOR_SIZE: &str = "XCURSOR_SIZE";
const HOME: &str = "HOME";

const HEADER_SIZE: u32 = 16;

pub static DEFAULT_CURSOR_SIZE: Lazy<u32> = Lazy::new(|| {
    if let Ok(size) = env::var(XCURSOR_SIZE) {
        if let Ok(val) = size.parse() {
            return val;
        }
    }
    24
});

pub trait Cursor {
    fn render(&self, renderer: &mut Renderer, x: Fixed, y: Fixed);
    fn render_hardware_cursor(&self, renderer: &mut Renderer);
    fn extents_at_scale(&self, scale: Scale) -> Rect;
    fn set_output(&self, output: &Rc<OutputNode>) {
        let _ = output;
    }
    fn handle_set(self: Rc<Self>) {}
    fn handle_unset(&self) {}
    fn tick(&self) {}
    fn needs_tick(&self) -> bool {
        false
    }
    fn time_until_tick(&self) -> Duration {
        Duration::new(0, 0)
    }

    fn set_visible(&self, visible: bool) {
        let _ = visible;
    }
}

pub struct ServerCursors {
    pub default: ServerCursorTemplate,
    pub context_menu: ServerCursorTemplate,
    pub help: ServerCursorTemplate,
    pub pointer: ServerCursorTemplate,
    pub progress: ServerCursorTemplate,
    pub wait: ServerCursorTemplate,
    pub cell: ServerCursorTemplate,
    pub crosshair: ServerCursorTemplate,
    pub text: ServerCursorTemplate,
    pub vertical_text: ServerCursorTemplate,
    pub alias: ServerCursorTemplate,
    pub copy: ServerCursorTemplate,
    pub r#move: ServerCursorTemplate,
    pub no_drop: ServerCursorTemplate,
    pub not_allowed: ServerCursorTemplate,
    pub grab: ServerCursorTemplate,
    pub grabbing: ServerCursorTemplate,
    pub e_resize: ServerCursorTemplate,
    pub n_resize: ServerCursorTemplate,
    pub ne_resize: ServerCursorTemplate,
    pub nw_resize: ServerCursorTemplate,
    pub s_resize: ServerCursorTemplate,
    pub se_resize: ServerCursorTemplate,
    pub sw_resize: ServerCursorTemplate,
    pub w_resize: ServerCursorTemplate,
    pub ew_resize: ServerCursorTemplate,
    pub ns_resize: ServerCursorTemplate,
    pub nesw_resize: ServerCursorTemplate,
    pub nwse_resize: ServerCursorTemplate,
    pub col_resize: ServerCursorTemplate,
    pub row_resize: ServerCursorTemplate,
    pub all_scroll: ServerCursorTemplate,
    pub zoom_in: ServerCursorTemplate,
    pub zoom_out: ServerCursorTemplate,
    pub dnd_ask: ServerCursorTemplate,
    pub all_resize: ServerCursorTemplate,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, FromPrimitive)]
pub enum KnownCursor {
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
    DndAsk,
    AllResize,
}

impl ServerCursors {
    pub fn load(ctx: &Rc<dyn GfxContext>, state: &State) -> Result<Option<Self>, CursorError> {
        let paths = find_cursor_paths();
        log::debug!("Trying to load cursors from paths {:?}", paths);
        let sizes = state.cursor_sizes.to_vec();
        let scales = state.scales.to_vec();
        if sizes.is_empty() || scales.is_empty() {
            return Ok(None);
        }
        let xcursor_theme = env::var_os(XCURSOR_THEME);
        let theme = xcursor_theme.as_ref().map(|theme| BStr::new(theme.bytes()));

        let load =
            |names: &[&str]| ServerCursorTemplate::load(names, theme, &scales, &sizes, &paths, ctx);
        Ok(Some(Self {
            // default: load(&["wait", "watch"])?,
            default: load(&["default", "left_ptr"])?,
            context_menu: load(&["context-menu"])?,
            help: load(&["help"])?,
            pointer: load(&["pointer", "hand2", "hand1"])?,
            progress: load(&["progress"])?,
            wait: load(&["wait", "watch"])?,
            cell: load(&["cell"])?,
            crosshair: load(&["crosshair"])?,
            text: load(&["text", "xterm"])?,
            vertical_text: load(&["vertical-text"])?,
            alias: load(&["alias"])?,
            copy: load(&["copy"])?,
            r#move: load(&["move"])?,
            no_drop: load(&["no-drop"])?,
            not_allowed: load(&["not-allowed"])?,
            grab: load(&["grab"])?,
            grabbing: load(&["grabbing"])?,
            e_resize: load(&["e-resize", "right_side"])?,
            w_resize: load(&["w-resize", "left_side"])?,
            n_resize: load(&["n-resize", "top_side"])?,
            s_resize: load(&["s-resize", "bottom_side"])?,
            ns_resize: load(&["ns-resize", "v_double_arrow"])?,
            ew_resize: load(&["ew-resize", "h_double_arrow"])?,
            nw_resize: load(&["nw-resize", "top_left_corner"])?,
            ne_resize: load(&["ne-resize", "top_right_corner"])?,
            sw_resize: load(&["sw-resize", "bottom_left_corner"])?,
            se_resize: load(&["se-resize", "bottom_right_corner"])?,
            nesw_resize: load(&["nesw-resize"])?,
            nwse_resize: load(&["nwse-resize"])?,
            col_resize: load(&["col-resize"])?,
            row_resize: load(&["row-resize"])?,
            all_scroll: load(&["all-scroll", "grabbing"])?,
            zoom_in: load(&["zoom-in"])?,
            zoom_out: load(&["zoom-out"])?,
            dnd_ask: load(&["dnd-ask", "dnd-copy", "copy"])?,
            all_resize: load(&["all-resize", "move"])?,
        }))
    }
}

pub struct ServerCursorTemplate {
    var: ServerCursorTemplateVariant,
    pub xcursor: Vec<AHashMap<(Scale, u32), Rc<XCursorImage>>>,
}

enum ServerCursorTemplateVariant {
    Static(Rc<CursorImage>),
    Animated(Rc<Vec<CursorImage>>),
}

impl ServerCursorTemplate {
    fn load(
        names: &[&str],
        theme: Option<&BStr>,
        scales: &[Scale],
        sizes: &[u32],
        paths: &[BString],
        ctx: &Rc<dyn GfxContext>,
    ) -> Result<Self, CursorError> {
        match open_cursor(names, theme, scales, sizes, paths) {
            Ok(cs) => {
                if cs.images.len() == 1 {
                    let mut sizes = SmallMapMut::new();
                    for (k, c) in &cs.images[0] {
                        sizes.insert(
                            *k,
                            CursorImageScaled::from_bytes(
                                ctx, &c.pixels, c.width, c.height, c.xhot, c.yhot,
                            )?,
                        );
                    }
                    let cursor = CursorImage::from_sizes(0, sizes)?;
                    Ok(ServerCursorTemplate {
                        var: ServerCursorTemplateVariant::Static(Rc::new(cursor)),
                        xcursor: cs.images,
                    })
                } else {
                    let mut images = vec![];
                    for image in &cs.images {
                        let mut sizes = SmallMapMut::new();
                        let mut delay_ms = 0;
                        for (k, c) in image {
                            delay_ms = c.delay;
                            sizes.insert(
                                *k,
                                CursorImageScaled::from_bytes(
                                    ctx, &c.pixels, c.width, c.height, c.xhot, c.yhot,
                                )?,
                            );
                        }
                        let img = CursorImage::from_sizes(delay_ms as _, sizes)?;
                        images.push(img);
                    }
                    Ok(ServerCursorTemplate {
                        var: ServerCursorTemplateVariant::Animated(Rc::new(images)),
                        xcursor: cs.images,
                    })
                }
            }
            Err(e) => {
                log::warn!("Could not load cursor {:?}: {}", names, ErrorFmt(e));
                let empty: [Cell<u8>; 4] = unsafe { MaybeUninit::zeroed().assume_init() };
                let mut img_sizes = SmallMapMut::new();
                for scale in scales {
                    for size in sizes {
                        img_sizes.insert(
                            (*scale, *size),
                            CursorImageScaled::from_bytes(ctx, &empty, 1, 1, 0, 0)?,
                        );
                    }
                }
                let cursor = CursorImage::from_sizes(0, img_sizes)?;
                Ok(ServerCursorTemplate {
                    var: ServerCursorTemplateVariant::Static(Rc::new(cursor)),
                    xcursor: Default::default(),
                })
            }
        }
    }

    pub fn instantiate(&self, state: &State, size: u32) -> Rc<dyn Cursor> {
        match &self.var {
            ServerCursorTemplateVariant::Static(s) => Rc::new(StaticCursor {
                image: s.for_size(size),
            }),
            ServerCursorTemplateVariant::Animated(a) => Rc::new(AnimatedCursor {
                start: state.now(),
                eng: state.eng.clone(),
                next: NumCell::new(a[0].delay_ns),
                idx: Cell::new(0),
                images: a.iter().map(|c| c.for_size(size)).collect(),
            }),
        }
    }
}

struct CursorImageScaled {
    extents: Rect,
    tex: Rc<dyn GfxTexture>,
}

struct CursorImage {
    delay_ns: u64,
    sizes: SmallMapMut<(Scale, u32), Rc<CursorImageScaled>, 2>,
}

struct InstantiatedCursorImage {
    delay_ns: u64,
    scales: SmallMapMut<Scale, Rc<CursorImageScaled>, 2>,
}

impl CursorImageScaled {
    fn from_bytes(
        ctx: &Rc<dyn GfxContext>,
        data: &[Cell<u8>],
        width: i32,
        height: i32,
        xhot: i32,
        yhot: i32,
    ) -> Result<Rc<Self>, CursorError> {
        Ok(Rc::new(Self {
            extents: Rect::new_sized(-xhot, -yhot, width, height).unwrap(),
            tex: ctx
                .clone()
                .shmem_texture(None, data, ARGB8888, width, height, width * 4, None)?
                .into_texture(),
        }))
    }
}

impl CursorImage {
    fn from_sizes(
        delay_ms: u64,
        sizes: SmallMapMut<(Scale, u32), Rc<CursorImageScaled>, 2>,
    ) -> Result<Self, CursorError> {
        Ok(Self {
            delay_ns: delay_ms.max(1) * 1_000_000,
            sizes,
        })
    }

    fn for_size(&self, size: u32) -> InstantiatedCursorImage {
        let mut sizes = SmallMapMut::new();
        for ((scale, isize), v) in &self.sizes {
            if *isize == size {
                sizes.insert(*scale, v.clone());
            }
        }
        InstantiatedCursorImage {
            delay_ns: self.delay_ns,
            scales: sizes,
        }
    }
}

struct StaticCursor {
    image: InstantiatedCursorImage,
}

fn render_img(image: &InstantiatedCursorImage, renderer: &mut Renderer, x: Fixed, y: Fixed) {
    let scale = renderer.scale();
    let img = match image.scales.get(&scale) {
        Some(img) => img,
        _ => return,
    };
    let extents = if scale != 1 {
        let scalef = scale.to_f64();
        let x = (x.to_f64() * scalef).round() as i32;
        let y = (y.to_f64() * scalef).round() as i32;
        img.extents.move_(x, y)
    } else {
        img.extents.move_(x.round_down(), y.round_down())
    };
    if extents.intersects(&renderer.pixel_extents()) {
        renderer.base.render_texture(
            &img.tex,
            None,
            extents.x1(),
            extents.y1(),
            None,
            None,
            scale,
            None,
            None,
            AcquireSync::None,
            ReleaseSync::None,
        );
    }
}

impl Cursor for StaticCursor {
    fn render(&self, renderer: &mut Renderer, x: Fixed, y: Fixed) {
        render_img(&self.image, renderer, x, y);
    }

    fn render_hardware_cursor(&self, renderer: &mut Renderer) {
        if let Some(img) = self.image.scales.get(&renderer.scale()) {
            renderer.base.render_texture(
                &img.tex,
                None,
                0,
                0,
                None,
                None,
                renderer.scale(),
                None,
                None,
                AcquireSync::None,
                ReleaseSync::None,
            );
        }
    }

    fn extents_at_scale(&self, scale: Scale) -> Rect {
        match self.image.scales.get(&scale) {
            None => Rect::new_empty(0, 0),
            Some(i) => i.extents,
        }
    }
}

struct AnimatedCursor {
    start: Time,
    eng: Rc<AsyncEngine>,
    next: NumCell<u64>,
    idx: Cell<usize>,
    images: Vec<InstantiatedCursorImage>,
}

impl Cursor for AnimatedCursor {
    fn render(&self, renderer: &mut Renderer, x: Fixed, y: Fixed) {
        let img = &self.images[self.idx.get()];
        render_img(img, renderer, x, y);
    }

    fn render_hardware_cursor(&self, renderer: &mut Renderer) {
        let img = &self.images[self.idx.get()];
        if let Some(img) = img.scales.get(&renderer.scale()) {
            renderer.base.render_texture(
                &img.tex,
                None,
                0,
                0,
                None,
                None,
                renderer.scale(),
                None,
                None,
                AcquireSync::None,
                ReleaseSync::None,
            );
        }
    }

    fn extents_at_scale(&self, scale: Scale) -> Rect {
        let img = &self.images[self.idx.get()];
        match img.scales.get(&scale) {
            None => Rect::new_empty(0, 0),
            Some(i) => i.extents,
        }
    }

    fn tick(&self) {
        let dist = self.eng.now() - self.start;
        if (dist.as_nanos() as u64) < self.next.get() {
            return;
        }
        let idx = (self.idx.get() + 1) % self.images.len();
        self.idx.set(idx);
        let image = &self.images[idx];
        self.next.fetch_add(image.delay_ns);
    }

    fn needs_tick(&self) -> bool {
        true
    }

    fn time_until_tick(&self) -> Duration {
        let dist = self.eng.now() - self.start;
        let dist = dist.as_nanos() as u64;
        let nanos = self.next.get().saturating_sub(dist);
        Duration::from_nanos(nanos)
    }
}

struct OpenCursorResult {
    images: Vec<AHashMap<(Scale, u32), Rc<XCursorImage>>>,
}

fn open_cursor(
    names: &[&str],
    theme: Option<&BStr>,
    scales: &[Scale],
    sizes: &[u32],
    paths: &[BString],
) -> Result<OpenCursorResult, CursorError> {
    let mut file = None;
    let mut pairs_tested = AHashSet::new();
    if let Some(theme) = theme {
        for name in names {
            let name = name.as_bytes().as_bstr();
            file = open_cursor_file(&mut pairs_tested, paths, theme, name);
            if file.is_some() {
                break;
            }
        }
    }
    if file.is_none() {
        for name in names {
            let name = name.as_bytes().as_bstr();
            file = open_cursor_file(&mut pairs_tested, paths, b"default".as_bstr(), name);
            if file.is_some() {
                break;
            }
        }
    }
    let file = match file {
        Some(f) => f,
        _ => return Err(CursorError::NotFound),
    };
    let mut file = BufReader::new(file);
    parser_cursor_file(&mut file, scales, sizes)
}

fn open_cursor_file<'a>(
    pairs_tested: &mut AHashSet<(BString, &'a BStr)>,
    paths: &[BString],
    theme: &BStr,
    name: &'a BStr,
) -> Option<File> {
    if !pairs_tested.insert((theme.to_owned(), name)) {
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
            if let Some(file) = open_cursor_file(pairs_tested, paths, parent.as_bstr(), name) {
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
    ImportError(#[from] GfxError),
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
    scales: &[Scale],
    sizes: &[u32],
) -> Result<OpenCursorResult, CursorError> {
    let [magic, header] = read_u32_n(r)?;
    if magic != XCURSOR_MAGIC || header < HEADER_SIZE {
        return Err(CursorError::NotAnXcursorFile);
    }
    let [_version, ntoc] = read_u32_n(r)?;
    r.seek(SeekFrom::Current((HEADER_SIZE - header) as i64))?;
    if ntoc > 0x10000 {
        return Err(CursorError::OversizedXcursorFile);
    }
    struct Target {
        positions: Vec<u32>,
        effective_size: u32,
        size: u32,
        scale: Scale,
        best_fit: i64,
        best_fit_size: u32,
    }
    let mut targets = Vec::new();
    for scale in scales {
        let scalef = scale.to_f64();
        for size in sizes {
            let effective_size = (*size as f64 * scalef).round() as _;
            targets.push(Target {
                positions: vec![],
                effective_size,
                size: *size,
                scale: *scale,
                best_fit: i64::MAX,
                best_fit_size: 0,
            });
        }
    }
    let mut sizes = AHashSet::new();
    for _ in 0..ntoc {
        let [type_, size, position] = read_u32_n(r)?;
        if type_ != XCURSOR_IMAGE_TYPE {
            continue;
        }
        sizes.insert(size);
        for target in &mut targets {
            let fit = (size as i64 - target.effective_size as i64).abs();
            if fit < target.best_fit {
                target.best_fit = fit;
                target.best_fit_size = size;
                target.positions.clear();
            }
            if size == target.best_fit_size {
                target.positions.push(position);
            }
        }
    }
    let positions: AHashSet<_> = targets
        .iter()
        .flat_map(|t| t.positions.iter().copied())
        .collect();
    if positions.is_empty() {
        return Err(CursorError::EmptyXcursorFile);
    }
    let mut images = AHashMap::new();
    for position in positions {
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
            image.pixels.reserve_exact(num_bytes);
            image.pixels.set_len(num_bytes);
            r.read_exact(slice::from_raw_parts_mut(
                image.pixels.as_mut_ptr() as _,
                num_bytes,
            ))?;
        }
        images.insert(position, Rc::new(image));
    }
    let mut num = targets[0].positions.len();
    if num > 1 && targets.iter().any(|t| t.positions.len() != num) {
        log::warn!("Cursor file contains animated cursor but not all scales have the same number of images");
        num = 1;
    }
    let mut res = vec![];
    for i in 0..num {
        let mut idx_images = AHashMap::new();
        for target in &targets {
            let image = images.get(&target.positions[i]).unwrap();
            idx_images.insert((target.scale, target.size), image.clone());
        }
        res.push(idx_images);
    }
    Ok(OpenCursorResult { images: res })
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
