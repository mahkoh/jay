use {
    crate::it::{
        test_error::{TestErrorExt, TestResult},
        testrun::TestRun,
    },
    std::{
        io::{Read, Write},
        rc::Rc,
    },
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client1 = run.create_client().await?;
    let seat1 = client1.get_default_seat().await?;
    let dev1 = client1.data_device_manager.get_data_device(&seat1.seat)?;
    let entered = seat1.kb.enter.expect()?;
    let win1 = client1.create_window().await?;
    win1.map2().await?;
    let serial = entered.next()?.serial;
    let source1 = client1.data_device_manager.create_data_source()?;
    source1.offer("image")?;
    let sends1 = source1.sends.expect()?;

    let client2 = run.create_client().await?;
    let seat2 = client2.get_default_seat().await?;
    let data_control2 = client2.registry.get_data_control_manager().await?;
    let dev2 = data_control2.get_data_device(&seat2.seat)?;
    let source2 = data_control2.create_data_source()?;
    source2.offer("text")?;
    let sends2 = source2.sends.expect()?;

    let client3 = run.create_client().await?;
    let seat3 = client3.get_default_seat().await?;
    let data_control3 = client3.registry.get_data_control_manager().await?;
    let dev3 = data_control3.get_data_device(&seat3.seat)?;
    let selection = dev3.selection.expect()?;

    dev2.set_selection(&source2)?;
    client2.sync().await;
    client3.sync().await;

    let Some(sel) = selection.last().with_context(|| "selection 1")? else {
        bail!("no selection (1)");
    };
    tassert!(sel.offers.borrow().contains("text"));
    {
        let rfd = sel.receive("text")?;
        client3.sync().await;
        client2.sync().await;
        let (mime, sfd) = sends2.next().with_context(|| "sends2")?;
        tassert_eq!(mime, "text");
        sfd.borrow().write_all(b"abcd")?;
        drop(sfd);
        let mut buf = vec![];
        rfd.borrow().read_to_end(&mut buf)?;
        tassert_eq!(buf, b"abcd");
    }

    tassert_eq!(source2.cancelled.get(), false);
    dev1.set_selection(&source1, serial)?;
    client1.sync().await;
    client2.sync().await;
    tassert_eq!(source2.cancelled.get(), true);

    let Some(sel) = selection.last().with_context(|| "selection 2")? else {
        bail!("no selection (2)");
    };
    tassert!(sel.offers.borrow().contains("image"));
    {
        let rfd = sel.receive("image")?;
        client3.sync().await;
        client1.sync().await;
        let (mime, sfd) = sends1.next().with_context(|| "sends1")?;
        tassert_eq!(mime, "image");
        sfd.borrow().write_all(b"xyz")?;
        drop(sfd);
        let mut buf = vec![];
        rfd.borrow().read_to_end(&mut buf)?;
        tassert_eq!(buf, b"xyz");
    }

    Ok(())
}
