use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Error;
use syn::Generics;
use syn::Item;
use syn::ItemStruct;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;

pub fn derive_cached_values(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let res = input.build_impl();
    res.into()
}

struct Input {
    ident: Ident,
    prefix: String,
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
    name: Ident,
    ty: Type,
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let mut fields = vec![];
        for field in input.fields {
            fields.push(StructField {
                name: field.ident.unwrap(),
                ty: field.ty,
            });
        }
        Ok(Self {
            prefix: input.ident.to_string(),
            ident: input.ident,
            generics: input.generics,
            kind: Kind::Struct(StructInput { fields }),
        })
    }

    fn build_impl(&self) -> TokenStream {
        let Kind::Struct(s) = &self.kind;
        let params = &self.generics.params;
        let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
        let ident = &self.ident;
        let changed_name = Ident::new(&format!("{}Changed", self.prefix), self.ident.span());
        let op_name = Ident::new(&format!("{}Op", self.prefix), self.ident.span());
        let mut op_fields = vec![];
        let mut apply_cases = vec![];
        let mut changed_fields = vec![];
        let mut set_stmts = vec![];
        let mut default_fields = vec![];
        let mut update_fields = vec![];
        for (idx, field) in s.fields.iter().enumerate() {
            let op_variant = Ident::new(&format!("V{idx}"), field.name.span());
            let field_name = &field.name;
            let ty = &field.ty;
            op_fields.push(quote! {
                #op_variant(<#ty as crate::utils::cached_value::CachedValue>::Op),
            });
            apply_cases.push(quote! {
                #op_name::#op_variant(v) => {
                    <#ty as crate::utils::cached_value::CachedValue>::cached_apply(&self.#field_name, v);
                }
            });
            changed_fields.push(quote! {
                #field_name: <#ty as crate::utils::cached_value::CachedValue>::Changed,
            });
            set_stmts.push(quote! {
                <#ty as crate::utils::cached_value::CachedValue>::cached_set(&self.#field_name, v.#field_name);
            });
            default_fields.push(quote! {
                #field_name: <#ty as crate::utils::cached_value::CachedDefault>::cached_default(),
            });
            update_fields.push(quote! {
                changed.#field_name = <#ty as crate::utils::cached_value::CachedValue>::cached_update(
                    &self.#field_name,
                    v.#field_name,
                    |c| {
                        handle_change(#op_name::#op_variant(c));
                    },
                );
            });
        }
        quote! {
            pub enum #op_name <#params> #where_clause {
                #(#op_fields)*
            }

            #[derive(::derivative::Derivative)]
            #[derivative(Copy(bound=""), Clone(bound=""), Debug(bound=""), Default(bound=""))]
            pub struct #changed_name <#params> #where_clause {
                #(#changed_fields)*
            }

            const _: () = {
                impl #impl_generics Default for #ident #type_generics #where_clause {
                    fn default() -> Self {
                        <#ident #type_generics as crate::utils::cached_value::CachedDefault>::cached_default()
                    }
                }

                impl #impl_generics crate::utils::cached_value::CachedDefault for #ident #type_generics #where_clause {
                    fn cached_default() -> Self {
                        Self {
                            #(#default_fields)*
                        }
                    }
                }

                impl #impl_generics crate::utils::cached_value::CachedValue for #ident #type_generics #where_clause {
                    type Changed = #changed_name #type_generics;
                    type Op = #op_name #type_generics;

                    fn cached_set(&self, v: Self) {
                        #(#set_stmts)*
                    }

                    fn cached_apply(&self, v: Self::Op) {
                        match v {
                            #(#apply_cases)*
                        }
                    }

                    fn cached_update(
                        &self,
                        v: Self,
                        mut handle_change: impl FnMut(Self::Op),
                    ) -> Self::Changed {
                        let mut changed = #changed_name::default();
                        #(#update_fields)*
                        changed
                    }
                }
            };
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
