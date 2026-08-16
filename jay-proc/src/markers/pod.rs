use crate::markers::Input;
use quote::quote;
use syn::Error;
use syn::GenericParam;
use syn::Item;
use syn::TypeParam;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::spanned::Spanned;

pub fn derive_pod(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut item: Input = parse_macro_input!(item as Input);
    match item.item {
        Item::Struct(_) => {}
        Item::Union(_) => {}
        _ => {
            return Error::new(item.item.span(), "Only structs can derive Pod")
                .to_compile_error()
                .into();
        }
    }
    let trait_impl = {
        item.generics.make_where_clause();
        for x in &item.generics.params {
            if let GenericParam::Type(TypeParam { ident, .. }) = x {
                let predicates = &mut item.generics.where_clause.as_mut().unwrap().predicates;
                predicates.push(parse_quote!(#ident: ::uapi::Pod));
            }
        }
        let (a, b, c) = item.generics.split_for_impl();
        let name = &item.ident;
        let tys = item.critical_types.iter().map(|t| {
            quote! {
                let _: crate::utils::pod::AssertPod<#t>;
            }
        });
        quote! {
            unsafe impl #a ::uapi::Pod for #name #b #c { }

            const _: () = {
                fn _assert #a () #c {
                    #(#tys)*
                }
            };
        }
    };
    let res = quote! { #trait_impl };
    res.into()
}
