use std::str::FromStr;

use anyhow::*;
use proc_macro2::TokenStream;
use quote::{quote, TokenStreamExt};
use syn::{parse_macro_input, Attribute, DataEnum, DeriveInput, Fields, Variant};

#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    proc_macro::TokenStream::from(derive_route_inner(input).unwrap())
}

fn derive_route_inner(input: DeriveInput) -> Result<TokenStream> {
    let data = match input.data {
        syn::Data::Enum(data) => data,
        _ => bail!("#[derive(Route)] only supports enums"),
    };
    let name = input.ident;

    let path_arms = path_arms(&data).context("Constructing arms of the path method")?;
    let parse_blocks= parse_blocks(&data).context("Constructing blocks for parse method")?;

    Ok(quote! {
        impl routetype::Route for #name {
            fn parse<'a, 'b>(
                path: impl Iterator<Item = routetype::PathSegment<'a>>,
                query: Option<impl Iterator<Item = routetype::QueryPair<'b>>>,
            ) -> Option<Self> {
                use #name::*;
                // We should use a more efficient parsing tree approach like in Yesod
                let path = path.collect::<Vec<_>>();
                let query = routetype::QueryMap::from_iter(query);
                #parse_blocks
                None
            }

            fn path(&self) -> Vec<routetype::PathSegment> {
                use #name::*;
                let mut res = Vec::new();
                match self {
                    #path_arms
                };
                res
            }

            fn query(&self) -> Option<Vec<routetype::QueryPair>> {
                use #name::*;
                let mut res = Vec::new();
                // FIXME
                if res.is_empty() {
                    None
                } else {
                    Some(res)
                }
            }
        }
    })
}

fn path_arms(data: &DataEnum) -> Result<TokenStream> {
    let mut res = TokenStream::new();
    for v in data.variants.iter() {
        let ts = path_arm(v).context("Generating path() method arms")?;
        res.append_all(ts);
    }
    Ok(res)
}

fn path_arm(v: &Variant) -> Result<TokenStream> {
    let route = get_route(&v.ident, &v.attrs)?;
    let path_stmts = route.path_stmts();
    let ident = &v.ident;
    Ok(match &v.fields {
        Fields::Unit => {
            quote! { #ident => { #path_stmts } }
        }
        Fields::Named(fields) => {
            let field_patterns = field_patterns(fields)
                .with_context(|| format!("Generating field patterns for {}", ident))?;
            quote! { #ident{ #field_patterns } => { #path_stmts } }
        }
        Fields::Unnamed(_) => {
            bail!("Unsupported unnamed fields for variant {}", ident);
        }
    })
}

fn field_patterns(fields: &syn::FieldsNamed) -> Result<TokenStream> {
    let mut res = TokenStream::new();
    for field in &fields.named {
        let name = field.ident.as_ref().context("Found an unnamed field")?;
        res.append_all(quote! {
            #name,
        })
    }
    Ok(res)
}

fn get_route(ident: &syn::Ident, attrs: &[Attribute]) -> Result<Route> {
    get_route_inner(attrs).with_context(|| format!("Parsing route attribute for variant {}", ident))
}

fn get_route_inner(attrs: &[Attribute]) -> Result<Route> {
    for attr in attrs {
        if attr.path.is_ident("route") {
            return attr
                .parse_args::<syn::LitStr>()
                .context("route attribute must be a literal string")?
                .value()
                .parse()
                .context("Invalid content for route attribute");
        }
    }

    Err(anyhow!(
        "Did not found attribute named 'route', which is required."
    ))
}

struct Route {
    segs: Vec<Seg>,
    query: Vec<Query>,
}

impl FromStr for Route {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.find('?') {
            Some(idx) => Route {
                segs: parse_segs(&s[..idx])
                    .with_context(|| format!("Parsing segments in route {}", s))?,
                query: parse_query(&s[idx + 1..])
                    .with_context(|| format!("Parsing query string in route {}", s))?,
            },
            None => Route {
                segs: parse_segs(s).with_context(|| format!("Parsing segments in route {}", s))?,
                query: Vec::new(),
            },
        })
    }
}

