use proc_macro::Delimiter;
use proc_macro::Group;
use proc_macro::Ident;
use proc_macro::Literal;
use proc_macro::Punct;
use proc_macro::Spacing;
use proc_macro::Span;
use proc_macro::TokenStream;
use proc_macro::TokenTree;

pub fn extract_ident(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();
    let mut error = TokenStream::new();
    let mut macro_path = TokenStream::new();
    for tt in tokens.by_ref() {
        if let TokenTree::Punct(p) = &tt
            && *p == ','
        {
            break;
        }
        macro_path.extend([tt]);
    }
    let mut ident = vec![];
    let full = extract_ident_(&mut ident, tokens);
    let ident = match ident.len() {
        0 => {
            error.extend(compile_error(
                Span::call_site(),
                "Must contain exactly one @<ident> occurrence",
            ));
            Ident::new("_missing", Span::call_site())
        }
        1 => ident.pop().unwrap(),
        _ => {
            error.extend(compile_error(
                ident[1].span(),
                "Must contain exactly one @<ident> occurrence",
            ));
            ident.into_iter().next().unwrap()
        }
    };
    let mut inner = TokenStream::new();
    inner.extend([
        TokenTree::Ident(ident), //
        TokenTree::Punct(Punct::new(',', Spacing::Alone)),
    ]);
    inner.extend(full);
    let mut out = TokenStream::new();
    out.extend(error);
    out.extend(macro_path);
    out.extend([
        TokenTree::Group(Group::new(Delimiter::Parenthesis, inner)), //
    ]);
    out
}

#[allow(clippy::useless_conversion)]
fn extract_ident_(
    ident: &mut Vec<Ident>,
    tokens: impl IntoIterator<Item = TokenTree>,
) -> TokenStream {
    let mut tokens = tokens.into_iter().peekable();
    let mut output = vec![];
    while let Some(tree) = tokens.next() {
        match tree {
            TokenTree::Group(v) => {
                output.push(TokenTree::Group(Group::new(
                    v.delimiter(),
                    extract_ident_(ident, v.stream().into_iter()),
                )));
            }
            TokenTree::Punct(p)
                if p.as_char() == '@'
                    && let Some(next) = tokens.peek()
                    && let TokenTree::Ident(i) = next =>
            {
                ident.push(i.clone());
                output.push(tokens.next().unwrap());
            }
            _ => output.push(tree),
        }
    }
    output.into_iter().collect()
}

fn compile_error(span: Span, msg: &str) -> TokenStream {
    [
        TokenTree::Ident(Ident::new("compile_error", span)), //
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            [
                TokenTree::from(Literal::string(msg)), //
            ]
            .into_iter()
            .collect(),
        )),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]
    .into_iter()
    .map(|mut v| {
        v.set_span(span);
        v
    })
    .collect()
}
