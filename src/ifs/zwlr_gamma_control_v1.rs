use {
    crate::{
        backend::BackendGammaLutElement,
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError},
        ifs::{
            wl_output::OutputGlobalOpt, zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwlrGammaControlV1Id, zwlr_gamma_control_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrGammaControlV1 {
    id: ZwlrGammaControlV1Id,
    client: Rc<Client>,
    version: Version,
    output: Rc<OutputGlobalOpt>,
    pub is_valid: Cell<bool>,
    pub tracker: Tracker<Self>,
}

impl ZwlrGammaControlV1 {
    pub fn new(
        id: ZwlrGammaControlV1Id,
        manager: &Rc<ZwlrGammaControlManagerV1>,
        output: Rc<OutputGlobalOpt>,
    ) -> Self {
        Self {
            id,
            client: manager.client.clone(),
            version: manager.version,
            output,
            is_valid: Cell::new(false),
            tracker: Default::default(),
        }
    }

    pub fn send_gamma_size(self: &Rc<Self>, size: u32) {
        self.client.event(GammaSize {
            self_id: self.id,
            size,
        });
    }

    pub fn fail(self: &Rc<Self>) {
        if self.is_valid.get() {
            // The spec says that gamma tables are only restored when the object is destroyed, and only if the object is still valid.
            // So, when are gamma tables restored when an invalid set_gamma is provided after a valid one?
            // Spec break: Restore gamma tables upon the object becoming invalid.
            self.detach();
            self.send_failed();
        }
    }

    pub fn send_failed(self: &Rc<Self>) {
        self.client.event(Failed { self_id: self.id });
    }

    pub fn gamma_lut_size(self: &Rc<Self>) -> Option<u32> {
        self.output
            .node()
            .and_then(|node| node.global.connector.connector.gamma_lut_size())
    }

    fn detach(&self) {
        if self.is_valid.get() {
            self.is_valid.set(false);
            if let Some(node) = self.output.node() {
                if node.has_valid_zwlr_gamma_control.get() {
                    node.has_valid_zwlr_gamma_control.set(false);
                    node.set_gamma_lut(Rc::new(None));
                }
            };
        }
    }

    // Wayland's LUT is ([red], [green], [blue]). DRM's LUT is [(red, green, blue, _)]. Both are u16.
    fn wayland_gamma_lut_to_drm_gamma_lut(data: &[u16]) -> Vec<BackendGammaLutElement> {
        let elem_count = data.len() / 3;
        let red = &data[..elem_count];
        let green = &data[elem_count..(2 * elem_count)];
        let blue = &data[(2 * elem_count)..(3 * elem_count)];
        red.iter()
            .copied()
            .zip(green.iter().copied())
            .zip(blue.iter().copied())
            .map(|((red, green), blue)| BackendGammaLutElement {
                red,
                green,
                blue,
                reserved: 0,
            })
            .collect()
    }
}

impl ZwlrGammaControlV1RequestHandler for ZwlrGammaControlV1 {
    type Error = ZwlrGammaControlV1Error;

    fn set_gamma(&self, req: SetGamma, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.is_valid.get() {
            return Ok(());
        }

        let Some(node) = self.output.node() else {
            slf.fail();
            return Ok(());
        };

        if !node.has_valid_zwlr_gamma_control.get() {
            slf.fail();
            return Ok(());
        }

        let Some(gamma_lut_size) = slf.gamma_lut_size() else {
            slf.fail();
            return Ok(());
        };

        // 3 color channels
        let data_size = gamma_lut_size * 3;

        let mut gamma_lut = vec![];
        Rc::new(ClientMem::new_private(
            &req.fd,
            (2 * data_size) as _,
            true,
            Some(&self.client),
            None,
        )?)
        .offset(0)
        .read(&mut gamma_lut)?;
        let gamma_lut = &gamma_lut[..data_size as _];

        let gamma_lut = Self::wayland_gamma_lut_to_drm_gamma_lut(gamma_lut);
        if !node.set_gamma_lut(Rc::new(Some(gamma_lut))) {
            slf.fail();
        }

        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrGammaControlV1;
    version = self.version;
}

impl Object for ZwlrGammaControlV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwlrGammaControlV1);

#[derive(Debug, Error)]
pub enum ZwlrGammaControlV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    CLientMemError(#[from] ClientMemError),
}
efrom!(ZwlrGammaControlV1Error, ClientError);
