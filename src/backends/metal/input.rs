use crate::async_engine::FdStatus;
use crate::libinput::consts::LIBINPUT_EVENT_KEYBOARD_KEY;
use crate::libinput::event::LibInputEvent;
use crate::metal::MetalBackend;
use crate::ErrorFmt;
use std::rc::Rc;

impl MetalBackend {
    pub async fn handle_libinput_events(self: Rc<Self>) {
        loop {
            match self.libinput_fd.readable().await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for libinput fd to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(FdStatus::Err) => {
                    log::error!("libinput fd fd is in an error state");
                    break;
                }
                _ => {}
            }
            if let Err(e) = self.libinput.dispatch() {
                log::error!("Could not dispatch libinput events: {}", ErrorFmt(e));
                break;
            }
            while let Some(event) = self.libinput.event() {
                self.handle_event(event);
            }
        }
        log::error!("Libinput task exited. Future input events will be ignored.");
    }

    fn handle_event(self: &Rc<Self>, event: LibInputEvent) {
        match event.ty() {
            LIBINPUT_EVENT_KEYBOARD_KEY => self.handle_keyboard_event(event),
            _ => {}
        }
    }

    fn handle_keyboard_event(self: &Rc<Self>, event: LibInputEvent) {
        let event = match event.keyboard_event() {
            Some(event) => event,
            _ => return,
        };
        log::info!(
            "key: {}, state: {:?}, time: {}",
            event.key(),
            event.key_state(),
            event.time_usec()
        );
    }
}
