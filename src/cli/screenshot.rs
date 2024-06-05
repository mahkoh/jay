use {
    crate::{
        cli::{GlobalArgs, ScreenshotArgs, ScreenshotFormat},
        format::XRGB8888,
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::{errorfmt::ErrorFmt, queue::AsyncQueue, windows::WindowsExt},
        video::{
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
            drm::Drm,
            gbm::{GbmDevice, GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING},
        },
        wire::{
            jay_compositor::TakeScreenshot,
            jay_screenshot::{Dmabuf, Error},
        },
    },
    chrono::Local,
    jay_algorithms::qoi::xrgb8888_encode_qoi,
    png::{BitDepth, ColorType, Encoder, SrgbRenderingIntent},
    std::rc::Rc,
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
        res.push(Ok(buf));
    });
    let buf = match result.pop().await {
        Ok(b) => b,
        Err(e) => {
            fatal!("Could not take a screenshot: {}", e);
        }
    };
    let format = screenshot.args.format;
    let data = buf_to_bytes(&DmaBufIds::default(), &buf, format);
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

pub fn buf_to_bytes(dma_buf_ids: &DmaBufIds, buf: &Dmabuf, format: ScreenshotFormat) -> Vec<u8> {
    let drm = match Drm::reopen(buf.drm_dev.raw(), false) {
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
    let mut planes = PlaneVec::new();
    planes.push(DmaBufPlane {
        offset: buf.offset,
        stride: buf.stride,
        fd: buf.fd.clone(),
    });
    let dmabuf = DmaBuf {
        id: dma_buf_ids.next(),
        width: buf.width as _,
        height: buf.height as _,
        format: XRGB8888,
        modifier: (buf.modifier_hi as u64) << 32 | (buf.modifier_lo as u64),
        planes,
    };
    let bo = match gbm.import_dmabuf(&dmabuf, GBM_BO_USE_LINEAR | GBM_BO_USE_RENDERING) {
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
        return xrgb8888_encode_qoi(data, buf.width, buf.height, bo_map.stride() as u32);
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
        let mut encoder = Encoder::new(&mut out, buf.width, buf.height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_srgb(SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&image_data).unwrap();
    }
    out
}
