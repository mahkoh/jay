use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::Error;
use syn::Generics;
use syn::Item;
use syn::ItemStruct;
use syn::LitInt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;

pub fn derive_get_liveness(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let get_liveness = input.build_get_liveness();
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    let res = quote_spanned! { input.span =>
        const _: () = {
            #[automatically_derived]
            impl #impl_generics
            crate::utils::liveness::GetLiveness for #ident #type_generics
            #where_clause
            {
                #[inline(always)]
                fn get_liveness(&self) -> &crate::utils::liveness::Liveness {
                    crate::utils::liveness::GetLiveness::get_liveness(#get_liveness)
                }
            }
        };
    };
    res.into()
}

struct Input {
    span: Span,
    ident: Ident,
    generics: Generics,
    kind: Kind,
}

enum Kind {
    Struct(StructField),
}

struct StructField {
    pos: usize,
    name: Option<Ident>,
}

fn build_get_liveness_struct(field: &StructField) -> TokenStream {
    match &field.name {
        Some(name) => quote! { &self.#name },
        None => {
            let idx = LitInt::new(&field.pos.to_string(), Span::call_site());
            quote! { &self.#idx }
        }
    }
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut fields: Vec<_> = input
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.attrs.iter().any(|a| a.path().is_ident("liveness")))
            .collect();
        if fields.is_empty() {
            fields.extend(
                input
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.ident.as_ref().is_some_and(|i| i == "liveness")),
            );
        }
        if fields.len() != 1 {
            return Err(Error::new(
                input.span(),
                "Exactly one of the fields must be annotated with #[liveness] or have name liveness",
            ));
        }
        let (pos, field) = fields.pop().unwrap();
        let field = StructField {
            pos,
            name: field.ident.clone(),
        };
        Ok(Self {
            span,
            ident: input.ident,
            generics: input.generics,
            kind: Kind::Struct(field),
        })
    }

    fn build_get_liveness(&self) -> TokenStream {
        match &self.kind {
            Kind::Struct(s) => build_get_liveness_struct(s),
        }
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        match item {
            Item::Struct(s) => Self::parse_struct(s),
            _ => Err(Error::new(item.span(), "expected struct")),
        }
    }
}
