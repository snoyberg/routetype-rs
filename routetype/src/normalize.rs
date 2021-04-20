use super::{PathSegment, QueryPair};
use std::borrow::Cow;

/// How to normalize paths
///
/// For now, this is hard-coded to a single set of rules:
///
/// * Repeated slashes (e.g. `/foo//bar///baz`) are collapsed (e.g. `/foo/bar/baz`)
/// * Trailing slashes (e.g. `/foo/bar/`) are removed (e.g. `/foo/bar`)
///
/// Path normalization ensures that you have one canonical URL within your application.
/// The expected behavior is that, if normalization fails, your application will generate
/// a redirect to the normalized path.
///
/// In the future, this may be configurable.
#[derive(Clone, Debug)]
pub struct Normalization {
    rules: NormalizationRules,
}

impl Default for Normalization {
    fn default() -> Self {
        Normalization {
            rules: NormalizationRules::NoTrailing,
        }
    }
}

#[derive(Clone, Debug)]
enum NormalizationRules {
    NoTrailing,
}

fn drop_one(s: &mut Cow<str>) {
    s.to_mut().pop(); // FIXME inefficient
}

impl Normalization {
    /// Apply normalization rules for incoming route, either returning the unmodified path and query or the normalized version.
    pub fn normalize_parse<'a, 'b>(
        &self,
        path: impl Iterator<Item = PathSegment<'a>>,
        query: Option<impl Iterator<Item = QueryPair<'b>>>,
    ) -> Result<
        (
            Vec<PathSegment<'a>>,
            Option<impl Iterator<Item = QueryPair<'b>>>,
        ),
        String,
    > {
        let mut path = path.collect::<Vec<PathSegment<'a>>>();
        if path.contains(&Cow::Borrowed("")) {
            let path = path.iter().filter(|s| !s.is_empty()).map(|s| s.as_ref());
            // FIXME make this more elegant
            match query {
                None => {
                    return Err(super::raw::render_path_and_query(
                        path,
                        None::<std::iter::Empty<_>>,
                    ))
                }
                Some(query) => {
                    let query = query.collect::<Vec<_>>();
                    let query = query.iter().map(|(k, v)| {
                        (
                            k.as_ref(),
                            v.as_ref().map(|v| match v {
                                Cow::Borrowed(s) => *s,
                                Cow::Owned(s) => &s,
                            }),
                        )
                    });
                    return Err(super::raw::render_path_and_query(path, Some(query)));
                }
            }
        }
        path.iter_mut().for_each(|s| {
            if !s.contains(|c| c != '-') {
                assert!(!s.is_empty());
                drop_one(s);
            }
        });
        Ok((path, query))
    }

    /// Apply normalization rules for outgoing path segments
    pub fn normalize_render_path<'a>(
        &self,
        mut path: Vec<PathSegment<'a>>,
    ) -> Vec<PathSegment<'a>> {
        path.iter_mut().for_each(|seg| {
            if !seg.contains(|c| c != '-') {
                // It's only dashes. Let's handle a few simple cases to cut down on heap allocations
                match seg.len() {
                    0 => *seg = Cow::Borrowed("-"),
                    1 => *seg = Cow::Borrowed("--"),
                    2 => *seg = Cow::Borrowed("---"),
                    3 => *seg = Cow::Borrowed("----"),
                    _ => seg.to_mut().push('-'),
                }
            }
        });
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper(path: &[&'static str]) -> Result<Vec<&'static str>, String> {
        let q: Option<std::iter::Empty<_>> = None;
        let (p, q) =
            Normalization::default().normalize_parse(path.iter().copied().map(Cow::Borrowed), q)?;
        assert!(q.is_none());
        let p = p
            .into_iter()
            .map(|cow| match cow {
                Cow::Borrowed(s) => s,
                Cow::Owned(s) => panic!("should never get owned here: {}", s),
            })
            .collect();
        Ok(p)
    }

    #[test]
    fn basics() {
        assert_eq!(helper(&[]), Ok(vec![]));
        assert_eq!(helper(&[""]), Err("/".to_owned()));
        assert_eq!(helper(&["foo", "bar"]), Ok(vec!["foo", "bar"]));
        assert_eq!(helper(&["foo", "bar", ""]), Err("/foo/bar".to_owned()));
    }
}
