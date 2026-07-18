use {
    markers::{clone, hash},
    proc_macro::TokenStream,
};

mod drm_object_properties;
mod markers;
mod reset;

#[proc_macro_derive(Reset)]
pub fn derive_reset(input: TokenStream) -> TokenStream {
    reset::derive_reset(input)
}

#[proc_macro_attribute]
pub fn jay_clone(attr: TokenStream, item: TokenStream) -> TokenStream {
    clone::derive_jay_clone(attr, item)
}

#[proc_macro_attribute]
pub fn jay_hash(attr: TokenStream, item: TokenStream) -> TokenStream {
    hash::derive_jay_hash(attr, item)
}

#[proc_macro_derive(PrepareDrmObjectProperties)]
pub fn derive_prepare_drm_object_properties(input: TokenStream) -> TokenStream {
    drm_object_properties::derive_prepare_drm_object_properties(input)
}
