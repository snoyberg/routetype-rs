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
    #[route("/goodbye/{}")]
    Goodbye(String),
    #[route("/?readiness")]
    Readiness,
    #[route("/?poll={}")]
    Poll(bool),
    #[route("/refresh?force=true")]
    Refresh,
}

#[test]
fn render_home() {
    assert_eq!(MyRoute::Home.render(), "/");
}

#[test]
fn render_style() {
    assert_eq!(MyRoute::Style.render(), "/style.css");
}

#[test]
fn render_hello() {
    assert_eq!(
        MyRoute::Hello {
            name: "alice".to_owned()
        }
        .render(),
        "/hello/alice"
    );
}

#[test]
fn parse_style() {
    assert_eq!(MyRoute::parse_str("/style.css?foo"), Ok(MyRoute::Style));
}

#[test]
fn parse_hello() {
    assert_eq!(
        MyRoute::parse_str("/hello/alice"),
        Ok(MyRoute::Hello {
            name: "alice".to_owned()
        })
    );
    assert_eq!(
        MyRoute::parse_str("/hello/alice/"),
        Err(RouteError::NormalizationFailed("/hello/alice".to_owned()))
    );
}

#[test]
fn foo() {
    assert_eq!(
        MyRoute::parse_str("foo?bar=42"),
        Ok(MyRoute::Foo { bar: 42 })
    );
    assert_eq!(
        MyRoute::parse_str("foo?bar=fortytwo"),
        Err(RouteError::NoMatch)
    );
    assert_eq!("/foo?bar=42", MyRoute::Foo { bar: 42 }.render());

    match MyRoute::parse_str("foo?bar=42").unwrap() {
        MyRoute::Foo { bar } => assert_eq!(bar, 42),
        _ => panic!(),
    }
}

#[test]
fn parse_invalid() {
    assert_eq!(
        MyRoute::parse_str("/does/not/exist"),
        Err(RouteError::NoMatch)
    );
}

#[test]
fn parse_normalize() {
    assert_eq!(
        MyRoute::parse_str("//foo/bar//baz///bin/"),
        Err(RouteError::NormalizationFailed(
            "/foo/bar/baz/bin".to_owned()
        ))
    );
    assert_eq!(
        MyRoute::Hello {
            name: "".to_owned()
        }
        .render(),
        "/hello/-"
    );
}
