use {
    crate::{
        backend::{BackendGammaLut, BackendGammaLutElement},
        client::{Client, ClientError, ClientId},
        clientmem::{ClientMem, ClientMemError},
        ifs::{
            wl_output::OutputGlobalOpt, zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwlrGammaControlV1Id, zwlr_gamma_control_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwlrGammaControlV1 {
    id: ZwlrGammaControlV1Id,
    client: Rc<Client>,
    version: Version,
    output: Rc<OutputGlobalOpt>,
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
            tracker: Default::default(),
        }
    }

    pub fn id(&self) -> (ClientId, ZwlrGammaControlV1Id) {
        (self.client.id, self.id)
    }

    pub fn send_gamma_size(&self, size: u32) {
        self.client.event(GammaSize {
            self_id: self.id,
            size,
        });
    }

    pub fn send_failed(&self) {
        self.client.event(Failed { self_id: self.id });
    }

    pub fn gamma_lut_size(&self) -> Option<u32> {
        self.output
            .node()
            .and_then(|node| node.global.connector.connector.gamma_lut_size())
    }

    fn detach(&self) {
        if let Some(node) = self.output.node()
            && let Some(active_zwlr_gamma_control) = node.active_zwlr_gamma_control.get()
            && active_zwlr_gamma_control.id() == self.id()
        {
            node.active_zwlr_gamma_control.set(None);
            let _ = node.set_gamma_lut(None);
        }
    }
}

// Wayland's LUT is ([red], [green], [blue]). DRM's LUT is [(red, green, blue, _)]. Both are u16.
fn wayland_gamma_lut_to_drm_gamma_lut(data: &[u16]) -> Vec<BackendGammaLutElement> {
    let elem_count = data.len() / 3;
    let (red, rest) = data.split_at(elem_count);
    let (green, blue) = rest.split_at(elem_count);
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

impl ZwlrGammaControlV1RequestHandler for ZwlrGammaControlV1 {
    type Error = ZwlrGammaControlV1Error;

    fn set_gamma(&self, req: SetGamma, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let fail = || {
            self.detach();
            self.send_failed();
        };

        let Some(node) = self.output.node() else {
            return Ok(());
        };

        // if the active gamma control isn't us, that implies we are not valid, and have already
        // sent a failed event
        if node.active_zwlr_gamma_control.get().map(|v| v.id()) != Some(self.id()) {
            return Ok(());
        }

        let Some(gamma_lut_size) = self.gamma_lut_size() else {
            fail();
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

        let gamma_lut = wayland_gamma_lut_to_drm_gamma_lut(gamma_lut);
        let gamma_lut = Rc::new(BackendGammaLut::new(gamma_lut));
        if node.set_gamma_lut(Some(gamma_lut)).is_err() {
            fail();
            return Ok(());
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
