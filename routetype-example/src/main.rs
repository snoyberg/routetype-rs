use routetype::*;

#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("style.css")]
    Style,
    #[route("hello/{name}")]
    Hello { name: String },
    #[route("foo?bar={bar}")]
    Foo { bar: i32 },
}

fn main() {
    println!("{:?}", MyRoute::parse_str("/foo?bar=32").unwrap());
}
