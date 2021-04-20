pub use anyhow::*;
pub use async_trait::async_trait;
pub use hyper::{
    header::{HeaderName, HeaderValue},
    Body, Request, Response, StatusCode,
};
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
};
pub use routetype::*;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};

#[cfg(feature = "askama")]
pub use askama::Template;

pub mod respond;

pub struct DispatchInput<D: Dispatch> {
    pub app: Arc<D>,
    pub request: hyper::Request<hyper::Body>,
    pub remote: SocketAddr,
}

#[async_trait]
pub trait Dispatch: Sized + Send + Sync + 'static {
    type Route: Route;

    async fn dispatch(input: DispatchInput<Self>, route: Self::Route) -> Result<Response<Body>>;

    async fn not_found(_input: DispatchInput<Self>) -> Result<Response<Body>> {
        Ok(default_not_found())
    }

    fn into_server(self) -> DispatchServer<Self> {
        DispatchServer(self)
    }
}

pub fn default_not_found() -> Response<Body> {
    respond::html("<h1>File not found</h1>")
}

pub trait DispatchOutput: Sized {
    fn into_response(self) -> Result<Response<Body>>;
}

impl<B: Into<hyper::Body>> DispatchOutput for hyper::Response<B> {
    fn into_response(self) -> Result<Response<Body>> {
        Ok(self.map(|b| b.into()))
    }
}

impl<B: Into<hyper::Body>> DispatchOutput for Result<hyper::Response<B>> {
    fn into_response(self) -> Result<Response<Body>> {
        self.map(|res| res.map(|b| b.into()))
    }
}

pub struct DispatchServer<T>(T);

impl<T: Dispatch> DispatchServer<T> {
    pub async fn run(self, addr: impl Into<SocketAddr>) {
        let addr = addr.into();
        let app = Arc::new(self.0);
        let server = hyper::Server::bind(&addr).serve(make_service_fn(move |conn: &AddrStream| {
            let addr = conn.remote_addr();
            let app = app.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    helper(addr, app.clone(), req)
                }))
            }
        }));
        if let Err(e) = server.await {
            panic!("Hyper server exited with error: {}", e);
        }
    }
}

async fn helper<T: Dispatch>(
    remote: SocketAddr,
    app: Arc<T>,
    request: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    let route = T::Route::parse_str(
        request
            .uri()
            .path_and_query()
            .expect("path_and_query cannot be None")
            .as_str(),
    );
    let input = DispatchInput {
        app,
        request,
        remote,
    };
    let output = match route {
        Err(RouteError::NoMatch) => T::not_found(input).await,
        Err(RouteError::NormalizationFailed(dest)) => respond::redirect::temporary(dest),
        Ok(route) => T::dispatch(input, route).await,
    };
    let res = match output {
        Ok(res) => res,
        Err(e) => {
            let mut res = respond::html(e.to_string());
            *res.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
            res
        }
    };
    Ok(res)
}
