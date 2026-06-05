use {
    proc_macro2::{Ident, Span, TokenStream},
    quote::{quote, quote_spanned},
    syn::{
        Error, Generics, Item, ItemStruct, LitInt, Type,
        parse::{Parse, ParseStream},
        parse_macro_input, parse_quote,
        spanned::Spanned,
    },
};

pub fn derive_reset(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input: Input = parse_macro_input!(input as Input);
    let reset = input.build_reset();
    let where_clause = input.generics.make_where_clause();
    for ty in &input.critical_types {
        where_clause
            .predicates
            .push(parse_quote!(#ty: crate::utils::reset::Reset));
    }
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    let res = quote_spanned! { input.span =>
        const _: () = {
            #[automatically_derived]
            impl #impl_generics
            crate::utils::reset::Reset for #ident #type_generics
            #where_clause
            {
                fn reset(&mut self) {
                    #reset
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
    critical_types: Vec<Type>,
    kind: Kind,
}

enum Kind {
    Struct(StructInput),
}

struct StructInput {
    fields: Vec<StructField>,
}

struct StructField {
    original_name: Option<Ident>,
    generated_name: Option<Ident>,
    ty: Type,
}

fn build_reset_struct(fields: &[StructField]) -> TokenStream {
    let mut parts = vec![];
    for (idx, field) in fields.iter().enumerate().rev() {
        let idx = LitInt::new(&idx.to_string(), Span::call_site());
        let ref_name = match &field.generated_name {
            Some(i) => quote! {#i},
            None => match &field.original_name {
                Some(i) => quote! { &mut self.#i },
                None => quote! { &mut self.#idx },
            },
        };
        let ty = &field.ty;
        parts.push(quote! {
            <#ty as crate::utils::reset::Reset>::reset(#ref_name);
        });
    }
    quote! {
        #(#parts)*
    }
}

impl StructInput {
    fn build_reset(&self) -> TokenStream {
        build_reset_struct(&self.fields)
    }
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut critical_types = Vec::new();
        let mut fields = vec![];
        for field in input.fields {
            critical_types.push(field.ty.clone());
            fields.push(StructField {
                original_name: field.ident,
                generated_name: None,
                ty: field.ty,
            });
        }
        Ok(Self {
            span,
            ident: input.ident,
            generics: input.generics,
            critical_types,
            kind: Kind::Struct(StructInput { fields }),
        })
    }

    fn build_reset(&self) -> TokenStream {
        match &self.kind {
            Kind::Struct(s) => s.build_reset(),
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
