use {
    crate::{
        client::{Client, ClientError},
        ifs::head_management::{
            HeadTransaction, jay_head_manager_v1::JayHeadManagerV1,
            jay_head_transaction_result_v1::JayHeadTransactionResultV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            JayHeadTransactionResultV1Id, JayHeadTransactionV1Id,
            jay_head_transaction_v1::{Apply, Destroy, JayHeadTransactionV1RequestHandler, Test},
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayHeadTransactionV1 {
    pub id: JayHeadTransactionV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub manager: Rc<JayHeadManagerV1>,
    pub tran: Rc<HeadTransaction>,
    pub serial: u64,
}

impl JayHeadTransactionV1 {
    fn execute(
        &self,
        id: JayHeadTransactionResultV1Id,
        _apply: bool,
    ) -> Result<(), JayHeadTransactionV1Error> {
        let obj = Rc::new(JayHeadTransactionResultV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        if self.serial == self.manager.serial.get() {
            obj.send_success();
        } else {
            obj.send_failed();
        }
        Ok(())
    }
}

impl JayHeadTransactionV1RequestHandler for JayHeadTransactionV1 {
    type Error = JayHeadTransactionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn apply(&self, req: Apply, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.execute(req.result, true)?;
        Ok(())
    }

    fn test(&self, req: Test, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.execute(req.result, false)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadTransactionV1;
    version = self.version;
}

impl Object for JayHeadTransactionV1 {}

dedicated_add_obj!(
    JayHeadTransactionV1,
    JayHeadTransactionV1Id,
    jay_head_transactions
);

#[derive(Debug, Error)]
pub enum JayHeadTransactionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayHeadTransactionV1Error, ClientError);
