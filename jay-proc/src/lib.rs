use markers::clone;
use markers::hash;
use proc_macro::TokenStream;

mod drm_object_properties;
mod markers;
mod reset;
mod str_fmt;

#[proc_macro_derive(Reset)]
pub fn derive_reset(input: TokenStream) -> TokenStream {
    reset::derive_reset(input)
}

#[proc_macro_derive(StrFmt)]
pub fn derive_str_fmt(input: TokenStream) -> TokenStream {
    str_fmt::derive_str_fmt(input)
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
