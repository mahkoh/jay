use {
    crate::{
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_connection::EiConnection,
            ei_object::{EiInterface, EiObject, EiVersion, EI_HANDSHAKE_ID},
            EiContext,
        },
        leaks::Tracker,
        wire_ei::{
            ei_handshake::{
                ClientHandshakeVersion, ClientInterfaceVersion, Connection, ContextType,
                EiHandshakeRequestHandler, Finish, Name, ServerHandshakeVersion,
                ServerInterfaceVersion,
            },
            EiHandshake, EiHandshakeId,
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct EiHandshake {
    pub id: EiHandshakeId,
    client: Rc<EiClient>,
    version: Cell<EiVersion>,
    pub tracker: Tracker<Self>,
    have_context_type: Cell<bool>,
}

impl EiHandshake {
    pub fn new(client: &Rc<EiClient>) -> Self {
        Self {
            id: EI_HANDSHAKE_ID,
            client: client.clone(),
            version: Cell::new(EiVersion(1)),
            tracker: Default::default(),
            have_context_type: Cell::new(false),
        }
    }

    pub fn send_handshake_version(&self) {
        self.client.event(ServerHandshakeVersion {
            self_id: self.id,
            version: 1,
        });
    }

    fn send_interface_version(&self, interface: EiInterface, version: EiVersion) {
        self.client.event(ServerInterfaceVersion {
            self_id: self.id,
            name: interface.0,
            version: version.0,
        });
    }

    fn send_connection(&self, serial: u32, connection: &EiConnection) {
        self.client.event(Connection {
            self_id: self.id,
            serial,
            connection: connection.id,
            version: connection.version.0,
        });
    }
}

impl EiHandshakeRequestHandler for EiHandshake {
    type Error = EiHandshakeError;

    fn client_handshake_version(
        &self,
        req: ClientHandshakeVersion,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let version = EiVersion(req.version);
        if version > self.client.versions.ei_handshake.server_max_version {
            return Err(EiHandshakeError::UnknownHandshakeVersion);
        }
        self.client
            .versions
            .ei_handshake
            .set_client_version(version);
        Ok(())
    }

    fn finish(&self, _req: Finish, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if !self.have_context_type.get() {
            return Err(EiHandshakeError::NoContextType);
        }
        if self.client.name.borrow().is_none() {
            return Err(EiHandshakeError::NoName);
        }
        if self.client.versions.ei_connection.version.get() == EiVersion(0) {
            return Err(EiHandshakeError::NoConnectionVersion);
        }
        if self.client.versions.ei_callback.version.get() == EiVersion(0) {
            return Err(EiHandshakeError::NoCallbackVersion);
        }
        self.client.versions.for_each(|interface, version| {
            let version = version.version.get();
            if version > EiVersion(0) && interface != EiHandshake {
                self.send_interface_version(interface, version);
            }
        });
        let connection = Rc::new(EiConnection {
            id: self.client.new_id(),
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.client.versions.ei_connection.version.get(),
        });
        self.client.add_server_obj(&connection);
        track!(self.client, connection);
        self.client.connection.set(Some(connection.clone()));
        self.send_connection(self.client.serial(), &connection);
        self.client.remove_obj(self)?;
        for seat in self.client.state.globals.seats.lock().values() {
            connection.announce_seat(seat);
        }
        Ok(())
    }

    fn context_type(&self, req: ContextType, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.have_context_type.replace(true) {
            return Err(EiHandshakeError::ContextTypeSet);
        }
        let ty = match req.context_type {
            1 => EiContext::Receiver,
            2 => EiContext::Sender,
            _ => return Err(EiHandshakeError::UnknownContextType(req.context_type)),
        };
        self.client.context.set(ty);
        self.have_context_type.set(true);
        Ok(())
    }

    fn name(&self, req: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let name = &mut *self.client.name.borrow_mut();
        if name.is_some() {
            return Err(EiHandshakeError::NameSet);
        }
        *name = Some(req.name.to_string());
        Ok(())
    }

    fn client_interface_version(
        &self,
        req: ClientInterfaceVersion<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.client.versions.match_(req.name, |v| {
            v.set_client_version(EiVersion(req.version));
        });
        Ok(())
    }
}

ei_object_base! {
    self = EiHandshake;
    version = self.version.get();
}

impl EiObject for EiHandshake {
    fn context(&self) -> EiContext {
        panic!("context requested for EiHandshake")
    }
}

#[derive(Debug, Error)]
pub enum EiHandshakeError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("ei_handshake version is too large")]
    UnknownHandshakeVersion,
    #[error("Name is already set")]
    NameSet,
    #[error("Unknown context type {0}")]
    UnknownContextType(u32),
    #[error("Context type is already set")]
    ContextTypeSet,
    #[error("Client did not set connection version")]
    NoConnectionVersion,
    #[error("Client did not set callback version")]
    NoCallbackVersion,
    #[error("Client did not set context type")]
    NoContextType,
    #[error("Client did not set name")]
    NoName,
}
efrom!(EiHandshakeError, EiClientError);
