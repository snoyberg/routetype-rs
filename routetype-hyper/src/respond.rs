use hyper::{Body, header::HeaderValue};
use anyhow::*;

use crate::DispatchOutput;

pub fn html<B: Into<Body>>(body: B) -> DispatchOutput {
    let mut res = hyper::Response::new(body);
    res.headers_mut().append(hyper::header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    res.into()
}

#[cfg(feature = "askama")]
pub fn askama<T: askama::Template>(t: T) -> Result<DispatchOutput> {
    t.render().map(html).context("Unable to render Askama template")
}

pub fn css<B: Into<Body>>(body: B) -> DispatchOutput {
    let mut res = hyper::Response::new(body);
    res.headers_mut().append(hyper::header::CONTENT_TYPE, HeaderValue::from_static("text/css; charset=utf-8"));
    res.into()
}

pub mod redirect {
    use std::convert::TryInto;
    use anyhow::*;
    use hyper::header::HeaderValue;

    pub fn temporary<T: TryInto<HeaderValue>>(dest: T) -> Result<crate::DispatchOutput>
    where
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        let mut res = hyper::Response::new(hyper::Body::empty());
        *res.status_mut() = hyper::StatusCode::TEMPORARY_REDIRECT;
        res.headers_mut().append(hyper::header::LOCATION, dest.try_into().context("Could not convert dest to header value")?);
        Ok(res.into())
    }
}
