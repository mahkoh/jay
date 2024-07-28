use {
    crate::{
        cli::{GlobalArgs, ScreenshotArgs, ScreenshotFormat},
        format::XRGB8888,
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::{errorfmt::ErrorFmt, queue::AsyncQueue, windows::WindowsExt},
        video::{
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
            drm::Drm,
            gbm::GbmDevice,
        },
        wire::{
            jay_compositor::TakeScreenshot,
            jay_screenshot::{Dmabuf, Error, Format, Plane},
        },
    },
    chrono::Local,
    jay_algorithms::qoi::xrgb8888_encode_qoi,
    png::{BitDepth, ColorType, Encoder, SrgbRenderingIntent},
    std::{cell::RefCell, mem, rc::Rc},
    uapi::OwnedFd,
};

pub fn main(global: GlobalArgs, args: ScreenshotArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
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
    let result = Rc::new(AsyncQueue::new());
    Error::handle(tc, sid, result.clone(), |res, err| {
        res.push(Err(err.msg.to_owned()));
    });
    Dmabuf::handle(tc, sid, result.clone(), |res, buf| {
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: buf.offset,
            stride: buf.stride,
            fd: buf.fd,
        });
        let dmabuf = DmaBuf {
            id: DmaBufIds::default().next(),
            width: buf.width as _,
            height: buf.height as _,
            format: XRGB8888,
            modifier: buf.modifier_lo as u64 | ((buf.modifier_hi as u64) << 32),
            planes,
        };
        res.push(Ok(ScreenshotWithDevice {
            dev: buf.drm_dev,
            buf: dmabuf,
        }));
    });
    let planes = Rc::new(RefCell::new(PlaneVec::new()));
    Plane::handle(tc, sid, planes.clone(), |planes, buf| {
        planes.borrow_mut().push(DmaBufPlane {
            offset: buf.offset,
            stride: buf.stride,
            fd: buf.fd,
        });
    });
    Format::handle(
        tc,
        sid,
        (planes.clone(), result.clone()),
        |(planes, res), buf| {
            let dmabuf = DmaBuf {
                id: DmaBufIds::default().next(),
                width: buf.width as _,
                height: buf.height as _,
                format: XRGB8888,
                modifier: buf.modifier_lo as u64 | ((buf.modifier_hi as u64) << 32),
                planes: mem::take(&mut *planes.borrow_mut()),
            };
            res.push(Ok(ScreenshotWithDevice {
                dev: buf.drm_dev,
                buf: dmabuf,
            }));
        },
    );
    let shot = match result.pop().await {
        Ok(b) => b,
        Err(e) => {
            fatal!("Could not take a screenshot: {}", e);
        }
    };
    let format = screenshot.args.format;
    let data = buf_to_bytes(&shot.dev, &shot.buf, format);
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

pub struct ScreenshotWithDevice {
    pub dev: Rc<OwnedFd>,
    pub buf: DmaBuf,
}

pub fn buf_to_bytes(dev: &Rc<OwnedFd>, buf: &DmaBuf, format: ScreenshotFormat) -> Vec<u8> {
    let drm = match Drm::reopen(dev.raw(), false) {
        Ok(drm) => drm,
        Err(e) => {
            fatal!("Could not open the drm device: {}", ErrorFmt(e));
        }
    };
    let gbm = match GbmDevice::new(&drm) {
        Ok(g) => g,
        Err(e) => {
            fatal!("Could not create a gbm device: {}", ErrorFmt(e));
        }
    };
    let bo = match gbm.import_dmabuf(&buf, 0) {
        Ok(bo) => Rc::new(bo),
        Err(e) => {
            fatal!("Could not import screenshot dmabuf: {}", ErrorFmt(e));
        }
    };
    let bo_map = match bo.map_read() {
        Ok(map) => map,
        Err(e) => {
            fatal!("Could not map dmabuf: {}", ErrorFmt(e));
        }
    };
    let data = unsafe { bo_map.data() };
    if format == ScreenshotFormat::Qoi {
        return xrgb8888_encode_qoi(
            data,
            buf.width as _,
            buf.height as _,
            bo_map.stride() as u32,
        );
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
        encoder.set_srgb(SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&image_data).unwrap();
    }
    out
}
