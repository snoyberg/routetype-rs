use std::{convert::{Infallible, TryFrom, TryInto}, str::FromStr};

use anyhow::*;
use proc_macro2::{Punct, TokenStream};
use quote::{format_ident, quote, TokenStreamExt};
use syn::{DeriveInput, Ident, token::Token};

#[derive(Debug)]
pub struct Routes {
    pub ident: Ident,
    pub routes: Vec<Route>,
}

impl Routes {
    pub fn gen_path_arms(&self) -> TokenStream {
        let mut res = TokenStream::new();
        for route in &self.routes {
            let pattern = route.gen_pattern();
            let path_stmts = route.path_arm_stmts();

            res.append_all(quote! { #pattern => { #path_stmts } });
        }
        res
    }

    pub fn gen_parse_blocks(&self) -> TokenStream {
        let mut res = TokenStream::new();
        for route in &self.routes {
            route.gen_parse_block(&mut res);
        }
        res
    }
}

impl Routes {
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
}

#[derive(Debug)]
pub struct Route {
    pub ident: Ident,
    pub variant_type: VariantType,
}

impl Route {
    fn parse(variant: &syn::Variant) -> Result<Self> {
        let ident = variant.ident.clone();
        let raw_route: String = raw_route_attr(&variant.attrs).with_context(|| {
            format!("route attribute is required, missing on variant {}", ident)
        })?;
        let variant_type = match &variant.fields {
            syn::Fields::Named(fields) => VariantType::parse_named(&raw_route, fields),
            syn::Fields::Unnamed(fields) => VariantType::parse_positional(&raw_route, fields),
            syn::Fields::Unit => VariantType::parse_unit(&raw_route),
        }
        .with_context(|| format!("Parsing fields of route variant {}", ident))?;
        Ok(Route {
            ident,
            variant_type,
        })
    }

    pub fn gen_pattern(&self) -> TokenStream {
        let ident = &self.ident;
        match &self.variant_type {
            VariantType::Unit(_) => quote! { Self::#ident },
            VariantType::Positional(pq) => {
                let patterns = pq.patterns();
                quote! { Self::#ident(#patterns) }
            },
            VariantType::Named(pq) => {
                let patterns = pq.patterns();
                quote! { Self::#ident { #patterns } }
            },
        }
        /*
        match self.variant_type {
            VariantType::Tuple => {
                let mut fields= TokenStream::new();
                for seg in &self.segs {
                    match &seg.0 {
                        RouteValue::Literal(_) => (),
                        RouteValue::Field { field, local } => {
                            assert!(field.is_none());
                            fields.append_all(quote! { #local, });
                        },
                    }
                }
                for query in &self.query {
                    match &query.value {
                        None => (),
                        Some(RouteValue::Literal(_)) => (),
                        Some(RouteValue::Field { field, local }) => {
                            assert!(field.is_none());
                            fields.append_all(quote! { #local, });
                            /*
                            let field = field.as_ref().expect("Named/unnamed logic mismatch");
                            fields.append_all( quote! { #field: #local });
                            */
                        }
                    }
                }
                quote!{ Self::#ident(#fields) }
            }
            VariantType::Named => {
                let mut fields= TokenStream::new();
                quote!{ Self::#ident { #fields }}
            }
        }
        */
    }

    pub fn path_arm_stmts(&self) -> TokenStream {
        let mut ts = TokenStream::new();
        match &self.variant_type {
            VariantType::Unit(pq) => pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts)),
            VariantType::Positional(pq) => pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts)),
            VariantType::Named(pq) => pq.path.iter().for_each(|seg| seg.path_arm_stmts(&mut ts)),
        }
        ts
    }

    fn gen_parse_block(&self, res: &mut TokenStream) {
        let (parse_path, parse_query, construct_route) = self.variant_type.gen_parse_pieces(&self.ident);
        res.append_all(quote! {
            if let Some(route) = (|| {
                let mut path = path.iter();
                #parse_path
                if path.next().is_some() { return None; }
                #parse_query
                Some(#construct_route)
            })() {
                return Some(route);
            }
        })
    }
}

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

