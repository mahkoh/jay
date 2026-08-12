use crate::async_engine::AsyncEngine;
use crate::io_uring::IoUring;
use crate::utils::client_trace::ClientTraceArgVal;
use crate::utils::client_trace::ClientTraceRead;
use crate::utils::client_trace::ClientTraceWrite;
use crate::utils::client_trace::generated::ClientTraceArray;
use crate::utils::client_trace::generated::ClientTracePod;
use crate::utils::str_fmt::StrCtx;
use crate::wire::ObjectId;

fn pair() -> (ClientTraceWrite, ClientTraceRead) {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let writer = ClientTraceWrite::new(&ring).unwrap();
    let reader = unsafe { ClientTraceRead::new(&ring, writer.memfd(), false).unwrap() };
    (writer, reader)
}

fn attach(buffer: crate::wire::WlBufferId, x: i32, y: i32) -> crate::wire::wl_surface::Attach {
    crate::wire::wl_surface::Attach {
        self_id: crate::wire::WlSurfaceId::from_raw(7),
        buffer,
        x,
        y,
    }
}

#[test]
fn fixed_size_roundtrip() {
    let (writer, mut reader) = pair();
    let msg = attach(crate::wire::WlBufferId::from_raw(9), -3, 4);
    writer.write_msg(ObjectId::from_raw(7), 3_723_456_789, &msg);
    let msg = reader.try_read().unwrap();
    assert_eq!(msg.us, 3_723_456_789);
    assert_eq!(msg.obj, 1); // raw id 7 is mapped to dense id 1
    assert_eq!(msg.args.len(), 3);
    match msg.args[0].val {
        ClientTraceArgVal::Id(id) => assert_eq!(id, 2), // raw id 9 is mapped to dense id 2
        _ => panic!("unexpected arg"),
    }
    match msg.args[1].val {
        ClientTraceArgVal::I32(x) => assert_eq!(x, -3),
        _ => panic!("unexpected arg"),
    }
    match msg.args[2].val {
        ClientTraceArgVal::I32(y) => assert_eq!(y, 4),
        _ => panic!("unexpected arg"),
    }
    let mut s = String::new();
    msg.fmt_text(&mut s, &StrCtx::default(), false, 1000);
    assert_eq!(
        s,
        "[01:02:03.456789] {1000} -> wl_surface#1.attach(buffer: wl_buffer#2, x: -3, y: 4)\n"
    );
    let mut j = String::new();
    msg.fmt_jsonl(&mut j, &StrCtx::default(), 1000);
    assert_eq!(
        j,
        concat!(
            r#"{"t":"m","cl":1000,"us":3723456789,"inf":"wl_surface","id":1,"#,
            r#""msg":"attach","args":{"buffer":2,"x":-3,"y":4}}"#,
            "\n"
        )
    );
}

#[test]
fn nil_id_is_preserved() {
    let (writer, mut reader) = pair();
    let msg = attach(crate::wire::WlBufferId::NONE, 0, 0);
    writer.write_msg(ObjectId::from_raw(7), 100, &msg);
    let msg = reader.try_read().unwrap();
    match msg.args[0].val {
        ClientTraceArgVal::Id(id) => assert_eq!(id, 0),
        _ => panic!("unexpected arg"),
    }
    let mut s = String::new();
    msg.fmt_text(&mut s, &StrCtx::default(), false, 1);
    assert!(s.contains("buffer: wl_buffer#nil"), "{}", s);
}

#[test]
fn variable_size_roundtrip() {
    let (writer, mut reader) = pair();
    let msg = crate::wire::jay_input::SetKeymapFromNames {
        self_id: crate::wire::JayInputId::from_raw(3),
        seat: "seat0",
        rules: Some("evdev"),
        model: None,
        layout: Some("us"),
        variant: Some(""),
        options: None,
    };
    writer.write_msg(ObjectId::from_raw(3), 100, &msg);
    let msg = reader.try_read().unwrap();
    let arg = |idx: usize| match &msg.args[idx].val {
        ClientTraceArgVal::Str(s) => *s,
        _ => panic!("unexpected arg"),
    };
    assert_eq!(arg(0), Some(b"seat0".as_slice()));
    assert_eq!(arg(1), Some(b"evdev".as_slice()));
    assert_eq!(arg(2), None);
    assert_eq!(arg(3), Some(b"us".as_slice()));
    assert_eq!(arg(4), Some(b"".as_slice()));
    assert_eq!(arg(5), None);
    let mut s = String::new();
    msg.fmt_text(&mut s, &StrCtx::default(), false, 1);
    assert_eq!(
        s,
        concat!(
            r#"[00:00:00.000100] {1} -> jay_input#1.set_keymap_from_names("#,
            r#"seat: "seat0", rules: "evdev", model: nil, layout: "us", "#,
            r#"variant: "", options: nil)"#,
            "\n"
        )
    );
    let mut j = String::new();
    msg.fmt_jsonl(&mut j, &StrCtx::default(), 1);
    assert_eq!(
        j,
        concat!(
            r#"{"t":"m","cl":1,"us":100,"inf":"jay_input","id":1,"#,
            r#""msg":"set_keymap_from_names","args":{"seat":"seat0","#,
            r#""rules":"evdev","model":null,"layout":"us","variant":"","#,
            r#""options":null}}"#,
            "\n"
        )
    );
}

