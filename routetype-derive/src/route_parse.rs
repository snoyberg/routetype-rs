use std::{convert::Infallible, str::FromStr};

use anyhow::*;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, TokenStreamExt};
use syn::{DeriveInput, Ident};

/// Represents the fields and attributes of a single user defined `enum` route type.
#[derive(Debug)]
pub struct Routes {
    /// Name of the data type
    ident: Ident,
    /// Each of the variants/routes
    routes: Vec<Route>,
}

impl Routes {
    /// Parse a `Routes` value from user supplied input.
    ///
    /// This should follow the principle of failing early, returning a helpful error message on any invalid input.
    pub fn parse(input: &DeriveInput) -> Result<Self> {
        let data = match &input.data {
            syn::Data::Enum(data) => data,
            _ => bail!("Derive macro can only be used on enums"),
        };

        Ok(Routes {
            ident: input.ident.clone(),
            routes: data
                .variants
                .iter()
                .map(Route::parse)
                .collect::<Result<_>>()?,
        })
    }

    /// Generate the full `impl Route` for this type
    pub fn gen_impl(&self) -> TokenStream {
        let ident = &self.ident;
        let path_arms = self.gen_path_arms();
        let query_arms = self.gen_query_arms();
        let parse_blocks = self.gen_parse_blocks();

        quote! {
            impl routetype::Route for #ident {
                fn parse<'a, 'b>(
                    path: impl Iterator<Item = routetype::PathSegment<'a>>,
                    query: Option<impl Iterator<Item = routetype::QueryPair<'b>>>,
                ) -> Result<Self, routetype::RouteError> {
                    // We should use a more efficient parsing tree approach like in Yesod
                    let (path, query) = routetype::normalize::Normalization::default().normalize_parse(path, query)
                        .map_err(routetype::RouteError::NormalizationFailed)?;
                    let query = routetype::QueryMap::from_query_iter(query);
                    #parse_blocks
                    Err(routetype::RouteError::NoMatch)
                }

                fn path(&self) -> Vec<routetype::PathSegment> {
                    let mut res = Vec::new();
                    match self {
                        #path_arms
                    };
                    routetype::normalize::Normalization::default().normalize_render_path(res)
                }

                fn query(&self) -> Option<Vec<routetype::QueryPair>> {
                    let mut res = Vec::new();
                    match self {
                        #query_arms
                    }
                    if res.is_empty() {
                        None
                    } else {
                        Some(res)
                    }
                }
            }
        }
    }

    /// Generate the match arms within the `path` method implementation
    fn gen_path_arms(&self) -> TokenStream {
        let mut res = TokenStream::new();
        for route in &self.routes {
            let pattern = route.gen_pattern();
            let path_stmts = route.path_arm_stmts();

            res.append_all(quote! { #pattern => { #path_stmts } });
        }
        res
    }

    /// Generate the match arms within the `query` method implementation
    fn gen_query_arms(&self) -> TokenStream {
        let mut res = TokenStream::new();
        for route in &self.routes {
            let pattern = route.gen_pattern();
            let query_stmts = route.query_arm_stmts();

            res.append_all(quote! { #pattern => { #query_stmts } });
        }
        res
    }

    /// Generate the individual parse blocks within the `parse` method implementation
    fn gen_parse_blocks(&self) -> TokenStream {
        let mut res = TokenStream::new();
        for route in &self.routes {
            route.gen_parse_block(&mut res);
        }
        res
    }
}

/// A single variant of a user defined route enum
#[derive(Debug)]
struct Route {
    /// Name of the variant
    ident: Ident,
    /// The definition of the route
    route_contents: RouteContents,
}

impl Route {
    /// Parse out information on this route from the variant, including the attributes included on it.
    fn parse(variant: &syn::Variant) -> Result<Self> {
        let ident = variant.ident.clone();
        let raw_route: String = raw_route_attr(&variant.attrs).with_context(|| {
            format!("route attribute is required, missing on variant {}", ident)
        })?;
        let variant_type = match &variant.fields {
            syn::Fields::Named(fields) => RouteContents::parse_named(&raw_route, fields),
            syn::Fields::Unnamed(fields) => RouteContents::parse_positional(&raw_route, fields),
            syn::Fields::Unit => RouteContents::parse_unit(&raw_route),
        }
        .with_context(|| format!("Parsing fields of route variant {}", ident))?;
        Ok(Route {
            ident,
            route_contents: variant_type,
        })
    }

    /// Generate the pattern match for this `Route`.
    ///
    /// This will handle the differences between unit, tuple, and field syntax and bind all fields to their derived local names.
    fn gen_pattern(&self) -> TokenStream {
        let ident = &self.ident;
        match &self.route_contents {
            RouteContents::Unit(_) => quote! { Self::#ident },
            RouteContents::Positional(pq) => {
                let patterns = pq.patterns();
                quote! { Self::#ident(#patterns) }
            }
            RouteContents::Named(pq) => {
                let patterns = pq.patterns();
                quote! { Self::#ident { #patterns } }
            }
        }
    }

    /// Generate the contents of the match arms of the `path` method.
    ///
    /// These statements will populate the `path` `Vec`.
    fn path_arm_stmts(&self) -> TokenStream {
        let mut ts = TokenStream::new();
        match &self.route_contents {
            RouteContents::Unit(pq) => pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts)),
            RouteContents::Positional(pq) => {
                pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts))
            }
            RouteContents::Named(pq) => pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts)),
        }
        ts
    }

    /// Generate the contents of the match arms of the `query` method.
    ///
    /// These statements will populate the `query` `Vec`.
    fn query_arm_stmts(&self) -> TokenStream {
        let mut ts = TokenStream::new();
        match &self.route_contents {
            RouteContents::Unit(pq) => pq.query.iter().for_each(|query| query.stmts(&mut ts)),
            RouteContents::Positional(pq) => pq.query.iter().for_each(|query| query.stmts(&mut ts)),
            RouteContents::Named(pq) => pq.query.iter().for_each(|query| query.stmts(&mut ts)),
        }
        ts
    }

    /// Generate the contents of the `parse` method
    fn gen_parse_block(&self, res: &mut TokenStream) {
        let (parse_path, parse_query, construct_route) =
            self.route_contents.gen_parse_pieces(&self.ident);
        res.append_all(quote! {
            if let Some(route) = (|| {
                let mut path = path.iter();
                #parse_path
                if path.next().is_some() { return None; }
                #parse_query
                Some(#construct_route)
            })() {
                return Ok(route);
            }
        })
    }
}

/// Extract the raw contents of the `#[route(...)]` attribute, if present and a string literal.
fn raw_route_attr(attrs: &[syn::Attribute]) -> Result<String> {
    for attr in attrs {
        if attr.path.is_ident("route") {
            return Ok(attr
                .parse_args::<syn::LitStr>()
                .context("route attribute must be a string literal")?
                .value());
        }
    }
    Err(anyhow!("Attribute named route not found"))
}

/// Parse out the information on the path segments.
///
/// This combines the path information from the `route` attr and the fields defined on the `enum`.
fn parse_path_fields<Field: AsField>(
    raw_path: &str,
    fields: &mut Vec<&syn::Field>,
) -> Result<Vec<Seg<Field>>> {
    let raw_path = raw_path.strip_prefix('/').unwrap_or(raw_path);
    if raw_path.is_empty() {
        return Ok(vec![]);
    }
    let mut counter = 0;
    raw_path
        .split('/')
        .map(|raw_seg| {
            let rv = RouteValue::parse(raw_seg, RouteValueType::Path, &mut counter)?;
            rv.remove_field(fields)?;
            Ok(Seg(rv))
        })
        .collect()
}

/// Same as `parse_path_fields` but for the query string.
fn parse_query_fields<Field: AsField>(
    raw_query: &str,
    fields: &mut Vec<&syn::Field>,
) -> Result<Vec<Query<Field>>> {
    if raw_query.is_empty() {
        bail!("Empty query string specified, please omit the question mark");
    }
    let mut counter = 0;
    raw_query
        .split('&')
        .map(|raw_pair| match raw_pair.find('=') {
            None => Ok(Query {
                key: raw_pair.to_owned(),
                value: None,
            }),
            Some(idx) => {
                let key = raw_pair[..idx].to_owned();
                let value = &raw_pair[idx + 1..];
                let value = RouteValue::parse(value, RouteValueType::Query, &mut counter)?;
                value.remove_field(fields)?;
                Ok(Query {
                    key,
                    value: Some(value),
                })
            }
        })
        .collect()
}

/// Ensure that the provided fields are empty, raising a descriptive error message otherwise.
fn require_fields_used(fields: Vec<&syn::Field>) -> Result<()> {
    if fields.is_empty() {
        Ok(())
    } else {
        let mut unused = Vec::new();
        for field in &fields {
            if let Some(ident) = field.ident.as_ref() {
                unused.push(ident.to_string());
            }
        }
        Err(anyhow!("Not all fields used: {:?}", unused))
    }
}

/// The contents of a single route.
///
/// This supports the three ways of specifying a variant and provides some type safety by inserting a type parameter into [PathAndQuery].
#[derive(Debug)]
enum RouteContents {
    /// Unit variant, e.g. `enum Route { Home }`. No fields are allowed, so we use [Infallible].
    Unit(PathAndQuery<Infallible>),
    /// Positional/tuple variant, e.g. `enum Route { Hello(String) }`. Fields are handled positionally, and we don't track that position, so we use a unit.
    Positional(PathAndQuery<()>),
    /// Named variant, e.g. `enum Route { Hello { name: String } }`. Fields are recognized by identifier.
    Named(PathAndQuery<Ident>),
}

impl RouteContents {
    fn parse_named(raw_route: &str, fields: &syn::FieldsNamed) -> Result<Self> {
        let fields: Vec<_> = fields.named.iter().collect();
        Ok(Self::Named(PathAndQuery::parse(raw_route, fields)?))
    }

    fn parse_positional(raw_route: &str, fields: &syn::FieldsUnnamed) -> Result<Self> {
        let fields: Vec<_> = fields.unnamed.iter().collect();
        Ok(Self::Positional(PathAndQuery::parse(raw_route, fields)?))
    }

    fn parse_unit(raw_route: &str) -> Result<Self> {
        Ok(Self::Unit(PathAndQuery::parse(raw_route, vec![])?))
    }

    /// parse the path, parse the query, construct the route
    fn gen_parse_pieces(&self, ident: &Ident) -> (TokenStream, TokenStream, TokenStream) {
        match self {
            RouteContents::Unit(pq) => pq.gen_parse_pieces(ident),
            RouteContents::Positional(pq) => pq.gen_parse_pieces(ident),
            RouteContents::Named(pq) => pq.gen_parse_pieces(ident),
        }
    }
}

/// The true contents of a single route, parameterized on `Field`.
///
/// See both [RouteContents] and [AsField] for the purpose of this type parameter.
#[derive(Debug)]
struct PathAndQuery<Field: AsField> {
    path: Vec<Seg<Field>>,
    query: Vec<Query<Field>>,
}

impl<Field: AsField> PathAndQuery<Field> {
    /// Parse the complete [PathAndQuery] based on the given route attribute and fields for the variant.
    fn parse(raw_route: &str, mut fields: Vec<&syn::Field>) -> Result<Self> {
        Ok(match raw_route.find('?') {
            None => {
                let path = parse_path_fields(raw_route, &mut fields)?;
                require_fields_used(fields)?;
                PathAndQuery {
                    path,
                    query: vec![],
                }
            }
            Some(idx) => {
                let raw_path = &raw_route[..idx];
                let raw_query = &raw_route[idx + 1..];
                let path = parse_path_fields(raw_path, &mut fields)?;
                let query = parse_query_fields(raw_query, &mut fields)?;
                require_fields_used(fields)?;
                PathAndQuery { path, query }
            }
        })
    }

    /// Generate the comma-separated contents of a pattern match for this route.
    ///
    /// Note that tuple and record variants will need to wrap this up with parens or braces, respectively.
    fn patterns(&self) -> TokenStream {
        let mut res = TokenStream::new();
        self.path.iter().for_each(|seg| seg.gen_pattern(&mut res));
        self.query
            .iter()
            .for_each(|query| query.gen_pattern(&mut res));
        res
    }

    /// parse the path, parse the query, construct the route
    fn gen_parse_pieces(&self, ident: &Ident) -> (TokenStream, TokenStream, TokenStream) {
        let mut parse_path = TokenStream::new();
        self.path
            .iter()
            .for_each(|seg| seg.gen_parse(&mut parse_path));

        let mut parse_query = TokenStream::new();
        self.query
            .iter()
            .for_each(|query| query.gen_parse(&mut parse_query));

        let mut construct = TokenStream::new();
        self.path
            .iter()
            .for_each(|seg| seg.construct(&mut construct));
        self.query
            .iter()
            .for_each(|query| query.construct(&mut construct));
        let construct_route = Field::wrap_construct(ident, &construct);
        (parse_path, parse_query, construct_route)
    }
}

/// A single value within either a path segment or a query string parameter
#[derive(Debug)]
enum RouteValue<Field> {
    /// Literal value, e.g. `/hello/` or `?foo=bar`.
    Literal(String),
    /// Field, e.g. `/hello/{name}` or `?page={}`
    Field { field: Field, local: Ident },
}

/// Where a route value comes from, used for nicer error messages and generated identifiers.
#[derive(Clone, Copy)]
enum RouteValueType {
    Path,
    Query,
}

impl RouteValueType {
    /// Generate the next anonymous identifier
    fn next_ident(&self, counter: &mut usize) -> Ident {
        *counter += 1;
        format_ident!(
            "_route_value_{}_{}",
            match self {
                RouteValueType::Path => "path",
                RouteValueType::Query => "query",
            },
            counter
        )
    }
}

/// Raw parse of a path segment or query string value
enum RouteValueRaw {
    Literal(String),
    Positional,
    Named(Ident),
}

impl FromStr for RouteValueRaw {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "{}" {
            Ok(RouteValueRaw::Positional)
        } else {
            match s.strip_prefix('{') {
                None => Ok(RouteValueRaw::Literal(s.to_owned())),
                Some(s1) => match s1.strip_suffix('}') {
                    Some(s2) => Ok(RouteValueRaw::Named(format_ident!("{}", s2))),
                    None => Err(anyhow!("Invalid route value {:?}", s)),
                },
            }
        }
    }
}

