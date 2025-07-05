use {
    crate::{
        backend::{self},
        client::ClientError,
        ifs::head_management::{HeadCommonError, HeadState},
        wire::{
            jay_head_ext_physical_display_info_v1::{
                JayHeadExtPhysicalDisplayInfoV1RequestHandler, Manufacturer, Mode, Model,
                NonDesktop, PhysicalSize, Reset, SerialNumber, VrrCapable,
            },
            jay_head_manager_ext_physical_display_info_v1::JayHeadManagerExtPhysicalDisplayInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

ext! {
    snake = physical_display_info_v1,
    camel = PhysicalDisplayInfoV1,
    version = 1,
    after_announce = after_announce,
}

impl JayHeadExtPhysicalDisplayInfoV1 {
    fn after_announce(&self, shared: &HeadState) {
        self.send_info(shared);
    }

    pub fn send_info(&self, state: &HeadState) {
        self.send_reset();
        if let Some(mi) = &state.monitor_info {
            for mode in &mi.modes {
                self.send_mode(mode);
            }
            self.send_manufacturer(&mi.output_id.manufacturer);
            self.send_model(&mi.output_id.model);
            self.send_serial_number(&mi.output_id.serial_number);
            self.send_physical_size(mi.width_mm, mi.height_mm);
            if mi.non_desktop {
                self.send_non_desktop();
            }
            if mi.vrr_capable {
                self.send_vrr_capable();
            }
        }
    }

    fn send_reset(&self) {
        self.client.event(Reset { self_id: self.id });
    }

    fn send_mode(&self, mode: &backend::Mode) {
        self.client.event(Mode {
            self_id: self.id,
            width: mode.width,
            height: mode.height,
            refresh_mhz: mode.refresh_rate_millihz,
        });
    }

    fn send_physical_size(&self, width_mm: i32, height_mm: i32) {
        self.client.event(PhysicalSize {
            self_id: self.id,
            width_mm,
            height_mm,
        });
    }

    fn send_manufacturer(&self, manufacturer: &str) {
        self.client.event(Manufacturer {
            self_id: self.id,
            manufacturer,
        });
    }

    fn send_model(&self, model: &str) {
        self.client.event(Model {
            self_id: self.id,
            model,
        });
    }

    fn send_serial_number(&self, serial_number: &str) {
        self.client.event(SerialNumber {
            self_id: self.id,
            serial_number,
        });
    }

    fn send_non_desktop(&self) {
        self.client.event(NonDesktop { self_id: self.id });
    }

    fn send_vrr_capable(&self) {
        self.client.event(VrrCapable { self_id: self.id });
    }
}

impl JayHeadManagerExtPhysicalDisplayInfoV1RequestHandler
    for JayHeadManagerExtPhysicalDisplayInfoV1
{
    type Error = JayHeadExtPhysicalDisplayInfoV1Error;

    ext_common_req!(physical_display_info_v1);
}

impl JayHeadExtPhysicalDisplayInfoV1RequestHandler for JayHeadExtPhysicalDisplayInfoV1 {
    type Error = JayHeadExtPhysicalDisplayInfoV1Error;

    head_common_req!(physical_display_info_v1);
}

error! {
    PhysicalDisplayInfoV1
}
