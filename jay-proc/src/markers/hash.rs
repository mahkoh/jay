use {
    crate::markers::Input,
    quote::quote,
    syn::{
        GenericParam, TypeParam,
        parse::{Parse, ParseStream},
        parse_macro_input, parse_quote,
    },
};

pub fn derive_jay_hash(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let _attrs: Attributes = parse_macro_input!(attr as Attributes);
    let mut item: Input = parse_macro_input!(item as Input);
    let trait_impl = {
        item.generics.make_where_clause();
        for x in &item.generics.params {
            if let GenericParam::Type(TypeParam { ident, .. }) = x {
                let predicates = &mut item.generics.where_clause.as_mut().unwrap().predicates;
                predicates.push(parse_quote!(#ident: PartialEq));
                predicates.push(parse_quote!(#ident: Hash));
                predicates.push(parse_quote!(#ident: crate::utils::markers::JayHash));
            }
        }
        let (a, b, c) = item.generics.split_for_impl();
        let name = &item.ident;
        let tys = item.critical_types.iter().map(|t| {
            quote! {
                let _: crate::utils::markers::AssertJayHash<#t>;
            }
        });
        quote! {
            unsafe impl #a  crate::utils::markers::JayHash for #name #b  #c {
                fn _assert(&self) {
                    #(#tys)*
                }
            }
        }
    };
    let res = {
        let item = &item.item;
        quote! {
            #[derive(PartialEq, Hash)]
            #item

            #trait_impl
        }
    };
    res.into()
}

struct Attributes {}

impl Parse for Attributes {
    fn parse(_input: ParseStream) -> syn::Result<Self> {
        Ok(Attributes {})
    }
}
