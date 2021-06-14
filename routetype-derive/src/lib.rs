mod route_parse;

use syn::{parse_macro_input, DeriveInput};

/** Derive the `Route` trait for the given type.

Typically, this is used on an `enum` to provide a `Route` `impl`. You'll need to
provide `route` attributes on each individual variant, e.g.:

```ignore
#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("css/style.css")]
    Style,
    #[route("/hello/{name}")]
    Hello { name: String },
}
```

*/
#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let routes =
        route_parse::Routes::parse(&input).expect("Could not parse attributes for route deriving");
    proc_macro::TokenStream::from(routes.gen_impl())
}
