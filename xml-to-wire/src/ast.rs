pub(crate) struct Protocol {
    pub(crate) _name: String,
    pub(crate) _copyright: Option<Copyright>,
    pub(crate) _description: Option<Description>,
    pub(crate) interfaces: Vec<Interface>,
}

pub(crate) struct Copyright {
    pub(crate) _body: String,
}

#[derive(Debug)]
pub(crate) struct Description {
    pub(crate) _summary: Option<String>,
    pub(crate) _body: String,
}

pub(crate) struct Interface {
    pub(crate) name: String,
    pub(crate) _version: u32,
    pub(crate) _description: Option<Description>,
    pub(crate) messages: Vec<Message>,
    pub(crate) _enums: Vec<Enum>,
}

#[derive(Debug)]
pub(crate) struct Arg {
    pub(crate) name: String,
    pub(crate) ty: ArgType,
    pub(crate) _summary: Option<String>,
    pub(crate) _description: Option<Description>,
    pub(crate) interface: Option<String>,
    pub(crate) allow_null: bool,
    pub(crate) enum_: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum ArgType {
    NewId,
    Int,
    Uint,
    Fixed,
    String,
    Object,
    Array,
    Fd,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum MessageType {
    Destructor,
}

pub(crate) struct Entry {
    pub(crate) _name: String,
    pub(crate) _value: String,
    pub(crate) _value_u32: u32,
    pub(crate) _summary: Option<String>,
    pub(crate) _since: Option<u32>,
    pub(crate) _deprecated_since: Option<u32>,
    pub(crate) _description: Option<Description>,
}

pub(crate) struct Enum {
    pub(crate) _name: String,
    pub(crate) _since: Option<u32>,
    pub(crate) _bitfield: bool,
    pub(crate) _description: Option<Description>,
    pub(crate) _entries: Vec<Entry>,
}

pub(crate) struct Message {
    pub(crate) name: String,
    pub(crate) request: bool,
    pub(crate) ty: Option<MessageType>,
    pub(crate) since: Option<u32>,
    pub(crate) _deprecated_since: Option<u32>,
    pub(crate) _description: Option<Description>,
    pub(crate) args: Vec<Arg>,
}
