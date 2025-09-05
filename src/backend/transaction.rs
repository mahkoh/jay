use {
    crate::{
        backend::{
            BackendColorSpace, BackendConnectorState, BackendEotfs, Connector, ConnectorId,
            ConnectorKernelId, Mode,
        },
        backends::metal::MetalError,
        state::State,
        utils::{errorfmt::ErrorFmt, hash_map_ext::HashMapExt},
        video::drm::DrmError,
    },
    ahash::AHashMap,
    std::{
        any::{Any, TypeId},
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        hash::{Hash, Hasher},
        rc::Rc,
    },
    thiserror::Error,
};

pub trait BackendConnectorTransactionType: Hash + Eq + Any {}

pub trait BackendConnectorTransactionTypeDyn: Any {
    fn eq(&self, other: &dyn BackendConnectorTransactionTypeDyn) -> bool;
    fn hash(&self, hasher: &mut dyn Hasher);
}

impl<T> BackendConnectorTransactionTypeDyn for T
where
    T: BackendConnectorTransactionType,
{
    fn eq(&self, other: &dyn BackendConnectorTransactionTypeDyn) -> bool {
        let Some(other) = (other as &dyn Any).downcast_ref::<Self>() else {
            return false;
        };
        self.eq(other)
    }

    fn hash(&self, hasher: &mut dyn Hasher) {
        struct BufHasher<'a> {
            buf: Vec<u8>,
            clear: Cell<bool>,
            any: Cell<bool>,
            hasher: RefCell<&'a mut dyn Hasher>,
        }
        impl Hasher for BufHasher<'_> {
            fn finish(&self) -> u64 {
                let hasher = &mut *self.hasher.borrow_mut();
                if self.any.take() {
                    self.clear.set(true);
                    hasher.write(&self.buf);
                }
                hasher.finish()
            }

            fn write(&mut self, bytes: &[u8]) {
                if self.clear.take() {
                    self.buf.clear();
                }
                self.any.set(true);
                self.buf.extend_from_slice(bytes);
            }
        }
        let mut hasher = BufHasher {
            buf: Default::default(),
            clear: Cell::new(false),
            any: Cell::new(false),
            hasher: RefCell::new(hasher),
        };
        TypeId::of::<Self>().hash(&mut hasher);
        self.hash(&mut hasher)
    }
}

impl PartialEq for dyn BackendConnectorTransactionTypeDyn {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl Eq for dyn BackendConnectorTransactionTypeDyn {}

impl Hash for dyn BackendConnectorTransactionTypeDyn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state)
    }
}

