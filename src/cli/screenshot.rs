use crate::allocator::Allocator;
use crate::allocator::AllocatorError;
use crate::allocator::BufferUsage;
use crate::allocator::MappedBuffer;
use crate::cli::GlobalArgs;
use crate::cli::ScreenshotArgs;
use crate::cli::ScreenshotFormat;
use crate::eventfd_cache::EventfdCache;
use crate::format::XRGB8888;
use crate::gfx_apis;
use crate::tools::tool_client::Handle;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::udmabuf::Udmabuf;
use crate::udmabuf::UdmabufError;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::queue::AsyncQueue;
use crate::utils::windows::WindowsExt;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::video::drm::Drm;
use crate::video::drm::DrmError;
use crate::video::gbm::GbmDevice;
use crate::video::gbm::GbmError;
use crate::wire::jay_compositor::TakeScreenshot;
use crate::wire::jay_screenshot::Dmabuf;
use crate::wire::jay_screenshot::Dmabuf2;
use crate::wire::jay_screenshot::DrmDev;
use crate::wire::jay_screenshot::Error;
use crate::wire::jay_screenshot::Plane;
use chrono::Local;
use jay_algorithms::qoi::xrgb8888_encode_qoi;
use png::BitDepth;
use png::ColorType;
use png::Encoder;
use png::SrgbRenderingIntent;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

pub fn main(_global: GlobalArgs, args: ScreenshotArgs) {
    with_tool_client(|tc| async move {
        let screenshot = Rc::new(Screenshot {
            tc: tc.clone(),
            args,
        });
        run(screenshot).await;
    });
}

struct Screenshot {
    tc: Rc<ToolClient>,
    args: ScreenshotArgs,
}

async fn run(screenshot: Rc<Screenshot>) {
    let tc = &screenshot.tc;
    let comp = tc.jay_compositor().await;
    let sid = tc.id();
    tc.send(TakeScreenshot {
        self_id: comp,
        id: sid,
    });
    let result = Rc::new(AsyncQueue::default());
    Error::handle(tc, sid, result.clone(), |res, err| {
        res.push(Err(err.msg.to_owned()));
    });
    Dmabuf::handle(tc, sid, result.clone(), |res, ev| {
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        let buf = DmaBuf::new(
            &DmaBufIds::default(),
            ev.width as _,
            ev.height as _,
            XRGB8888,
            ev.modifier,
            planes,
        );
        res.push(Ok((buf, Some(ev.drm_dev))));
    });
    let drm_dev = Rc::new(Cell::new(None));
    let planes = Rc::new(RefCell::new(PlaneVec::new()));
    DrmDev::handle(tc, sid, drm_dev.clone(), |res, buf| {
        res.set(Some(buf.drm_dev));
    });
    Plane::handle(tc, sid, planes.clone(), |res, buf| {
        res.borrow_mut().push(DmaBufPlane {
            offset: buf.offset,
            stride: buf.stride,
            fd: buf.fd,
        });
    });
    Dmabuf2::handle(
        tc,
        sid,
        (drm_dev, planes, result.clone()),
        |(dev, planes, res), ev| {
            let buf = DmaBuf::new(
                &DmaBufIds::default(),
                ev.width,
                ev.height,
                XRGB8888,
                ev.modifier,
                planes.take(),
            );
            res.push(Ok((buf, dev.take())))
        },
    );
    let (buf, drm_dev) = match result.pop().await {
        Ok(b) => b,
        Err(e) => {
            fatal!("Could not take a screenshot: {}", e);
        }
    };
    let format = screenshot.args.format;
    let eventfd_cache = EventfdCache::new(&tc.ring, &tc.eng);
    let data = match buf_to_bytes(&eventfd_cache, drm_dev.as_ref(), &buf, format) {
        Ok(d) => d,
        Err(e) => fatal!("{}", ErrorFmt(e)),
    };
    let filename = match &screenshot.args.filename {
        Some(f) => f.clone(),
        _ => {
            let ext = match format {
                ScreenshotFormat::Png => "png",
                ScreenshotFormat::Qoi => "qoi",
            };
            format!("%Y-%m-%d-%H%M%S_jay.{ext}")
        }
    };
    let filename = Local::now().format(&filename).to_string();
    if let Err(e) = std::fs::write(&filename, data) {
        fatal!("Could not write `{}`: {}", filename, ErrorFmt(e));
    }
}