fn parse_path_fields<Field: AsField>(
    raw_path: &str,
    fields: &mut Vec<&syn::Field>,
) -> Result<Vec<Seg<Field>>> {
    let raw_path = raw_path.strip_prefix('/').unwrap_or(raw_path);
    if raw_path.is_empty() {
        return Ok(vec![]);
    }
    let mut counter = 0;
    raw_path.split('/').map(|raw_seg| {
        let rv = RouteValue::parse(raw_seg, RouteValueType::Path, &mut counter)?;
        rv.remove_field(fields)?;
        Ok(Seg(rv))
    }).collect()
}

fn parse_query_fields<Field: AsField>(
    raw_query: &str,
    mut fields: Vec<&syn::Field>,
) -> Result<Vec<Query<Field>>> {
    if raw_query.is_empty() {
        bail!("Empty query string specified, please omit the question mark");
    }
    let mut counter = 0;
    let res: Result<_> = raw_query.split('&').map(|raw_pair| {
        match raw_pair.find('=') {
            None => Ok(Query {
                key: raw_pair.to_owned(),
                value: None,
            }),
            Some(idx) => {
                let key = raw_pair[..idx].to_owned();
                let value = &raw_pair[idx + 1..];
                let value = RouteValue::parse(value, RouteValueType::Query, &mut counter)?;
                value.remove_field(&mut fields)?;
                Ok(Query {
                    key,
                    value: Some(value),
                })
            }
        }
    }).collect();
    if res.is_ok() && !fields.is_empty() {
        let mut unused = Vec::new();
        for field in &fields {
            if let Some(ident) = field.ident.as_ref() {
                unused.push(ident.to_string());
            }
        }
        bail!("Not all fields used: {:?}", unused);
    }
    res
}

#[derive(Debug)]
pub enum VariantType {
    Unit(PathAndQuery<Infallible>),
    Positional(PathAndQuery<()>),
    Named(PathAndQuery<Ident>),
}

impl VariantType {
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
            VariantType::Unit(pq) => pq.gen_parse_pieces(ident),
            VariantType::Positional(pq) => pq.gen_parse_pieces(ident),
            VariantType::Named(pq) => pq.gen_parse_pieces(ident),
        }
    }
}

#[derive(Debug)]
pub struct PathAndQuery<Field> {
    pub path: Vec<Seg<Field>>,
    pub query: Vec<Query<Field>>,
}

impl<Field: AsField> PathAndQuery<Field> {
    fn parse(raw_route: &str, mut fields: Vec<&syn::Field>) -> Result<Self> {
        Ok(match raw_route.find('?') {
            None => {
                let path = parse_path_fields(raw_route, &mut fields)?;
                if !fields.is_empty() {
                    use quote::ToTokens;
                    let mut ts = proc_macro2::TokenStream::new();
                    fields.iter().for_each(|f| f.to_tokens(&mut ts));
                    bail!(
                        "route attribute does not specify all fields in path-only attribute {:?}",
                        ts
                    );
                }
                PathAndQuery {
                    path,
                    query: vec![],
                }
            }
            Some(idx) => {
                let raw_path = &raw_route[..idx];
                let raw_query = &raw_route[idx + 1..];
                let path = parse_path_fields(raw_path, &mut fields)?;
                let query = parse_query_fields(raw_query, fields)?;
                PathAndQuery { path, query }
            }
        })
    }

    fn patterns(&self) -> TokenStream {
        let mut res = TokenStream::new();
        self.path.iter().for_each(|seg| seg.gen_pattern(&mut res));
        self.query.iter().for_each(|query| query.gen_pattern(&mut res));
        res
    }

    /// parse the path, parse the query, construct the route
    fn gen_parse_pieces(&self, ident: &Ident) -> (TokenStream, TokenStream, TokenStream) {
        let mut parse_path = TokenStream::new();
        self.path.iter().for_each(|seg| seg.gen_parse(&mut parse_path));

        let mut parse_query = TokenStream::new();
        self.query.iter().for_each(|query| query.gen_parse(&mut parse_query));

        let mut construct = TokenStream::new();
        self.path.iter().for_each(|seg| seg.construct(&mut construct));
        self.query.iter().for_each(|query| query.construct(&mut construct));
        let construct_route = Field::wrap_construct(ident, &construct);
        (parse_path, parse_query, construct_route)
    }
}

