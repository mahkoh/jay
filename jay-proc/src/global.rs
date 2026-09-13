use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::Error;
use syn::Item;
use syn::ItemStruct;
use syn::Meta;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;

pub fn derive_global(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let ident = input.ident;
    let interface = match input.interface {
        Some(i) => i,
        _ => {
            let interface = ident
                .to_string()
                .strip_suffix("Global")
                .expect("Global name must end in Global")
                .to_string();
            Ident::new(&interface, ident.span())
        }
    };
    let mut add = quote! {
        let _ = globals;
    };
    let mut remove = quote! {
        let _ = globals;
    };
    if let Some(id) = &input.dedicated {
        add = quote! {
            globals.#id.set(self.name, self);
        };
        remove = quote! {
            globals.#id.remove(&self.name);
        };
    }
    let res = quote_spanned! { input.span =>
        impl crate::globals::GlobalBase for #ident {
            fn name(&self) -> crate::globals::GlobalName {
                self.name
            }

            fn bind<'a>(
                self: std::rc::Rc<Self>,
                client: &'a std::rc::Rc<crate::client::Client>,
                id: crate::wire::ObjectId,
                version: crate::object::Version,
            ) -> Result<(), crate::globals::GlobalsError> {
                if let Err(e) = self.bind_(id.into(), client, version) {
                    let e = crate::globals::GlobalError {
                        interface: crate::wire::#interface,
                        error: Box::new(e),
                    };
                    return Err(crate::globals::GlobalsError::GlobalError(e));
                }
                Ok(())
            }

            fn interface(&self) -> crate::object::Interface {
                crate::wire::#interface
            }

            fn singleton(&self) -> Option<crate::globals::Singleton> {
                crate::globals::interface_singletons::#interface
            }
        }

        impl crate::globals::WaylandGlobal for #ident {
            fn add(self: std::rc::Rc<Self>, globals: &crate::globals::Globals) {
                #add
            }
            fn remove(&self, globals: &crate::globals::Globals) {
                #remove
            }
        }
    };
    res.into()
}

struct Input {
    span: Span,
    ident: Ident,
    dedicated: Option<Ident>,
    interface: Option<Ident>,
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut dedicated = None;
        let mut interface = None;
        for attr in &input.attrs {
            if let Meta::List(l) = &attr.meta
                && l.path.is_ident("dedicated")
                && let Ok(id) = l.parse_args::<Ident>()
            {
                dedicated = Some(id);
            }
            if let Meta::List(l) = &attr.meta
                && l.path.is_ident("interface")
                && let Ok(id) = l.parse_args::<Ident>()
            {
                interface = Some(id);
            }
        }
        Ok(Self {
            span,
            ident: input.ident,
            dedicated,
            interface,
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