#[derive(Debug, Error)]
pub enum ScreenshotError {
    #[error("Could not open the drm device")]
    OpenDrmDevice(#[source] DrmError),
    #[error("Could not create a gbm device")]
    CreateGbmDevice(#[source] GbmError),
    #[error("Could not create a udmabuf allocator")]
    CreateUdmabuf(#[source] UdmabufError),
    #[error("Could not import a dmabuf")]
    ImportDmabuf(#[source] AllocatorError),
    #[error("Could not map a dmabuf")]
    MapDmabuf(#[source] AllocatorError),
    #[error("Could not create a vulkan allocator")]
    CreateVulkanAllocator(#[source] AllocatorError),
    #[error("Could not map the dmabuf with any allocator")]
    MapDmabufAny,
}

fn map(
    allocator: Rc<dyn Allocator>,
    buf: &Rc<DmaBuf>,
) -> Result<Box<dyn MappedBuffer>, ScreenshotError> {
    let bo = allocator
        .import_dmabuf(buf, BufferUsage::none())
        .map_err(ScreenshotError::ImportDmabuf)?;
    let bo_map = bo.map_read().map_err(ScreenshotError::MapDmabuf)?;
    Ok(bo_map)
}

pub fn buf_to_bytes(
    eventfd_cache: &Rc<EventfdCache>,
    drm_dev: Option<&Rc<OwnedFd>>,
    buf: &Rc<DmaBuf>,
    format: ScreenshotFormat,
) -> Result<Vec<u8>, ScreenshotError> {
    let mut allocators =
        Vec::<Box<dyn FnOnce() -> Result<Rc<dyn Allocator>, ScreenshotError>>>::new();
    match drm_dev {
        Some(drm_dev) => {
            let drm = || Drm::reopen(drm_dev.raw(), false).map_err(ScreenshotError::OpenDrmDevice);
            let gbm = Box::new(move || {
                GbmDevice::new(&drm()?)
                    .map(|d| Rc::new(d) as _)
                    .map_err(ScreenshotError::CreateGbmDevice)
            });
            let vulkan = Box::new(move || {
                gfx_apis::create_vulkan_allocator(&drm()?, eventfd_cache)
                    .map_err(ScreenshotError::CreateVulkanAllocator)
            });
            allocators.push(vulkan);
            allocators.push(gbm);
        }
        None => {
            let udmabuf = Box::new(|| {
                Udmabuf::new()
                    .map(|u| Rc::new(u) as _)
                    .map_err(ScreenshotError::CreateUdmabuf)
            });
            allocators.push(udmabuf);
        }
    }
    let bo_map = 'create_bo_map: {
        for allocator in allocators {
            let allocator = match allocator() {
                Ok(a) => a,
                Err(e) => {
                    log::error!("Could not create allocator: {}", ErrorFmt(e));
                    continue;
                }
            };
            match map(allocator, buf) {
                Ok(m) => break 'create_bo_map m,
                Err(e) => {
                    log::error!("Could not map dmabuf: {}", ErrorFmt(e));
                    continue;
                }
            };
        }
        return Err(ScreenshotError::MapDmabufAny);
    };
    let data = unsafe { bo_map.data() };
    if format == ScreenshotFormat::Qoi {
        return Ok(xrgb8888_encode_qoi(
            data,
            buf.width as _,
            buf.height as _,
            bo_map.stride() as u32,
        ));
    }

    let mut out = vec![];
    {
        let mut image_data = Vec::with_capacity((buf.width * buf.height * 4) as usize);
        let lines = data[..(buf.height as usize * bo_map.stride() as usize)]
            .chunks_exact(bo_map.stride() as usize);
        for line in lines {
            for pixel in line[..(buf.width as usize * 4)].array_chunks_ext::<4>() {
                image_data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255])
            }
        }
        let mut encoder = Encoder::new(&mut out, buf.width as _, buf.height as _);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_source_srgb(SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&image_data).unwrap();
    }
    Ok(out)
}
