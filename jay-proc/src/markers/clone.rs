use crate::markers::Input;
use proc_macro2::Ident;
use quote::quote;
use syn::Error;
use syn::GenericParam;
use syn::Token;
use syn::TypeParam;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::parse_quote;

pub fn derive_jay_clone(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs: Attributes = parse_macro_input!(attr as Attributes);
    let mut item: Input = parse_macro_input!(item as Input);
    let derive = match attrs.copy {
        true => quote!(#[derive(Copy, Clone)]),
        false => quote!(#[derive(Clone)]),
    };
    let trait_impl = {
        item.generics.make_where_clause();
        for x in &item.generics.params {
            if let GenericParam::Type(TypeParam { ident, .. }) = x {
                let predicates = &mut item.generics.where_clause.as_mut().unwrap().predicates;
                predicates.push(parse_quote!(#ident: Clone));
                predicates.push(parse_quote!(#ident: crate::utils::markers::JayClone));
            }
        }
        let (a, b, c) = item.generics.split_for_impl();
        let name = &item.ident;
        let tys = item.critical_types.iter().map(|t| {
            quote! {
                let _: crate::utils::markers::AssertJayClone<#t>;
            }
        });
        quote! {
            unsafe impl #a  crate::utils::markers::JayClone for #name #b  #c {
                fn _assert(&self) {
                    #(#tys)*
                }
            }
        }
    };
    let res = {
        let item = &item.item;
        quote! {
            #derive
            #item

            #trait_impl
        }
    };
    res.into()
}

struct Attributes {
    copy: bool,
}

impl Parse for Attributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let idents = input.parse_terminated(Ident::parse, Token![,])?;
        let mut copy = false;
        for ident in idents {
            match &*ident.to_string() {
                "Copy" => copy = true,
                _ => return Err(Error::new(ident.span(), "unexpected trait")),
            }
        }
        Ok(Attributes { copy })
    }
}
