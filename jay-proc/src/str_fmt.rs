use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::Error;
use syn::GenericParam;
use syn::Generics;
use syn::Item;
use syn::ItemStruct;
use syn::LitInt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::spanned::Spanned;

pub fn derive_str_fmt(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input: Input = parse_macro_input!(input as Input);
    let str_fmt = input.build_str_fmt();
    input.generics.make_where_clause();
    for ty in &input.generics.params {
        if let GenericParam::Type(ty) = ty {
            let ty = &ty.ident;
            input
                .generics
                .where_clause
                .as_mut()
                .unwrap()
                .predicates
                .push(parse_quote!(#ty: crate::utils::str_fmt::StrFmt));
        }
    }
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    let res = quote_spanned! { input.span =>
        const _: () = {
            #[automatically_derived]
            impl #impl_generics
            crate::utils::str_fmt::StrFmt for #ident #type_generics
            #where_clause
            {
                fn str_fmt(&self, dst: &mut String, ctx: &crate::utils::str_fmt::StrCtx) {
                    ctx.struct_prefix(dst);
                    #str_fmt
                    ctx.struct_suffix(dst);
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
    Struct(StructInput),
}

struct StructInput {
    fields: Vec<StructField>,
}

struct StructField {
    name: Option<Ident>,
}

fn build_str_fmt_struct(fields: &[StructField]) -> TokenStream {
    let mut parts = vec![];
    for (idx, field) in fields.iter().enumerate() {
        let (name, ref_name) = match &field.name {
            Some(i) => (i.to_string(), quote! { #i }),
            None => {
                let name = idx.to_string();
                let idx = LitInt::new(&name, Span::call_site());
                (name, quote! { #idx })
            }
        };
        let first = idx == 0;
        parts.push(quote! {
            ctx.struct_field(dst, #name, &self.#ref_name, #first);
        });
    }
    quote! {
        #(#parts)*
    }
}

impl StructInput {
    fn build_str_fmt(&self) -> TokenStream {
        build_str_fmt_struct(&self.fields)
    }
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut fields = vec![];
        for field in input.fields {
            fields.push(StructField { name: field.ident });
        }
        Ok(Self {
            span,
            ident: input.ident,
            generics: input.generics,
            kind: Kind::Struct(StructInput { fields }),
        })
    }

    fn build_str_fmt(&self) -> TokenStream {
        match &self.kind {
            Kind::Struct(s) => s.build_str_fmt(),
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
