use crate::cli::GlobalArgs;
use crate::cli::ScreenshotArgs;
use crate::cli::ScreenshotFormat;
use crate::format::Format;
use crate::format::XRGB8888;
use crate::format::XRGB2101010;
use crate::format::formats;
use crate::ifs::jay_compositor::UDMABUF_SCREENSHOT_SINCE;
use crate::tools::tool_client::Handle;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::queue::AsyncQueue;
use crate::utils::windows::WindowsExt;
use crate::video::LINEAR_MODIFIER;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::wire::jay_compositor::TakeScreenshot3;
use crate::wire::jay_screenshot::Error;
use crate::wire::jay_screenshot::Udmabuf;
use chrono::Local;
use jay_algorithms::mmap::mmap;
use jay_algorithms::oserror::OsError;
use jay_algorithms::qoi::xrgb8888_encode_qoi;
use png::BitDepth;
use png::ColorType;
use png::Encoder;
use png::SrgbRenderingIntent;
use png::chunk;
use std::rc::Rc;
use thiserror::Error;
use uapi::c;

/// cICP chunk contents for HDR10: BT.2020 primaries, ST 2084 (PQ) transfer
/// characteristics, identity matrix coefficients (PNG stores RGB, not YCbCr),
/// full-range samples.
const CICP_HDR10: [u8; 4] = [9, 16, 0, 1];

pub fn main(_global: GlobalArgs, args: ScreenshotArgs) {
    if args.hdr10 && args.format != ScreenshotFormat::Png {
        fatal!("--hdr10 is only supported with --format=png");
    }
    with_tool_client(async move |tc| {
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
    let version = tc.jay_compositor_version().await;
    if version < UDMABUF_SCREENSHOT_SINCE {
        fatal!("The compositor version is no longer supported by this CLI");
    } else {
        tc.send(TakeScreenshot3 {
            self_id: comp,
            id: sid,
            include_cursor: false,
            hdr10: screenshot.args.hdr10,
        });
    }
    let result = Rc::new(AsyncQueue::default());
    Error::handle(tc, sid, result.clone(), |res, err| {
        res.push(Err(err.msg.to_owned()));
    });
    Udmabuf::handle(tc, sid, result.clone(), |res, ev| {
        let Some(format) = formats().get(&ev.format).copied() else {
            res.push(Err(format!(
                "Compositor sent an unknown format {}",
                ev.format
            )));
            return;
        };
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: 0,
            stride: ev.stride,
            fd: ev.fd,
        });
        let buf = DmaBuf::new(
            &DmaBufIds::default(),
            ev.width,
            ev.height,
            format,
            LINEAR_MODIFIER,
            planes,
        );
        res.push(Ok(buf));
    });
    let buf = match result.pop().await {
        Ok(b) => b,
        Err(e) => {
            fatal!("Could not take a screenshot: {}", e);
        }
    };
    let format = screenshot.args.format;
    let data = match buf_to_bytes(&buf, format, screenshot.args.hdr10) {
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
    #[error("Could not map a dmabuf")]
    MapDmabuf(#[source] OsError),
    #[error("Don't know how to encode format `{}` as a PNG", .0.name)]
    UnknownFormat(&'static Format),
}

pub fn buf_to_bytes(
    buf: &Rc<DmaBuf>,
    format: ScreenshotFormat,
    hdr10: bool,
) -> Result<Vec<u8>, ScreenshotError> {
    let plane = &buf.planes[0];
    let map = mmap(
        buf.height as usize * plane.stride as usize,
        c::PROT_READ,
        c::MAP_SHARED,
        plane.fd.raw(),
        0,
    )
    .map_err(ScreenshotError::MapDmabuf)?;
    let data = unsafe { &*map.ptr };
    if format == ScreenshotFormat::Qoi {
        return Ok(xrgb8888_encode_qoi(
            data,
            buf.width as _,
            buf.height as _,
            plane.stride,
        ));
    }

    // Decoding the raw buffer into 8- or 16-bit-per-channel RGBA is purely a function
    // of the pixel format we got back from the compositor. Whether we annotate the
    // resulting PNG as HDR10 (via a cICP chunk) is a separate question, controlled by
    // whether we requested HDR10 from the compositor in the first place.
    let (bit_depth, image_data) = if buf.format == XRGB2101010 {
        // Unpack 10-bit-per-channel x:R:G:B samples (as used by XRGB2101010 /
        // VK_FORMAT_A2R10G10B10_UNORM_PACK32) into 16-bit-per-channel RGBA, expanding
        // each 10-bit value to 16 bits by replicating its high bits into the low bits
        // so that 0 maps to 0 and 1023 maps to 65535.
        let mut image_data = Vec::with_capacity((buf.width * buf.height * 8) as usize);
        let lines = data[..(buf.height as usize * plane.stride as usize)]
            .chunks_exact(plane.stride as usize);
        for line in lines {
            for pixel in line[..(buf.width as usize * 4)].array_chunks_ext::<4>() {
                let word = u32::from_le_bytes(*pixel);
                let r10 = (word >> 20) & 0x3ff;
                let g10 = (word >> 10) & 0x3ff;
                let b10 = word & 0x3ff;
                let expand = |v: u32| -> [u8; 2] { (((v << 6) | (v >> 4)) as u16).to_be_bytes() };
                image_data.extend_from_slice(&expand(r10));
                image_data.extend_from_slice(&expand(g10));
                image_data.extend_from_slice(&expand(b10));
                image_data.extend_from_slice(&0xffffu16.to_be_bytes());
            }
        }
        (BitDepth::Sixteen, image_data)
    } else if buf.format == XRGB8888 {
        let mut image_data = Vec::with_capacity((buf.width * buf.height * 4) as usize);
        let lines = data[..(buf.height as usize * plane.stride as usize)]
            .chunks_exact(plane.stride as usize);
        for line in lines {
            for pixel in line[..(buf.width as usize * 4)].array_chunks_ext::<4>() {
                image_data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            }
        }
        (BitDepth::Eight, image_data)
    } else {
        return Err(ScreenshotError::UnknownFormat(buf.format));
    };

    let mut out = vec![];
    {
        let mut encoder = Encoder::new(&mut out, buf.width as _, buf.height as _);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(bit_depth);
        if !hdr10 {
            encoder.set_source_srgb(SrgbRenderingIntent::Perceptual);
        }
        let mut writer = encoder.write_header().unwrap();
        if hdr10 {
            // BT.2020 primaries, ST 2084 (PQ) transfer characteristics.
            writer.write_chunk(chunk::cICP, &CICP_HDR10).unwrap();
        }
        writer.write_image_data(&image_data).unwrap();
    }
    Ok(out)
}
