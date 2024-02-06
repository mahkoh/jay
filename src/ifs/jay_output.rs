use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        tree::OutputNode,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), JayOutputError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.remove_from_node();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn remove_from_node(&self) {
        if let Some(output) = self.output.get() {
            output.jay_outputs.remove(&(self.client.id, self.id));
        }
    }
}

object_base! {
    self = JayOutput;

    DESTROY => destroy,
}

impl Object for JayOutput {
    fn break_loops(&self) {
        self.remove_from_node();
    }
}

dedicated_add_obj!(JayOutput, JayOutputId, jay_outputs);

#[derive(Debug, Error)]
pub enum JayOutputError {
    #[error("Parsing failed")]
    MsgParserError(Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayOutputError, MsgParserError);
efrom!(JayOutputError, ClientError);
