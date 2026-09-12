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

pub fn derive_object(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Input = parse_macro_input!(input as Input);
    let ident = input.ident;
    let mut break_loops = quote! {};
    if !input.break_loops {
        break_loops = quote_spanned! { input.span =>
            impl crate::object::BreakLoops for #ident { }
        };
    }
    let mut add = quote! {
        let _ = client;
    };
    let mut remove = quote! {
        let _ = client;
    };
    let mut lookup = quote! {};
    if let Some(field) = input.dedicated {
        add = quote_spanned! { input.span => {
            client.objects.#field.set(self.id, self.clone());
        }};
        remove = quote_spanned! { input.span => {
            client.objects.#field.remove(&self.id);
        }};
        let idname = Ident::new(&format!("{}Id", ident), ident.span());
        lookup = quote_spanned! { input.span =>
            impl crate::client::WaylandObjectLookup for crate::wire::#idname {
                type Object = #ident;
                const INTERFACE: crate::object::Interface = crate::wire::#ident;

                fn lookup(client: &crate::client::Client, id: Self) -> Option<Rc<#ident>> {
                    client.objects.#field.get(&id)
                }
            }
        };
    }
    let res = quote_spanned! { input.span =>
        impl crate::object::Object for #ident {
            fn id(&self) -> crate::wire::ObjectId {
                self.id.into()
            }

            fn version(&self) -> crate::object::Version {
                self.version
            }

            fn handle_request(
                self: std::rc::Rc<Self>,
                client: &crate::client::Client,
                request: u32,
                parser: crate::utils::buffd::MsgParser<'_, '_>,
            ) -> Result<(), crate::client::ClientError> {
                self.handle_request_impl(client, request, parser)
            }

            fn interface(&self) -> crate::object::Interface {
                crate::wire::#ident
            }
        }

        impl crate::object::AddObject for #ident {
            fn add(self: &Rc<Self>, client: &crate::client::Client) where Self: Sized {
                #add
            }

            fn remove(&self, client: &crate::client::Client) where Self: Sized {
                #remove
            }
        }

        #lookup

        #break_loops
    };
    res.into()
}

struct Input {
    span: Span,
    ident: Ident,
    dedicated: Option<Ident>,
    break_loops: bool,
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut break_loops = false;
        let mut dedicated = None;
        for attr in &input.attrs {
            if let Meta::Path(p) = &attr.meta
                && p.is_ident("break_loops")
            {
                break_loops = true;
            }
            if let Meta::List(l) = &attr.meta
                && l.path.is_ident("dedicated")
                && let Ok(id) = l.parse_args::<Ident>()
            {
                dedicated = Some(id);
            }
        }
        Ok(Self {
            span,
            ident: input.ident,
            dedicated,
            break_loops,
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
