use {
    crate::{
        it::{
            test_client::{DefaultSeat, TestClient},
            test_error::TestResult,
            test_ifs::{
                test_input_method::TestInputMethod,
                test_input_popup_surface::TestInputPopupSurface, test_text_input::TestTextInput,
            },
            test_utils::{
                test_expected_event::TestExpectedEvent, test_surface_ext::TestSurfaceExt,
                test_window::TestWindow,
            },
            testrun::TestRun,
        },
        wire::zwp_text_input_v3,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let consumer = create_consumer(&run).await?;
    let supplier = create_supplier(&run).await?;

    consumer.client.compare_screenshot("1", false).await?;

    supplier.client.sync().await;
    tassert!(supplier.activate.next().is_err());

    consumer.text.enable()?;
    consumer.text.set_cursor_rectangle(100, 100, 100, 100)?;
    consumer.text.commit()?;
    consumer.client.sync().await;

    supplier.client.sync().await;
    tassert!(matches!(supplier.activate.next(), Ok(true)));
    tassert!(supplier.done.next().is_ok());

    consumer.client.compare_screenshot("1", false).await?;

    supplier.surface.commit()?;
    supplier.client.sync().await;

    consumer.client.compare_screenshot("2", false).await?;

    supplier.im.commit_string("hello world")?;
    supplier.im.commit()?;
    supplier.client.sync().await;

    consumer.client.sync().await;
    tassert_eq!(
        consumer.commit_string.next().expect("commit string"),
        "hello world"
    );
    tassert!(consumer.done.next().is_ok());

    consumer.text.disable()?;
    consumer.text.commit()?;
    consumer.client.sync().await;

    consumer.client.compare_screenshot("3", false).await?;

    Ok(())
}

struct Consumer {
    client: Rc<TestClient>,
    _seat: DefaultSeat,
    _window: Rc<TestWindow>,
    text: Rc<TestTextInput>,
    _enter: TestExpectedEvent<zwp_text_input_v3::Enter>,
    _leave: TestExpectedEvent<zwp_text_input_v3::Leave>,
    commit_string: TestExpectedEvent<String>,
    done: TestExpectedEvent<zwp_text_input_v3::Done>,
}

async fn create_consumer(run: &Rc<TestRun>) -> TestResult<Consumer> {
    let client = run.create_client().await?;
    let seat = client.get_default_seat().await?;
    let text = client
        .registry
        .get_text_input_manager()
        .await?
        .get_text_input(&seat.seat)?;
    let window = client.create_window().await?;
    window.map2().await?;
    client.sync().await;
    Ok(Consumer {
        _enter: text.enter.expect()?,
        _leave: text.leave.expect()?,
        commit_string: text.commit_string.expect()?,
        done: text.done.expect()?,
        client,
        _seat: seat,
        _window: window,
        text,
    })
}

struct Supplier {
    client: Rc<TestClient>,
    _seat: DefaultSeat,
    im: Rc<TestInputMethod>,
    surface: TestSurfaceExt,
    _popup: Rc<TestInputPopupSurface>,
    activate: TestExpectedEvent<bool>,
    done: TestExpectedEvent<()>,
}

async fn create_supplier(run: &Rc<TestRun>) -> TestResult<Supplier> {
    let client = run.create_client().await?;
    let seat = client.get_default_seat().await?;
    let im = client
        .registry
        .get_input_method_manager()
        .await?
        .get_input_method(&seat.seat)?;
    let surface = client.create_surface_ext().await?;
    surface.set_color(255, 0, 0, 255);
    surface.map(100, 100).await?;
    let popup = im.get_popup(&surface)?;
    client.sync().await;
    Ok(Supplier {
        activate: im.activate.expect()?,
        done: im.done.expect()?,
        client,
        _seat: seat,
        im,
        surface,
        _popup: popup,
    })
}
