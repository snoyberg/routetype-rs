use routetype::*;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoringRoute;

impl Route for BoringRoute {
    fn parse<'a, 'b>(
        mut path: impl Iterator<Item = PathSegment<'a>>,
        _query: Option<impl Iterator<Item = QueryPair<'b>>>,
    ) -> Option<Self> {
        match path.next() {
            None => Some(Self),
            Some(_) => None,
        }
    }

    fn path(&self) -> Vec<PathSegment> {
        vec![]
    }

    fn query(&self) -> Option<Vec<QueryPair>> {
        None
    }
}

#[test]
fn boring_parse_render() {
    assert_eq!(BoringRoute::parse_str(""), Some(BoringRoute));
    assert_eq!(BoringRoute::parse_str("/"), Some(BoringRoute));
    assert_eq!(BoringRoute.path(), Vec::<Cow<str>>::new());
    assert_eq!(BoringRoute.query(), None);
}

#[test]
fn boring_parse_failure() {
    assert_eq!(BoringRoute::parse_str("hello"), None);
}
