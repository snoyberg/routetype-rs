mod route_parse;

use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let routes =
        route_parse::Routes::parse(&input).expect("Could not parse attributes for route deriving");
    proc_macro::TokenStream::from(routes.gen_impl())
}
