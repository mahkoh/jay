use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote_spanned;
use syn::Error;
use syn::Item;
use syn::ItemEnum;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;

pub fn derive_private_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let ident = input.ident;
    let private_ident = Ident::new(&format!("Jc{}", ident), ident.span());
    let variants = &input.variants;
    let res = quote_spanned! { input.span =>
        /// This is an implementation detail. Don't use it.
        #[derive(::serde::Serialize, ::serde::Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
        #[doc(hidden)]
        pub enum #private_ident {
            #(#variants,)*
        }
        impl #private_ident {
            #[allow(dead_code)]
            pub(crate) fn to_public(self) -> #ident {
                match self {
                    #(#private_ident::#variants => #ident::#variants,)*
                }
            }
        }
        impl #ident {
            #[allow(dead_code)]
            pub(crate) fn to_private(self) -> #private_ident {
                match self {
                    #(#ident::#variants => #private_ident::#variants,)*
                }
            }
        }
    };
    res.into()
}

struct Input {
    span: Span,
    ident: Ident,
    variants: Vec<Ident>,
}

impl Input {
    fn parse_enum(input: ItemEnum) -> syn::Result<Self> {
        let span = input.span();
        let mut variants = vec![];
        for variant in input.variants {
            variants.push(variant.ident);
        }
        Ok(Self {
            span,
            ident: input.ident,
            variants,
        })
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let item: Item = input.parse()?;
        match item {
            Item::Enum(s) => Self::parse_enum(s),
            _ => Err(Error::new(item.span(), "expected enum")),
        }
    }
}
