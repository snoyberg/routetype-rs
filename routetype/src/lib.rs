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
    /// The type returned by the [Self::path] method.
    type PathIter: Iterator<Item = PathSegment<'static>>;

    /// The type returned by the [Self::query] method.
    type QueryIter: Iterator<Item = QueryPair<'static>>;

    /// Attempt to parse from the given path segments and query pairs.
    fn parse<'a, 'b>(path: impl Iterator<Item = PathSegment<'a>>, query: Option<impl Iterator<Item = QueryPair<'b>>>) -> Option<Self>;

    /// Produce an iterator with the path segments.
    fn path(&self) -> Self::PathIter;

    /// Produce an iterator with the query string pairs.
    fn query(&self) -> Option<Self::QueryIter>;

    /// Helper function that parses from a string instead of iterators.
    ///
    /// For details on the parsing of the underlying string, see [parse_path_and_query].
    fn parse_str(path_and_query: &str) -> Option<Self> {
        let (path, query) = parse_path_and_query(path_and_query);
        Self::parse(path, query)
    }

    /// Convenience method around [Self::path] which generates a `Vec`.
    fn path_vec(&self) -> Vec<PathSegment<'static>> {
        self.path().collect()
    }

    /// Convenience method around [Self::query] which generates a `Vec`.
    fn query_vec(&self) -> Option<Vec<QueryPair<'static>>> {
        self.query().map(|i| i.collect())
    }
}

/// A convenience type for unstructured route handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainRoute {
    pub path: Vec<String>,
    pub query: Option<Vec<(String, Option<String>)>>,
}

impl Route for PlainRoute {
    type PathIter = ();
    type QueryIter = ();

    fn parse<'a, 'b>(path: impl Iterator<Item = PathSegment<'a>>, query: Option<impl Iterator<Item = QueryPair<'b>>>) -> Option<Self> {
        Some(PlainRoute {
            path: path.map(Cow::into_owned).collect(),
            query: query.map(|q| q.map(|(k, v)| (k.into_owned(), v.map(Cow::into_owned))).collect())
        })
    }

    fn path(&self) -> Self::PathIter {
        self.path.iter()
    }

    fn query(&self) -> Option<Self::QueryIter> {
        self.query.map(|v| v.iter().map(|(k, v)| (Cow::Borrowed(k), None)))
    }
}
