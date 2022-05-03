use {
    crate::{
        it::{
            test_error::TestError,
            test_object::{Deleted, TestObject},
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{jay_screenshot::*, JayScreenshotId},
    },
    std::cell::Cell,
};

pub struct TestJayScreenshot {
    pub id: JayScreenshotId,
    pub result: Cell<Option<Result<Dmabuf, String>>>,
    pub deleted: Deleted,
}

impl TestJayScreenshot {
    fn handle_dmabuf(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Dmabuf::parse_full(parser)?;
        self.result.set(Some(Ok(ev)));
        Ok(())
    }

    fn handle_error(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Error::parse_full(parser)?;
        self.result.set(Some(Err(ev.msg.to_string())));
        Ok(())
    }
}

test_object! {
    TestJayScreenshot, JayScreenshot;

    DMABUF => handle_dmabuf,
    ERROR => handle_error,
}

impl TestObject for TestJayScreenshot {}