fn parse_segs(s0: &str) -> Result<Vec<Seg>> {
    if s0.is_empty() || s0 == "/" {
        return Ok(Vec::new());
    }
    let s = if s0.as_bytes()[0] == b'/' {
        &s0[1..]
    } else {
        s0
    };

    Ok(s.split('/')
        .map(|seg| {
            seg.parse()
                .with_context(|| format!("Invalid segment while parsing route {}", s0))
        })
        .collect::<Result<Vec<Seg>>>()?)
}

impl FromStr for Seg {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .map(|s| Seg::Named(quote::format_ident!("{}", s)))
            .unwrap_or_else(|| Seg::Literal(s.to_owned())))
    }
}

enum Seg {
    Literal(String),
    Named(syn::Ident),
}

struct Query {
    key: String,
    ident: syn::Ident,
}

impl FromStr for Query {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.find('=') {
            None => Err(anyhow!("Query string component {:?} requires an equal sign", s)),
            Some(idx) => {
                let key = s[..idx].to_owned();
                let ident = quote::format_ident!("{}", &s[idx + 1..]);
                Ok(Query { key, ident })
            }
        }
    }
}

fn parse_query(s: &str) -> Result<Vec<Query>> {
    s.split('&').map(|s2| Query::from_str(s2).with_context(|| format!("Unable to parse query string {}", s2))).collect()
}

impl Route {
    fn path_stmts(&self) -> TokenStream {
        use Seg::*;
        let mut res = TokenStream::new();
        for seg in &self.segs {
            let ts = match seg {
                Literal(s) => {
                    quote! {
                        res.push(std::borrow::Cow::Borrowed(#s));
                    }
                }
                Named(name) => {
                    quote! {
                        res.push(routetype::RoutePiece::render_route_piece(&*#name));
                    }
                }
            };
            res.append_all(ts);
        }
        res
    }
}

fn parse_blocks(data: &DataEnum) -> Result<TokenStream> {
    let mut res = TokenStream::new();
    for v in &data.variants {
        let parse_variant = parse_variant(v).with_context(|| format!("Constructing parse variant code for {}", v.ident))?;
        res.append_all(quote! {
            if let Some(route) = (|| {
                #parse_variant
            })() {
                return Some(route);
            }
        })
    }
    Ok(res)
}

fn parse_variant(v: &Variant) -> Result<TokenStream> {
    let route = get_route(&v.ident, &v.attrs)?;
    let parse_path = parse_path(&route)?;
    let parse_query = parse_query2(&route)?;
    let construct_route= construct_route(v)?;
    Ok(quote! {
        let mut path = path.iter();
        #parse_path
        #parse_query
        if path.next().is_some() { return None; }
        Some(#construct_route)
    })
}

fn parse_path(r: &Route) -> Result<TokenStream> {
    let mut res = TokenStream::new();
    for seg in &r.segs {
        res.append_all(match seg {
            Seg::Literal(s) => {
                quote! {
                    if path.next()? != #s { return None }
                }
            }
            Seg::Named(name) => {
                quote! {
                    let #name = routetype::RoutePiece::parse_route_piece(path.next()?)?;
                }
            }
        })
    }
    Ok(res)
}

fn parse_query2(r: &Route) -> Result<TokenStream> {
    let mut res = TokenStream::new();
    for query in &r.query {
        let ident = &query.ident;
        let key = &query.key;
        res.append_all(quote! {
            let #ident = routetype::RoutePiece::parse_route_piece(query.get_single(#key)?)?;
        });
    }
    Ok(res)
}

fn construct_route(v: &Variant) -> Result<TokenStream> {
    let ident = &v.ident;
    Ok(match &v.fields {
        Fields::Unit => {
            quote! {
                #ident
            }
        }
        Fields::Named(fields) => {
            let mut ts = TokenStream::new();
            for field in &fields.named {
                let ident = field.ident.as_ref().context("Fields must be named")?;
                ts.append_all(quote! {
                    #ident,
                });
            }
            quote! {
                #ident { #ts }
            }
        }
        Fields::Unnamed(_) => {
            bail!("Route derive macro does not support unnamed fields for variant {}", ident);
        }
    })
}
