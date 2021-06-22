pub use anyhow::*;
pub use async_trait::async_trait;
pub use hyper::{
    header::{HeaderName, HeaderValue},
    Body, Request, Response, StatusCode,
};
use hyper::{server::conn::AddrStream, service::Service};
pub use routetype::*;
use std::{convert::Infallible, future::Future, net::SocketAddr, pin::Pin, sync::Arc};

#[cfg(feature = "askama")]
pub use askama::Template;

#[cfg(feature = "tonic")]
mod grpc;

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
        DispatchServer(Arc::new(self))
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

pub struct DispatchServer<T>(Arc<T>);

impl<T> Clone for DispatchServer<T> {
    fn clone(&self) -> Self {
        DispatchServer(self.0.clone())
    }
}

pub struct DispatchServerConn<T> {
    pub addr: SocketAddr,
    pub app: Arc<T>,
}

impl<T> Clone for DispatchServerConn<T> {
    fn clone(&self) -> Self {
        DispatchServerConn {
            addr: self.addr,
            app: self.app.clone(),
        }
    }
}

impl<T: Dispatch> DispatchServer<T> {
    pub async fn run(self, addr: impl Into<SocketAddr>) {
        let addr = addr.into();
        let server = hyper::Server::bind(&addr).serve(self);
        if let Err(e) = server.await {
            panic!("Hyper server exited with error: {}", e);
        }
    }

    pub fn get_arc(&self) -> Arc<T> {
        self.0.clone()
    }
}

impl<T> Service<&AddrStream> for DispatchServer<T> {
    type Response = DispatchServerConn<T>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: &AddrStream) -> Self::Future {
        std::future::ready(Ok(DispatchServerConn {
            addr: req.remote_addr(),
            app: self.0.clone(),
        }))
    }
}

impl<T: Dispatch> Service<Request<Body>> for DispatchServerConn<T> {
    type Response = Response<Body>;
    type Error = Infallible;
    #[allow(clippy::clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + 'static + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        Box::pin(helper(self.addr, self.app.clone(), req))
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
