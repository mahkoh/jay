use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::Error;
use syn::Item;
use syn::ItemStruct;
use syn::LitStr;
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
    let include_path = LitStr::new(&format!("/dedicated/{}.rs", ident), ident.span());
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

        #break_loops

        include!(concat!(env!("OUT_DIR"), #include_path));
    };
    res.into()
}

struct Input {
    span: Span,
    ident: Ident,
    break_loops: bool,
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let mut break_loops = false;
        for attr in &input.attrs {
            if let Meta::Path(p) = &attr.meta
                && p.is_ident("break_loops")
            {
                break_loops = true;
            }
        }
        Ok(Self {
            span,
            ident: input.ident,
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
