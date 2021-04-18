mod route_parse;
use route_parse::*;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let routes =
        route_parse::Routes::parse(&input).expect("Could not parse attributes for route deriving");
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