/// Types which can be treated as parameterized fields in a route.
///
/// This generalization allows us to handle unit, tuple, and named field variants in a type safe way. The alternative would be to use tricks like `Option<Ident>` and runtime checking.
///
/// Instead, we have runtime guarantees that, if you have an `Infallible`, it _must_ be part of a unit variant. Similarly, `()` is always positional, and `Ident` is always named fields.
///
/// See [PathAndQuery] for more information.
trait AsField: Sized {
    /// Produce a value from a positional field, if allowed.
    fn from_positional() -> Result<Self>;

    /// Produce a value from a named field, if allowed.
    fn from_named(ident: Ident) -> Result<Self>;

    /// Generate the necessary construction code for this one field
    fn construct(&self, local: &Ident, ts: &mut TokenStream);

    /// Generate pattern matching for this one field
    fn gen_pattern(&self, local: &Ident, ts: &mut TokenStream);

    /// Wrap up all of the constructed fields with appropriate wrapping for the given [Ident].
    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream;
}

/// Demonstrate the fact that some code can never be called.
fn absurd<T>(_: Infallible) -> T {
    unreachable!()
}

impl AsField for Infallible {
    fn from_positional() -> Result<Self> {
        Err(anyhow!("Unit variants may not have any interpolations"))
    }

