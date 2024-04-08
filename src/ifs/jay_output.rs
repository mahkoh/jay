use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::OutputNode,
        utils::clonecell::CloneCell,
        wire::{jay_output::*, JayOutputId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayOutput {
    pub id: JayOutputId,
    pub client: Rc<Client>,
    pub output: CloneCell<Option<Rc<OutputNode>>>,
    pub tracker: Tracker<Self>,
}

impl JayOutput {
    pub fn send_destroyed(&self) {
        self.client.event(Destroyed { self_id: self.id });
    }

    pub fn send_linear_id(&self) {
        if let Some(output) = self.output.get() {
            self.client.event(LinearId {
                self_id: self.id,
                linear_id: output.id.raw(),
            });
        }
    }

    fn remove_from_node(&self) {
        if let Some(output) = self.output.get() {
            output.jay_outputs.remove(&(self.client.id, self.id));
        }
    }
}

impl JayOutputRequestHandler for JayOutput {
    type Error = JayOutputError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.remove_from_node();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayOutput;
    version = Version(1);
}

impl Object for JayOutput {
    fn break_loops(&self) {
        self.remove_from_node();
    }
}

dedicated_add_obj!(JayOutput, JayOutputId, jay_outputs);

#[derive(Debug, Error)]
pub enum JayOutputError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayOutputError, ClientError);
