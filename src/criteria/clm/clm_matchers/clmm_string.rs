use {
    crate::{
        client::Client,
        criteria::{
            clm::{ClmRootMatcherMap, RootMatchers},
            crit_matchers::critm_string::{CritMatchString, StringAccess},
        },
        security_context_acceptor::AcceptorMetadata,
    },
    std::{marker::PhantomData, rc::Rc},
};

pub type ClmMatchString<T> = CritMatchString<Rc<Client>, T>;

pub type ClmMatchSandboxEngine = ClmMatchString<AcceptorMetadataAccess<SandboxEngineField>>;
pub type ClmMatchSandboxAppId = ClmMatchString<AcceptorMetadataAccess<SandboxAppIdField>>;
pub type ClmMatchSandboxInstanceId = ClmMatchString<AcceptorMetadataAccess<SandboxInstanceIdField>>;
pub type ClmMatchTag = ClmMatchString<AcceptorMetadataAccess<TagField>>;
pub type ClmMatchComm = ClmMatchString<CommAccess>;
pub type ClmMatchExe = ClmMatchString<ExeAccess>;

pub struct AcceptorMetadataAccess<T>(PhantomData<T>);
pub struct CommAccess;
pub struct ExeAccess;

trait AcceptorMetadataField: Sized + 'static {
    fn field(meta: &AcceptorMetadata) -> &Option<String>;
    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>>;
}

pub struct SandboxEngineField;
pub struct SandboxAppIdField;
pub struct SandboxInstanceIdField;
pub struct TagField;

impl<T> StringAccess<Rc<Client>> for AcceptorMetadataAccess<T>
where
    T: AcceptorMetadataField,
{
    fn with_string(data: &Rc<Client>, f: impl FnOnce(&str) -> bool) -> bool {
        f(T::field(&data.acceptor).as_deref().unwrap_or_default())
    }

    fn nodes(roots: &RootMatchers) -> &ClmRootMatcherMap<ClmMatchString<Self>> {
        T::nodes(roots)
    }
}

impl AcceptorMetadataField for SandboxEngineField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.sandbox_engine
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_engine
    }
}

impl AcceptorMetadataField for SandboxAppIdField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.app_id
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_app_id
    }
}

impl AcceptorMetadataField for SandboxInstanceIdField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.instance_id
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_instance_id
    }
}

impl AcceptorMetadataField for TagField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.tag
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.tag
    }
}

impl StringAccess<Rc<Client>> for CommAccess {
    fn with_string(data: &Rc<Client>, f: impl FnOnce(&str) -> bool) -> bool {
        f(&data.pid_info.comm)
    }

    fn nodes(roots: &RootMatchers) -> &ClmRootMatcherMap<ClmMatchString<Self>> {
        &roots.comm
    }
}

impl StringAccess<Rc<Client>> for ExeAccess {
    fn with_string(data: &Rc<Client>, f: impl FnOnce(&str) -> bool) -> bool {
        f(&data.pid_info.exe)
    }

    fn nodes(roots: &RootMatchers) -> &ClmRootMatcherMap<ClmMatchString<Self>> {
        &roots.exe
    }
}
