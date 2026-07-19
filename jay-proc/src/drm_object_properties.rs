use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::Error;
use syn::Generics;
use syn::Item;
use syn::ItemStruct;
use syn::LitStr;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;

pub fn derive_prepare_drm_object_properties(
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let mut reset_body = vec![];
    let mut update_body = vec![];
    let mut prepare_body = vec![];
    let mut differs_body = vec![];
    let mut prepare_conditional_body = vec![];
    for field in input.fields {
        let name = LitStr::new(&field.to_string(), field.span());
        reset_body.push(quote! {
            crate::video::drm::PrepareDrmObjectProperty::reset(&mut self.#field);
        });
        update_body.push(quote! {
            crate::video::drm::PrepareDrmObjectProperty::update(&mut self.#field, p);
        });
        prepare_body.push(quote! {
            crate::video::drm::PrepareDrmObjectProperty::prepare(&self.#field, change, #name, logging);
        });
        differs_body.push(quote! {
            crate::video::drm::PrepareDrmObjectProperty::differs(&self.#field, &old.#field)
        });
        prepare_conditional_body.push(quote! {
            crate::video::drm::PrepareDrmObjectProperty::prepare_conditional(&self.#field, &old.#field, change, #name, logging);
        });
    }
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let ident = input.ident;
    let res = quote_spanned! { input.span =>
        const _: () = {
            impl #impl_generics
            crate::utils::reset::Reset for #ident #type_generics
            #where_clause
            {
                fn reset(&mut self) {
                    #(#reset_body)*
                }
            }

            impl #impl_generics
            crate::video::drm::PrepareDrmObjectProperties for #ident #type_generics
            #where_clause
            {
                fn update(
                    &mut self,
                    p: &crate::utils::bhash::BHashMap<crate::video::drm::DrmProperty, u64>,
                ) {
                    #(#update_body)*
                }
                fn prepare(
                    &self,
                    change: &mut crate::video::drm::ObjectChange<'_>,
                    logging: Option<&crate::video::drm::Logging>,
                ) {
                    #(#prepare_body)*
                }
                fn differs(
                    &self,
                    old: &Self,
                ) -> bool {
                    #(#differs_body)||*
                }
                fn prepare_conditional(
                    &self,
                    old: &Self,
                    change: &mut crate::video::drm::ObjectChange<'_>,
                    logging: Option<&crate::video::drm::Logging>,
                ) {
                    #(#prepare_conditional_body)*
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
    fields: Vec<Ident>,
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut fields = vec![];
        for field in input.fields {
            let span = field.span();
            let ident = field
                .ident
                .ok_or(Error::new(span, "field names are required"))?;
            fields.push(ident);
        }
        Ok(Self {
            span,
            ident: input.ident,
            generics: input.generics,
            fields,
        })
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
