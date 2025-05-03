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
pub type ClmMatchComm = ClmMatchString<CommAccess>;

pub struct AcceptorMetadataAccess<T>(PhantomData<T>);
pub struct CommAccess;

trait SandboxField: Sized + 'static {
    fn field(meta: &AcceptorMetadata) -> &Option<String>;
    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>>;
}

pub struct SandboxEngineField;
pub struct SandboxAppIdField;
pub struct SandboxInstanceIdField;

impl<T> StringAccess<Rc<Client>> for AcceptorMetadataAccess<T>
where
    T: SandboxField,
{
    fn with_string(data: &Rc<Client>, f: impl FnOnce(&str) -> bool) -> bool {
        f(T::field(&data.acceptor).as_deref().unwrap_or_default())
    }

    fn nodes(roots: &RootMatchers) -> &ClmRootMatcherMap<ClmMatchString<Self>> {
        T::nodes(roots)
    }
}

impl SandboxField for SandboxEngineField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.sandbox_engine
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_engine
    }
}

impl SandboxField for SandboxAppIdField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.app_id
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_app_id
    }
}

impl SandboxField for SandboxInstanceIdField {
    fn field(meta: &AcceptorMetadata) -> &Option<String> {
        &meta.instance_id
    }

    fn nodes(
        roots: &RootMatchers,
    ) -> &ClmRootMatcherMap<ClmMatchString<AcceptorMetadataAccess<Self>>> {
        &roots.sandbox_instance_id
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
