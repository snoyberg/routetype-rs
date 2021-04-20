use anyhow::*;
use hyper::{header::HeaderValue, Body, Response};

pub fn html<B: Into<Body>>(body: B) -> Response<Body> {
    let mut res = hyper::Response::new(body.into());
    res.headers_mut().append(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    res
}

#[cfg(feature = "askama")]
pub fn askama<T: askama::Template>(t: T) -> Result<Response<Body>> {
    t.render()
        .map(html)
        .context("Unable to render Askama template")
}

pub fn css<B: Into<Body>>(body: B) -> Response<Body> {
    let mut res = hyper::Response::new(body.into());
    res.headers_mut().append(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("text/css; charset=utf-8"),
    );
    res
}

pub mod redirect {
    use anyhow::*;
    use hyper::header::HeaderValue;
    use std::convert::TryInto;

    pub fn temporary<T: TryInto<HeaderValue>>(dest: T) -> Result<hyper::Response<hyper::Body>>
    where
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        let mut res = hyper::Response::new(hyper::Body::empty());
        *res.status_mut() = hyper::StatusCode::TEMPORARY_REDIRECT;
        res.headers_mut().append(
            hyper::header::LOCATION,
            dest.try_into()
                .context("Could not convert dest to header value")?,
        );
        Ok(res)
    }
}