#[derive(Debug, Error)]
pub enum BackendConnectorTransactionError {
    #[error("The underlying DRM device of connector {} no longer exists", .0)]
    MissingDrmDevice(ConnectorKernelId),
    #[error("Connector {} does not support transactions", .0)]
    TransactionsNotSupported(ConnectorKernelId),
    #[error("Connector {} is not supported by this transaction", .0)]
    UnsupportedConnectorType(ConnectorKernelId),
    #[error("Connector {} cannot be modified because it is leased", .0)]
    LeasedConnector(ConnectorKernelId),
    #[error("Connector {} does not exist", .0)]
    UnknownConnector(ConnectorKernelId),
    #[error("Cannot initialize connector {} because no CRTC is available", .0)]
    NoCrtcForConnector(ConnectorKernelId),
    #[error("Cannot initialize connector {} because no primary plane is available", .0)]
    NoPrimaryPlaneForConnector(ConnectorKernelId),
    #[error("Connector {} does not support the requested mode {}", .0, .1)]
    UnsupportedMode(ConnectorKernelId, Mode),
    #[error("Connector {} does not support VRR", .0)]
    NotVrrCapable(ConnectorKernelId),
    #[error("Connector {} does not support tearing", .0)]
    TearingNotSupported(ConnectorKernelId),
    #[error("Connector {} does not support color space {:?}", .0, .1)]
    ColorSpaceNotSupported(ConnectorKernelId, BackendColorSpace),
    #[error("Connector {} does not support EOTF {:?}", .0, .1)]
    EotfNotSupported(ConnectorKernelId, BackendEotfs),
    #[error("Could not create an hdr metadata blob")]
    CreateHdrMetadataBlob(#[source] DrmError),
    #[error("Could not create a mode blob")]
    CreateModeBlob(#[source] DrmError),
    #[error("Could not allocate buffers for connector {}", .0)]
    AllocateScanoutBuffers(ConnectorKernelId, #[source] Box<MetalError>),
    #[error("Test commit failed")]
    AtomicTestFailed(#[source] DrmError),
    #[error("Commit failed")]
    AtomicCommitFailed(#[source] DrmError),
}

pub trait BackendConnectorTransaction {
    fn add(
        &mut self,
        connector: &Rc<dyn Connector>,
        change: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError>;

    fn prepare(
        self: Box<Self>,
    ) -> Result<Box<dyn BackendPreparedConnectorTransaction>, BackendConnectorTransactionError>;
}

pub trait BackendPreparedConnectorTransaction {
    fn apply(
        self: Box<Self>,
    ) -> Result<Box<dyn BackendAppliedConnectorTransaction>, BackendConnectorTransactionError>;
}

pub trait BackendAppliedConnectorTransaction {
    fn commit(self: Box<Self>);

    fn rollback(self: Box<Self>) -> Result<(), BackendConnectorTransactionError>;
}

struct Common {
    state: Rc<State>,
    states: AHashMap<ConnectorId, BackendConnectorState>,
}

pub struct ConnectorTransaction {
    common: Common,
    parts:
        AHashMap<Box<dyn BackendConnectorTransactionTypeDyn>, Box<dyn BackendConnectorTransaction>>,
}

pub struct PreparedConnectorTransaction {
    common: Common,
    parts: Vec<Box<dyn BackendPreparedConnectorTransaction>>,
}

pub struct AppliedConnectorTransaction {
    common: Common,
    parts: Vec<Box<dyn BackendAppliedConnectorTransaction>>,
}

impl ConnectorTransaction {
    pub fn new(state: &Rc<State>) -> Self {
        Self {
            common: Common {
                state: state.clone(),
                states: Default::default(),
            },
            parts: Default::default(),
        }
    }

    pub fn add(
        &mut self,
        connector: &Rc<dyn Connector>,
        mut state: BackendConnectorState,
    ) -> Result<(), BackendConnectorTransactionError> {
        state.serial = self.common.state.backend_connector_state_serials.next();
        let ty = connector.transaction_type();
        let tran = match self.parts.entry(ty) {
            Entry::Occupied(v) => v.into_mut(),
            Entry::Vacant(v) => v.insert(connector.create_transaction()?),
        };
        tran.add(connector, state)?;
        self.common.states.insert(connector.id(), state);
        Ok(())
    }

    pub fn prepare(
        mut self,
    ) -> Result<PreparedConnectorTransaction, BackendConnectorTransactionError> {
        let mut new = vec![];
        for tran in self.parts.drain_values() {
            new.push(tran.prepare()?);
        }
        Ok(PreparedConnectorTransaction {
            common: self.common,
            parts: new,
        })
    }
}

impl PreparedConnectorTransaction {
    pub fn apply(self) -> Result<AppliedConnectorTransaction, BackendConnectorTransactionError> {
        let mut applied = AppliedConnectorTransaction {
            common: self.common,
            parts: vec![],
        };
        for tran in self.parts {
            applied.parts.push(tran.apply()?);
        }
        Ok(applied)
    }
}

impl AppliedConnectorTransaction {
    pub fn commit(mut self) {
        for tran in self.parts.drain(..) {
            tran.commit();
        }
        for (connector_id, state) in self.common.states.drain() {
            if let Some(c) = self.common.state.connectors.get(&connector_id) {
                c.set_state(&self.common.state, state);
            }
        }
    }
}

impl Drop for AppliedConnectorTransaction {
    fn drop(&mut self) {
        for tran in self.parts.drain(..).rev() {
            if let Err(e) = tran.rollback() {
                log::error!("Could not roll back transaction: {}", ErrorFmt(e));
            }
        }
    }
}
