use super::either::Either;
use std::borrow::Cow;
use super::{PathSegment, QueryPair};

/** Parse a string containing both a path and query string.

This function relies on the underlying behavior of [parse_path] and [parse_query]. Please see those functions for details.

Importantly here, this function will return `None` for the query component if there is no question mark in this strict.

```rust
# use routetype::raw::parse_path_and_query;
assert!(parse_path_and_query("/foo/bar").1.is_none());
assert!(parse_path_and_query("/foo/bar?").1.is_some());
```

*/
pub fn parse_path_and_query(path_and_query: &str) -> (impl Iterator<Item = PathSegment>, Option<impl Iterator<Item = QueryPair>>) {
    match path_and_query.find('?') {
        None => (parse_path(path_and_query), None),
        Some(idx) => {
            let path = &path_and_query[..idx];
            let query = &path_and_query[idx + 1..];
            (parse_path(path), Some(parse_query(query)))
        }
    }
}

fn decode(s: &str) -> Cow<str> {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy()
}

/** Parse just the path portion (i.e., everything before the question mark).

This function will accept and ignore a leading forward slash.

```rust
# use routetype::raw::parse_path;
# use routetype::PathSegment;
let segments: Vec<PathSegment> = parse_path("/foo").collect();
assert_eq!(segments, vec!["foo"]);
```

Trailing slashes and repeated slashes produce empty segments:

```rust
# use routetype::raw::parse_path;
# use routetype::PathSegment;
let segments: Vec<PathSegment> = parse_path("foo//bar/").collect();
assert_eq!(segments, vec!["foo", "", "bar", ""]);
```
*/
pub fn parse_path(mut path: &str) -> impl Iterator<Item = PathSegment> {
    if path.bytes().next() == Some(b'/') {
        path = &path[1..];
    }
    if path.is_empty() {
        Either::Left(std::iter::empty())
    } else {
        Either::Right(path.split('/').map(decode))
    }
}

/** Parse the query string component into pairs.

This function assumes that any leading question mark has already been stripped off. If you provide a question mark, it will be treated as part of the first query pair key.

```rust
# use routetype::raw::parse_query;
# use routetype::QueryPair;
# use std::borrow::Cow;
let pairs: Vec<QueryPair> = parse_query("?key=value").collect();
assert_eq!(pairs, vec![(Cow::Borrowed("?key"), Some(Cow::Borrowed("value")))]);
```

This function distinguishes between two similar situations:

* No value is provided, e.g. `?foo`
* A value is provided but empty, e.g. `?foo=`

```rust
# use routetype::raw::parse_query;
# use routetype::QueryPair;
# use std::borrow::Cow;

let pairs: Vec<QueryPair> = parse_query("key").collect();
assert!(pairs[0].1.is_none());
let pairs: Vec<QueryPair> = parse_query("key=").collect();
assert!(pairs[0].1.is_some());
let pairs: Vec<QueryPair> = parse_query("key=value").collect();
assert_eq!(pairs[0].1, Some(Cow::Borrowed("value")));
```

*/
pub fn parse_query(query: &str) -> impl Iterator<Item = QueryPair> {
    if query.is_empty() {
        Either::Left(std::iter::empty())
    } else {
        Either::Right(query.split('&').map(parse_query_pair))
    }
}

fn parse_query_pair(pair: &str) -> QueryPair {
    match pair.find('=') {
        None => (decode(pair), None),
        Some(idx) => {
            let key = &pair[..idx];
            let value = &pair[idx + 1..];
            (decode(key), Some(decode(value)))
        }
    }
}

