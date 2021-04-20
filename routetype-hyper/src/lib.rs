pub use anyhow::*;
pub use async_trait::async_trait;
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response,
};
pub use routetype::*;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};

pub mod respond;

pub struct DispatchInput<D: Dispatch> {
    pub app: Arc<D>,
    pub request: hyper::Request<hyper::Body>,
    pub remote: SocketAddr,
}

#[async_trait]
pub trait Dispatch: Sized + Send + Sync + 'static {
    type Route: Route;

    async fn dispatch(input: DispatchInput<Self>, route: Self::Route) -> Result<DispatchOutput>;

    async fn not_found(input: DispatchInput<Self>) -> Result<DispatchOutput> {
        default_not_found(input).await
    }

    fn into_server(self) -> DispatchServer<Self> {
        DispatchServer(self)
    }
}

pub async fn default_not_found<T: Dispatch>(_input: DispatchInput<T>) -> Result<DispatchOutput> {
    Ok(respond::html("<h1>File not found</h1>"))
}

pub struct DispatchOutput(pub hyper::Response<hyper::Body>);

impl<B: Into<hyper::Body>> From<hyper::Response<B>> for DispatchOutput {
    fn from(res: hyper::Response<B>) -> Self {
        DispatchOutput(res.map(|b| b.into()))
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
        Ok(DispatchOutput(res)) => res,
        Err(e) => {
            let mut res = respond::html(e.to_string()).0;
            *res.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
            res
        }
    };
    Ok(res)
}
