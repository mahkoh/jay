use {
    crate::{
        backend::KeyState,
        clientmem::ClientMem,
        it::{test_error::TestResult, testrun::TestRun},
        kbvm::KbvmContext,
    },
    bstr::ByteSlice,
    std::rc::Rc,
    uapi::OwnedFd,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let virtual_keymap_str = {
        let xkb = KbvmContext::default();
        let map = xkb.parse_keymap(VIRTUAL_KEYMAP.as_bytes()).unwrap();
        read_keymap(&map.map.map, map.map.len)
    };

    let ds = run.create_default_setup().await?;

    let s_client = run.create_client().await?;
    let s_seat = s_client.get_default_seat().await?;
    let s_win = s_client.create_window().await?;
    s_win.map2().await?;
    s_client.sync().await;

    let s_keymap = s_seat.kb.keymap.expect()?;
    let s_key = s_seat.kb.key.expect()?;
    let s_modifiers = s_seat.kb.modifiers.expect()?;

    {
        let v_client = run.create_client().await?;
        let v_seat = v_client.get_default_seat().await?;
        let v_kb = v_client
            .registry
            .get_virtual_keyboard_manager()
            .await?
            .create_virtual_keyboard(&v_seat.seat)?;
        v_kb.set_keymap(VIRTUAL_KEYMAP)?;
        v_kb.key(10, KeyState::Pressed)?;
        v_kb.key(10, KeyState::Released)?;
        v_kb.modifiers(1, 2, 3, 0)?;
        v_kb.key(10, KeyState::Pressed)?;
        v_kb.key(10, KeyState::Released)?;
        v_kb.modifiers(0, 0, 0, 1)?;
        v_client.sync().await;
    }

    s_client.sync().await;
    let (start, keymap) = s_keymap.next().expect("virtual keymap");
    tassert_eq!(
        &read_keymap(&keymap.fd, keymap.size as _),
        &virtual_keymap_str
    );
    {
        let (pos, mods) = s_modifiers.next().expect("mods 0");
        tassert_eq!(pos, start + 1);
        tassert_eq!(
            (
                mods.mods_depressed,
                mods.mods_latched,
                mods.mods_locked,
                mods.group
            ),
            (0, 0, 0, 0)
        );
    }
    {
        let (pos, key) = s_key.next().expect("key 1");
        tassert_eq!(pos, start + 2);
        tassert_eq!((key.key, key.state), (10, 1));
    }
    {
        let (pos, key) = s_key.next().expect("key 2");
        tassert_eq!(pos, start + 3);
        tassert_eq!((key.key, key.state), (10, 0));
    }
    {
        let (pos, mods) = s_modifiers.next().expect("mods 1");
        tassert_eq!(pos, start + 4);
        tassert_eq!(
            (
                mods.mods_depressed,
                mods.mods_latched,
                mods.mods_locked,
                mods.group
            ),
            (1, 2, 3, 0)
        );
    }
    {
        let (pos, key) = s_key.next().expect("key 3");
        tassert_eq!(pos, start + 5);
        tassert_eq!((key.key, key.state), (10, 1));
    }
    {
        let (pos, key) = s_key.next().expect("key 4");
        tassert_eq!(pos, start + 6);
        tassert_eq!((key.key, key.state), (10, 0));
    }
    {
        let (pos, mods) = s_modifiers.next().expect("mods 2");
        tassert_eq!(pos, start + 7);
        tassert_eq!(
            (
                mods.mods_depressed,
                mods.mods_latched,
                mods.mods_locked,
                mods.group
            ),
            (0, 0, 0, 1)
        );
    }

    ds.kb.press(10);

    s_client.sync().await;
    let (pos, keymap) = s_keymap.next().expect("seat keymap");
    tassert_eq!(pos, start + 8);
    tassert!(read_keymap(&keymap.fd, keymap.size as _) != virtual_keymap_str);
    {
        let (pos, mods) = s_modifiers.next().expect("mods 0");
        tassert_eq!(pos, start + 9);
        tassert_eq!(
            (
                mods.mods_depressed,
                mods.mods_latched,
                mods.mods_locked,
                mods.group
            ),
            (0, 0, 0, 0)
        );
    }
    {
        let (pos, key) = s_key.next().expect("key 5");
        tassert_eq!(pos, start + 10);
        tassert_eq!((key.key, key.state), (10, 1));
    }
    {
        let (pos, key) = s_key.next().expect("key 6");
        tassert_eq!(pos, start + 11);
        tassert_eq!((key.key, key.state), (10, 0));
    }

    Ok(())
}

fn read_keymap(fd: &Rc<OwnedFd>, size: usize) -> String {
    let client_mem = ClientMem::new_private(fd, size - 1, true, None, None).unwrap();
    let client_mem = Rc::new(client_mem).offset(0);
    let mut v = vec![];
    client_mem.read(&mut v).unwrap();
    v.as_bstr().to_string()
}

const VIRTUAL_KEYMAP: &str = r#"
    xkb_keymap {
        xkb_keycodes {
              <2> =  10; # 1
             <29> =  37; # LEFTCTRL
        };

        xkb_types {
            type "TWO_LEVEL" {
                modifiers  = Control;
                map[Control] = Level2;
                level_name[Level1] = "Base";
                level_name[Level2] = "Control";
            };
        };

        xkb_compatibility {
            interpret.repeat  = False;
            interpret.locking = False;
            interpret Control_L {
                action = SetMods(modifiers=Control);
            };
        };

        xkb_symbols {
            key  <2> { [ 2, at ] };
            key <29> { [ Control_L ] };
        };

    };
"#;
