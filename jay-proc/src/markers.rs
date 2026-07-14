use {
    proc_macro2::Ident,
    syn::{
        Error, Generics, Item, Type,
        parse::{Parse, ParseStream},
        spanned::Spanned,
    },
};

pub mod clone;

struct Input {
    item: Item,
    ident: Ident,
    generics: Generics,
    critical_types: Vec<Type>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        let ident;
        let generics;
        let mut critical_types = Vec::new();
        match &item {
            Item::Struct(s) => {
                ident = s.ident.clone();
                generics = s.generics.clone();
                for field in &s.fields {
                    critical_types.push(field.ty.clone());
                }
            }
            Item::Enum(s) => {
                ident = s.ident.clone();
                generics = s.generics.clone();
                for variant in &s.variants {
                    for field in &variant.fields {
                        critical_types.push(field.ty.clone());
                    }
                }
            }
            _ => return Err(Error::new(item.span(), "expected struct or enum")),
        }
        Ok(Self {
            item,
            ident,
            generics,
            critical_types,
        })
    }
}
