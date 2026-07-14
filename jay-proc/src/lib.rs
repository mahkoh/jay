use {markers::clone, proc_macro::TokenStream};

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