#[test]
fn array_roundtrip() {
    let (writer, mut reader) = pair();
    let msg = crate::wire::zwp_linux_dmabuf_feedback_v1::TrancheFormats {
        self_id: crate::wire::ZwpLinuxDmabufFeedbackV1Id::from_raw(5),
        indices: &[1, 2, 3, 65535],
    };
    writer.write_msg(ObjectId::from_raw(5), 100, &msg);
    let msg = reader.try_read().unwrap();
    match msg.args[0].val {
        ClientTraceArgVal::Array(ClientTraceArray::V2(v)) => {
            assert_eq!(v, &[1, 2, 3, 65535]);
        }
        _ => panic!("unexpected arg"),
    }
}

#[test]
fn pod_roundtrip() {
    let (writer, mut reader) = pair();
    let msg = crate::wire::zwp_linux_dmabuf_feedback_v1::MainDevice {
        self_id: crate::wire::ZwpLinuxDmabufFeedbackV1Id::from_raw(5),
        device: 0x1234_5678,
    };
    writer.write_msg(ObjectId::from_raw(5), 100, &msg);
    let msg = reader.try_read().unwrap();
    match msg.args[0].val {
        ClientTraceArgVal::Pod(ClientTracePod::V0(v)) => {
            assert_eq!(v, 0x1234_5678);
        }
        _ => panic!("unexpected arg"),
    }
}

#[test]
fn u64_roundtrip() {
    let (writer, mut reader) = pair();
    let msg = crate::wire::wp_linux_drm_syncobj_surface_v1::SetReleasePoint {
        self_id: crate::wire::WpLinuxDrmSyncobjSurfaceV1Id::from_raw(1),
        timeline: crate::wire::WpLinuxDrmSyncobjTimelineV1Id::from_raw(2),
        point: 0xdead_beef_1234_5678,
    };
    writer.write_msg(ObjectId::from_raw(1), 100, &msg);
    let msg = reader.try_read().unwrap();
    match msg.args[1].val {
        ClientTraceArgVal::U64(v) => assert_eq!(v, 0xdead_beef_1234_5678),
        _ => panic!("unexpected arg"),
    }
}

#[test]
fn fd_arg_has_no_value() {
    let (writer, mut reader) = pair();
    let msg = crate::wire::wl_shm::CreatePool {
        self_id: crate::wire::WlShmId::from_raw(1),
        id: crate::wire::WlShmPoolId::from_raw(2),
        fd: std::rc::Rc::new(uapi::memfd_create("test", 0).unwrap()),
        size: 12,
    };
    writer.write_msg(ObjectId::from_raw(1), 100, &msg);
    let msg = reader.try_read().unwrap();
    assert_eq!(msg.args.len(), 3);
    match msg.args[0].val {
        ClientTraceArgVal::Id(id) => assert_eq!(id, 2),
        _ => panic!("unexpected arg"),
    }
    assert!(matches!(msg.args[1].val, ClientTraceArgVal::Fd));
    match msg.args[2].val {
        ClientTraceArgVal::I32(v) => assert_eq!(v, 12),
        _ => panic!("unexpected arg"),
    }
}

#[test]
fn delete_id_remaps() {
    let (writer, mut reader) = pair();
    let msg = attach(crate::wire::WlBufferId::NONE, 0, 0);
    writer.write_msg(ObjectId::from_raw(7), 100, &msg);
    assert_eq!(reader.try_read().unwrap().obj, 1);
    writer.write_delete_id(ObjectId::from_raw(7));
    // After the id was deleted, the same raw id is mapped to a new dense id.
    let msg = attach(crate::wire::WlBufferId::NONE, 1, 1);
    writer.write_msg(ObjectId::from_raw(7), 101, &msg);
    {
        let msg = reader.try_read().unwrap();
        assert_eq!(msg.obj, 2);
        assert_eq!(msg.us, 101);
    }
    assert!(reader.try_read().is_none());
}

#[test]
fn oversized_message_is_dropped() {
    let (writer, mut reader) = pair();
    let big = "x".repeat(10_000);
    let msg = crate::wire::jay_input::SetKeymapFromNames {
        self_id: crate::wire::JayInputId::from_raw(3),
        seat: &big,
        rules: None,
        model: None,
        layout: None,
        variant: None,
        options: None,
    };
    writer.write_msg(ObjectId::from_raw(3), 100, &msg);
    assert!(reader.try_read().is_none());
}
