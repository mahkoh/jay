use {
    crate::{
        cli::{GlobalArgs, ScreenshotArgs},
        format::XRGB8888,
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::{errorfmt::ErrorFmt, queue::AsyncQueue},
        video::{
            dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
            drm::Drm,
            gbm::{GbmDevice, GBM_BO_USE_LINEAR, GBM_BO_USE_RENDERING},
        },
        wire::{
            jay_compositor::TakeScreenshot,
            jay_screenshot::{Dmabuf, Error},
        },
    },
    algorithms::qoi::xrgb8888_encode_qoi,
    chrono::Local,
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
    let data = buf_to_qoi(&buf);
    let filename = screenshot
        .args
        .filename
        .as_deref()
        .unwrap_or("%Y-%m-%d-%H%M%S_jay.qoi");
    let filename = Local::now().format(filename).to_string();
    if let Err(e) = std::fs::write(&filename, &data) {
        fatal!("Could not write `{}`: {}", filename, ErrorFmt(e));
    }
}

pub fn buf_to_qoi(buf: &Dmabuf) -> Vec<u8> {
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
    let bo_map = match bo.map() {
        Ok(map) => map,
        Err(e) => {
            fatal!("Could not map dmabuf: {}", ErrorFmt(e));
        }
    };
    let data = unsafe { bo_map.data() };
    xrgb8888_encode_qoi(data, buf.width, buf.height, buf.stride)
}
