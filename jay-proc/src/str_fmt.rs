use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::Attribute;
use syn::Error;
use syn::Generics;
use syn::Item;
use syn::ItemStruct;
use syn::LitInt;
use syn::Meta;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::spanned::Spanned;

pub fn derive_str_fmt(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input: Input = parse_macro_input!(input as Input);
    let str_fmt = match input.build_str_fmt() {
        Ok(s) => s,
        Err(e) => return e.into_compile_error().into(),
    };
    input.generics.make_where_clause();
    for field in &input.fields {
        let ty = &field.ty;
        input
            .generics
            .where_clause
            .as_mut()
            .unwrap()
            .predicates
            .push(parse_quote!(#ty: crate::utils::str_fmt::StrFmt));
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
                fn str_fmt(&self, dst: &mut String, ctx: &crate::utils::str_fmt::StrCtx<'_>) {
                    #str_fmt
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
    fields: Vec<StructField>,
    transparent: bool,
}

struct StructField {
    name: Option<Ident>,
    pos: usize,
    ty: Type,
}

fn field_ref(field: &StructField) -> (String, TokenStream) {
    match &field.name {
        Some(i) => (i.to_string(), quote! { #i }),
        None => {
            let name = field.pos.to_string();
            let idx = LitInt::new(&name, Span::call_site());
            (name, quote! { #idx })
        }
    }
}

fn build_str_fmt_struct(fields: &[StructField]) -> TokenStream {
    let mut parts = vec![];
    for (idx, field) in fields.iter().enumerate() {
        let (name, ref_name) = field_ref(field);
        let first = idx == 0;
        parts.push(quote! {
            ctx.struct_field(dst, #name, &self.#ref_name, #first);
        });
    }
    quote! {
        ctx.struct_prefix(dst);
        #(#parts)*
        ctx.struct_suffix(dst);
    }
}

fn build_str_fmt_transparent(span: Span, fields: &[StructField]) -> syn::Result<TokenStream> {
    let [field] = fields else {
        return Err(Error::new(
            span,
            "transparent requires exactly one field that is not skipped",
        ));
    };
    let (_, ref_name) = field_ref(field);
    Ok(quote! {
        crate::utils::str_fmt::StrFmt::str_fmt(&self.#ref_name, dst, ctx);
    })
}

impl Input {
    fn parse_struct(input: ItemStruct) -> syn::Result<Self> {
        let span = input.span();
        let transparent = has_attr(&input.attrs, "transparent");
        let mut fields = vec![];
        for (pos, field) in input.fields.into_iter().enumerate() {
            if has_attr(&field.attrs, "skip") {
                continue;
            }
            fields.push(StructField {
                name: field.ident,
                pos,
                ty: field.ty,
            });
        }
        Ok(Self {
            span,
            ident: input.ident,
            generics: input.generics,
            fields,
            transparent,
        })
    }

    fn build_str_fmt(&self) -> syn::Result<TokenStream> {
        match self.transparent {
            true => build_str_fmt_transparent(self.span, &self.fields),
            false => Ok(build_str_fmt_struct(&self.fields)),
        }
    }
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    for attr in attrs {
        if let Meta::List(l) = &attr.meta
            && l.path.is_ident("str_fmt")
            && let Ok(id) = l.parse_args::<Ident>()
            && id == name
        {
            return true;
        }
    }
    false
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
