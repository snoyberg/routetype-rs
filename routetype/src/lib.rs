mod either;
/// Helper functions for parsing the raw strings received over the wire.
pub mod raw;

pub use routetype_derive::*;
use std::{borrow::Cow, collections::HashMap};

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
    fn parse<'a, 'b>(
        path: impl Iterator<Item = PathSegment<'a>>,
        query: Option<impl Iterator<Item = QueryPair<'b>>>,
    ) -> Option<Self>;

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
                Some(ref query) => Some(query.iter().map(|(k, v)| {
                    (
                        k.as_ref(),
                        match v {
                            Some(v) => Some(v.as_ref()),
                            None => None,
                        },
                    )
                })),
            },
        )
    }
}

/// A trait for values which can be a part of the path segments or query string values.
pub trait RoutePiece: Sized {
    /// Attempt to parse a piece from a given string.
    fn parse_route_piece(s: &str) -> Option<Self>;

    /// Render this piece into a string.
    fn render_route_piece(&self) -> Cow<str>;
}

impl RoutePiece for String {
    fn parse_route_piece(s: &str) -> Option<Self> {
        Some(s.to_owned())
    }

    fn render_route_piece(&self) -> Cow<str> {
        Cow::Borrowed(self)
    }
}

impl RoutePiece for i32 {
    fn parse_route_piece(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    fn render_route_piece(&self) -> Cow<str> {
        self.to_string().into()
    }
}

impl RoutePiece for bool {
    fn parse_route_piece(s: &str) -> Option<Self> {
        match s {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    fn render_route_piece(&self) -> Cow<str> {
        Cow::Borrowed(match self {
            true => "true",
            false => "false",
        })
    }
}

/// A simplified view of query string parameters.
#[derive(Debug)]
pub struct QueryMap<'a> {
    map: HashMap<Cow<'a, str>, (usize, Vec<Cow<'a, str>>)>,
}

impl<'a> QueryMap<'a> {
    pub fn from_iter(query: Option<impl Iterator<Item = QueryPair<'a>>>) -> Self {
        let mut map = HashMap::new();
        let query = match query {
            None => return QueryMap { map },
            Some(query) => query
        };
        for (key, value) in query {
            let entry = map.entry(key).or_insert_with(|| (0, Vec::new()));
            match value {
                None => entry.0 += 1,
                Some(value) => entry.1.push(value),
            }
        }
        QueryMap { map }
    }

    pub fn get_single(&self, name: &str) -> Option<&str> {
        let (_, v) = self.map.get(name)?;
        if v.len() == 1 {
            Some(&v[0])
        } else {
            None
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

/// A convenience type for unstructured route handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainRoute {
    pub path: Vec<String>,
    pub query: Option<Vec<(String, Option<String>)>>,
}

impl Route for PlainRoute {
    fn parse<'a, 'b>(
        path: impl Iterator<Item = PathSegment<'a>>,
        query: Option<impl Iterator<Item = QueryPair<'b>>>,
    ) -> Option<Self> {
        Some(PlainRoute {
            path: path.map(Cow::into_owned).collect(),
            query: query.map(|q| {
                q.map(|(k, v)| (k.into_owned(), v.map(Cow::into_owned)))
                    .collect()
            }),
        })
    }

    fn path(&self) -> Vec<PathSegment> {
        self.path
            .iter()
            .map(|s| Cow::Borrowed(s.as_str()))
            .collect()
    }

    fn query(&self) -> Option<Vec<QueryPair>> {
        match self.query {
            None => None,
            Some(ref query) => Some(
                query
                    .iter()
                    .map(|(k, v)| {
                        (
                            Cow::Borrowed(k.as_ref()),
                            match v {
                                None => None,
                                Some(v) => Some(Cow::Borrowed(v.as_ref())),
                            },
                        )
                    })
                    .collect(),
            ),
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
