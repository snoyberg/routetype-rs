mod either;
/// Helper functions for parsing the raw strings received over the wire.
pub mod raw;

use std::borrow::Cow;
pub use routetype_derive::*;

use raw::*;

/// A single piece of a URL path, with percent decoding already applied.
///
/// For example, in the path `/foo/bar%2Fbaz`, you would have the path segments logically containing `["foo", "bar/baz"]`.
///
/// For more details, see [raw::parse_path].
pub type PathSegment<'a> = Cow<'a, str>;

/// A single key/value pair for the query string.
///
/// This type distinguishes between "no value provided" and "empty value provided".
///
/// For more details, see [raw::parse_query].
pub type QueryPair<'a> = (Cow<'a, str>, Option<Cow<'a, str>>);

/// A type which can be parsed from and rendered to an HTTP path and query string.
pub trait Route: Sized + Clone + Send + Sync + 'static {
    /// Attempt to parse from the given path segments and query pairs.
    fn parse<'a, 'b>(path: impl Iterator<Item = PathSegment<'a>>, query: Option<impl Iterator<Item = QueryPair<'b>>>) -> Option<Self>;

    /// Produce a `Vec` with the path segments.
    fn path(&self) -> Vec<PathSegment>;

    /// Produce a `Vec` with the query string pairs.
    fn query(&self) -> Option<Vec<QueryPair>>;

    /// Helper function that parses from a string instead of iterators.
    ///
    /// For details on the parsing of the underlying string, see [parse_path_and_query].
    fn parse_str(path_and_query: &str) -> Option<Self> {
        let (path, query) = parse_path_and_query(path_and_query);
        Self::parse(path, query)
    }

    /// Helper function that renders this value into a `String`.
    ///
    /// For details on the exact output format, see [render_path_and_query].
    fn render(&self) -> String {
        render_path_and_query(
            self.path().iter().map(|x| x.as_ref()),
            match self.query() {
                None => None,
                Some(ref query) =>
            Some(query.iter().map(|(k, v)|
                (k.as_ref(), match v {
                    Some(v) => Some(v.as_ref()),
                    None => None,
                })))
            }
        )
    }
}

/// A convenience type for unstructured route handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainRoute {
    pub path: Vec<String>,
    pub query: Option<Vec<(String, Option<String>)>>,
}

impl Route for PlainRoute {
    fn parse<'a, 'b>(path: impl Iterator<Item = PathSegment<'a>>, query: Option<impl Iterator<Item = QueryPair<'b>>>) -> Option<Self> {
        Some(PlainRoute {
            path: path.map(Cow::into_owned).collect(),
            query: query.map(|q| q.map(|(k, v)| (k.into_owned(), v.map(Cow::into_owned))).collect())
        })
    }

    fn path(&self) -> Vec<PathSegment> {
        self.path.iter().map(|s| Cow::Borrowed(s.as_str())).collect()
    }

    fn query(&self) -> Option<Vec<QueryPair>> {
        match self.query {
            None => None,
            Some(ref query) => Some(
                query.iter().map(|(k, v)| {
                    (Cow::Borrowed(k.as_ref()), match v {
                        None => None,
                        Some(v) => Some(Cow::Borrowed(v.as_ref())),
                    })
                }).collect()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::*;

    quickcheck! {
        fn prop_round_trip_plainroute(path: Vec<String>, query: Option<Vec<(String, Option<String>)>>) -> bool {
            let plainroute = PlainRoute { path, query };
            let rendered: String = plainroute.render();
            let parsed: PlainRoute = PlainRoute::parse_str(&rendered).unwrap();
            assert_eq!(plainroute, parsed);
            true
        }
    }
}