#[derive(Debug)]
pub enum RouteValue<Field> {
    Literal(String),
    Field { field: Field, local: Ident },
}

#[derive(Clone, Copy)]
enum RouteValueType {
    Path,
    Query,
}

impl RouteValueType {
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
                    None => Err(anyhow!("Invalid route value {:?}", s))
                }
            }
        }
    }
}

pub trait AsField: Sized {
    fn from_positional() -> Result<Self>;
    fn from_named(ident: Ident) -> Result<Self>;

    fn construct(&self, local: &Ident, ts: &mut TokenStream);
    fn gen_pattern(&self, local: &Ident, ts: &mut TokenStream);

    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream;
}

impl AsField for Infallible {
    fn from_positional() -> Result<Self> {
        Err(anyhow!("Unit variants may not have any interpolations"))
    }

    fn from_named(_ident: Ident) -> Result<Self> {
        Err(anyhow!("Unit variants may not have any interpolations"))
    }

    fn construct(&self, local: &Ident, ts: &mut TokenStream) {
        panic!("Impossible! construct called on Infallible");
    }

    fn wrap_construct(ident: &Ident, contents: &TokenStream) -> TokenStream {
        quote! { Self::#ident }
    }

    fn gen_pattern(&self, local: &Ident, ts: &mut TokenStream) {
        panic!("Impossible! gen_pattern called on Infallible");
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

/*
impl RouteValue<Ident> {
    fn remove_field(&self, fields: &mut Vec<&syn::Field>) -> Result<()> {
        match self {
            RouteValue::Literal(_) => Ok(()),
            RouteValue::Field { field: ident, .. } => {
                for idx in 0..fields.len() {
                    let field = &fields[idx];
                    if field
                        .ident
                        .as_ref()
                        .context("Used named route values on positional variant")?
                        == ident
                    {
                        fields.remove(idx);
                        return Ok(());
                    }
                }
                Err(anyhow!("Field named {} not found", ident))
            }
        }
    }
}
*/

#[derive(Debug)]
pub struct Seg<Field>(RouteValue<Field>);

impl<Field: AsField> Seg<Field> {
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

    fn gen_pattern(&self, ts: &mut TokenStream) {
        match &self.0 {
            RouteValue::Literal(_) => (),
            RouteValue::Field { field, local } => field.gen_pattern(local, ts),
        }
    }

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

    fn construct(&self, ts: &mut TokenStream) {
        match &self.0 {
            RouteValue::Literal(_) => (),
            RouteValue::Field { field, local } => field.construct(local, ts),
        }
    }
}

#[derive(Debug)]
pub struct Query<Field> {
    pub key: String,
    pub value: Option<RouteValue<Field>>,
}

impl<Field: AsField> Query<Field> {
    fn gen_parse(&self, ts: &mut TokenStream) {
        let key = &self.key;
        ts.append_all(match &self.value {
            None => quote! {
                if !quote.contains(#key) { return None }
            },
            Some(RouteValue::Literal(s)) => quote! {
                if query.get_single(#key)? != #s { return None }
            },
            Some(RouteValue::Field { local, .. }) => quote! {
                println!("looking up a value {:?}", query.get_single(#key));
                let #local = routetype::RoutePiece::parse_route_piece(query.get_single(#key)?)?;
            }
        });
    }

    fn gen_pattern(&self, ts: &mut TokenStream) {
        match &self.value {
            None => (),
            Some(RouteValue::Literal(_)) => (),
            Some(RouteValue::Field { field, local }) => field.gen_pattern(local, ts),
        }
    }

    fn construct(&self, ts: &mut TokenStream) {
        match &self.value {
            None => (),
            Some(RouteValue::Literal(_)) => (),
            Some(RouteValue::Field { field, local }) => field.construct(local, ts),
        }
    }
}