    fn from_named(_ident: Ident) -> Result<Self> {
        Err(anyhow!("Unit variants may not have any interpolations"))
    }

    fn construct(&self, _local: &Ident, _ts: &mut TokenStream) {
        absurd(*self)
    }

    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream {
        assert!(contents.is_empty());
        quote! { Self::#ident }
    }

    fn gen_pattern(&self, _local: &Ident, _ts: &mut TokenStream) {
        absurd(*self)
    }
}

impl AsField for () {
    fn from_positional() -> Result<Self> {
        Ok(())
    }

    fn from_named(_ident: Ident) -> Result<Self> {
        Err(anyhow!("Cannot have named field for tuple variant"))
    }

    fn construct(&self, local: &Ident, ts: &mut TokenStream) {
        ts.append_all(quote! { #local, });
    }

    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream {
        quote! { Self::#ident(#contents) }
    }

    fn gen_pattern(&self, local: &Ident, ts: &mut TokenStream) {
        ts.append_all(quote! { #local, })
    }
}

impl AsField for Ident {
    fn from_positional() -> Result<Self> {
        Err(anyhow!("Cannot have positional field for named variant"))
    }

    fn from_named(ident: Ident) -> Result<Self> {
        Ok(ident)
    }

    fn construct(&self, local: &Ident, ts: &mut TokenStream) {
        ts.append_all(quote! { #self: #local, })
    }

    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream {
        quote! { Self::#ident { #contents } }
    }

    fn gen_pattern(&self, local: &Ident, ts: &mut TokenStream) {
        ts.append_all(quote! { #self: #local, })
    }
}

impl<Field: AsField> RouteValue<Field> {
    /// Parse a single route value from the given attribute contents.
    fn parse(raw: &str, typ: RouteValueType, counter: &mut usize) -> Result<Self> {
        let raw: RouteValueRaw = raw.parse()?;
        let field: Field = match raw {
            RouteValueRaw::Literal(l) => return Ok(RouteValue::Literal(l)),
            RouteValueRaw::Positional => Field::from_positional()?,
            RouteValueRaw::Named(name) => Field::from_named(name)?,
        };
        let local = typ.next_ident(counter);
        Ok(RouteValue::Field { field, local })
    }

    /// Remove this [RouteValue] from the fields, so that we can later detect missing fields.
    fn remove_field(&self, fields: &mut Vec<&syn::Field>) -> Result<()> {
        match self {
            RouteValue::Literal(_) => Ok(()),
            RouteValue::Field { .. } => {
                // Positional, just pop
                if fields.pop().is_none() {
                    Err(anyhow!("Too many pieces of route in positional variant"))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// A single segment of the path
#[derive(Debug)]
struct Seg<Field>(RouteValue<Field>);

impl<Field: AsField> Seg<Field> {
    /// Generate a statement for the `path` method to push this value
    fn path_arm_stmts(&self, ts: &mut TokenStream) {
        match &self.0 {
            RouteValue::Literal(s) => ts.append_all(quote! {
                res.push(std::borrow::Cow::Borrowed(#s));
            }),
            RouteValue::Field { local, .. } => ts.append_all(quote! {
                res.push(routetype::RoutePiece::render_route_piece(&*#local));
            }),
        }
    }

    /// Generate a part of a pattern match for this field, if it's not a literal
    fn gen_pattern(&self, ts: &mut TokenStream) {
        match &self.0 {
            RouteValue::Literal(_) => (),
            RouteValue::Field { field, local } => field.gen_pattern(local, ts),
        }
    }

    /// Generate parse code for this segmentj
    fn gen_parse(&self, ts: &mut TokenStream) {
        ts.append_all(match &self.0 {
            RouteValue::Literal(s) => {
                quote! {
                    if path.next()? != #s { return None }
                }
            }
            RouteValue::Field { local, .. } => {
                quote! {
                    let #local = routetype::RoutePiece::parse_route_piece(path.next()?)?;
                }
            }
        })
    }

    /// Call [AsField::construct] if not a literal.
    fn construct(&self, ts: &mut TokenStream) {
        match &self.0 {
            RouteValue::Literal(_) => (),
            RouteValue::Field { field, local } => field.construct(local, ts),
        }
    }
}

/// A single query string pair.
#[derive(Debug)]
struct Query<Field> {
    /// The key, always a literal string
    key: String,
    /// Value, [None] represents lack of a `=`
    value: Option<RouteValue<Field>>,
}

impl<Field: AsField> Query<Field> {
    /// Generate the statement for the `query` method.
    fn stmts(&self, ts: &mut TokenStream) {
        let key = &self.key;
        ts.append_all(match &self.value {
            None => quote! {
                res.push((std::borrow::Cow::Borrowed(#key), None));
            },
            Some(RouteValue::Literal(value)) => quote! {
                res.push((std::borrow::Cow::Borrowed(#key), Some(std::borrow::Cow::Borrowed(#value))));
            },
            Some(RouteValue::Field { local, .. }) => quote! {
                res.push((std::borrow::Cow::Borrowed(#key), Some(routetype::RoutePiece::render_route_piece(&*#local))));
            },
        })
    }

    /// Generate the statement for the `parse` method.
    fn gen_parse(&self, ts: &mut TokenStream) {
        let key = &self.key;
        ts.append_all(match &self.value {
            None => quote! {
                if !query.contains(#key) { return None }
            },
            Some(RouteValue::Literal(s)) => quote! {
                if query.get_single(#key)? != #s { return None }
            },
            Some(RouteValue::Field { local, .. }) => quote! {
                let #local = routetype::RoutePiece::parse_route_piece(query.get_single(#key)?)?;
            },
        });
    }

    /// Generate the pattern match if a non-literal and non-`None`.
    fn gen_pattern(&self, ts: &mut TokenStream) {
        match &self.value {
            None => (),
            Some(RouteValue::Literal(_)) => (),
            Some(RouteValue::Field { field, local }) => field.gen_pattern(local, ts),
        }
    }

    /// Generate the construction of this field if a non-literal and non-`None`.
    fn construct(&self, ts: &mut TokenStream) {
        match &self.value {
            None => (),
            Some(RouteValue::Literal(_)) => (),
            Some(RouteValue::Field { field, local }) => field.construct(local, ts),
        }
    }
}
