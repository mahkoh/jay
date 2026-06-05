mod reset;

#[proc_macro_derive(Reset)]
pub fn derive_reset(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    reset::derive_reset(input)
}
