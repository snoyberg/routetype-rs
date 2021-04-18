use routetype::*;

#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("style.css")]
    Style,
    #[route("hello/{name}")]
    Hello {
        name: String
    },
    #[route("foo?bar={bar}")]
    Foo {
        bar: i32
    },
    #[route("/goodbye/{}")]
    Goodbye(String),
}

#[test]
fn render_home() {
    assert_eq!(MyRoute::Home.render(), "/");
}

#[test]
fn render_style() {
    assert_eq!(MyRoute::Style.render(), "/style%2Ecss"); // FIXME don't want to percent encoding periods
}

#[test]
fn render_hello() {
    assert_eq!(MyRoute::Hello { name: "alice".to_owned() }.render(), "/hello/alice");
}

#[test]
fn parse_style() {
    assert_eq!(MyRoute::parse_str("/style.css?foo"), Some(MyRoute::Style));
}

#[test]
fn parse_hello() {
    assert_eq!(MyRoute::parse_str("/hello/alice"), Some(MyRoute::Hello { name: "alice".to_owned() }));
    assert_eq!(MyRoute::parse_str("/hello/alice/"), None);
}

#[test]
fn foo() {
    assert_eq!(MyRoute::parse_str("foo?bar=42"), Some(MyRoute::Foo { bar: 42 }));
    assert_eq!(MyRoute::parse_str("foo?bar=fortytwo"), None);
    assert_eq!("/foo?bar=42", MyRoute::Foo { bar: 42 }.render());

    match MyRoute::parse_str("foo?bar=42").unwrap() {
        MyRoute::Foo { bar } => assert_eq!(bar, 42),
        _ => panic!()
    }
}

#[test]
fn parse_invalid() {
    assert_eq!(MyRoute::parse_str("/does/not/exist"), None);
}
