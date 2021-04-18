mod route_parse;
use route_parse::*;

use std::str::FromStr;

// FIXME dedicated parsing step that figures out identifiers et al

use anyhow::*;
use proc_macro2::TokenStream;
use quote::{quote, TokenStreamExt};
use syn::{parse_macro_input, Attribute, DataEnum, DeriveInput, Fields, Variant};

#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let routes = route_parse::Routes::parse(&input).expect("Could not parse attributes for route deriving");
    proc_macro::TokenStream::from(derive_route_inner(&routes))
}

fn derive_route_inner(routes: &Routes) -> TokenStream {
    let ident = &routes.ident;
    let path_arms = routes.gen_path_arms();
    let query_arms = routes.gen_query_arms();
    let parse_blocks = routes.gen_parse_blocks();

    quote! {
        impl routetype::Route for #ident {
            fn parse<'a, 'b>(
                path: impl Iterator<Item = routetype::PathSegment<'a>>,
                query: Option<impl Iterator<Item = routetype::QueryPair<'b>>>,
            ) -> Option<Self> {
                // We should use a more efficient parsing tree approach like in Yesod
                let path = path.collect::<Vec<_>>();
                let query = routetype::QueryMap::from_iter(query);
                #parse_blocks
                None
            }

            fn path(&self) -> Vec<routetype::PathSegment> {
                let mut res = Vec::new();
                match self {
                    #path_arms
                };
                res
            }

            fn query(&self) -> Option<Vec<routetype::QueryPair>> {
                let mut res = Vec::new();
                match self {
                    #query_arms
                }
                if res.is_empty() {
                    None
                } else {
                    Some(res)
                }
            }
        }
    }
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

fn parse_query2(r: &Route) -> Result<TokenStream> {
    panic!("there5")
    /*
    let mut res = TokenStream::new();
    for query in &r.query {
        let ident = &query.ident;
        let key = &query.key;
        res.append_all(quote! {
            let #ident = routetype::RoutePiece::parse_route_piece(query.get_single(#key)?)?;
        });
    }
    Ok(res)
    */
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