/** Render path segments and query string pairs into a `String`.

This function will always prepend with a leading forward slash:

```rust
# use routetype::raw::render_path_and_query;
let path: Vec<&str> = vec![];
let query: Option<std::iter::Empty<_>> = None;
assert_eq!(render_path_and_query(path.iter().copied(), query), "/");

// FIXME consider if this behavior is desired or not
let path: Vec<&str> = vec![""];
let query: Option<std::iter::Empty<_>> = None;
assert_eq!(render_path_and_query(path.iter().copied(), query), "/");

let path: Vec<&str> = vec!["hello", "world"];
let query: Option<std::iter::Empty<_>> = None;
assert_eq!(render_path_and_query(path.iter().copied(), query), "/hello/world");
```

It will only include a question mark if a query string is provided.

```rust
# use routetype::raw::render_path_and_query;
let path: Vec<&str> = vec!["hello", "world"];
let query: Option<std::iter::Empty<_>> = Some(std::iter::empty());
assert_eq!(render_path_and_query(path.iter().copied(), query), "/hello/world?");
```

And similarly, it will only emit equal signs if a value is present.

```rust
# use routetype::raw::render_path_and_query;
let path: Vec<&str> = vec![];
let query: Vec<(&str, Option<&str>)> = vec![
    ("foo", None),
    ("bar", Some("")),
    ("baz", Some("bin")),
];
assert_eq!(render_path_and_query(path.iter().copied(), Some(query.into_iter())), "/?foo&bar=&baz=bin");
```

Percent encoding is handled correctly.

```rust
# use routetype::raw::render_path_and_query;
let path: Vec<&str> = vec![
    "hello",
    "שלום",
    "wor/ld"
];
let query: Vec<(&str, Option<&str>)> = vec![
    ("he?llo", Some("there#")),
];
assert_eq!(
    render_path_and_query(path.into_iter(), Some(query.into_iter())),
    "/hello/%D7%A9%D7%9C%D7%95%D7%9D/wor%2Fld?he?llo=there%23",
);
```
*/
pub fn render_path_and_query<'a, 'b, Path, Query>(path: Path, query: Option<Query>) -> String
where
    Path: Iterator<Item = &'a str>,
    Query: Iterator<Item = (&'b str, Option<&'b str>)>
{
    use percent_encoding::{AsciiSet, CONTROLS};

    fn encode_append(res: &mut String, s: &str, set: &'static AsciiSet) {
        for s in percent_encoding::utf8_percent_encode(s, set) {
            *res += s;
        }
    }

    // https://url.spec.whatwg.org/#query-percent-encode-set
    const QUERY_SET: AsciiSet = CONTROLS.add(b' ').add(b'"').add(b'#').add(b'<').add(b'>');
    const PATH_SET: AsciiSet = QUERY_SET.add(b'?').add(b'`').add(b'{').add(b'}').add(b'/');

    let mut res = String::new();
    for segment in path {
        res.push('/');
        encode_append(&mut res, segment, &PATH_SET);
    }
    if res.is_empty() {
        res.push('/');
    }

    if let Some(query) = query {
        res.push('?');
        let mut first = true;

        for (key, value) in query {
            if first {
                first = false;
            } else {
                res.push('&');
            }

            encode_append(&mut res, key, &QUERY_SET);
            if let Some(value) = value {
                res.push('=');
                encode_append(&mut res, value, &QUERY_SET);
            }
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pq(s: &str) -> (Vec<String>, Option<Vec<(String, Option<String>)>>) {
        let (path, query) = parse_path_and_query(s);
        let path = path.map(|x| x.into_owned()).collect();
        let query = query.map(|query| query.map(|(x, y)| (x.into_owned(), y.map(|y| y.into_owned()))).collect());
        (path, query)
    }

    #[test]
    fn pq_empty() {
        assert_eq!(pq(""), (vec![], None));
    }

    #[test]
    fn pq_slash() {
        assert_eq!(pq("/"), (vec![], None));
    }

    #[test]
    fn pq_question() {
        assert_eq!(pq("?"), (vec![], Some(vec![])));
    }

    #[test]
    fn pq_slash_question() {
        assert_eq!(pq("/?"), (vec![], Some(vec![])));
    }

    fn make_path(x: &[&str]) -> Vec<String> {
        x.iter().copied().map(|s| s.to_owned()).collect()
    }

    fn make_query(x: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        x.iter().copied().map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned()))).collect()
    }

    #[test]
    fn plain_pieces() {
        assert_eq!(pq("/foo/bar/baz"), (make_path(&["foo", "bar", "baz"]), None));
    }

    #[test]
    fn escaped_pieces() {
        assert_eq!(pq("/foo%2Fbar/baz"), (make_path(&["foo/bar", "baz"]), None));
        assert_eq!(pq("/foo%2fbar/baz"), (make_path(&["foo/bar", "baz"]), None));
    }

    #[test]
    fn query_values_missing() {
        assert_eq!(pq("?foo&bar=&baz=bin"), (vec![], Some(make_query(&[
            ("foo", None),
            ("bar", Some("")),
            ("baz", Some("bin")),
        ]))))
    }

    #[test]
    fn question_in_query() {
        assert_eq!(pq("/foo/?bar=baz?"), (make_path(&["foo", ""]), Some(make_query(&[("bar", Some("baz?"))]))))
    }
}
