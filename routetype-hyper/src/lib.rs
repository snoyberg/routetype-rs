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

#[cfg(feature = "tokio-rustls")]
pub mod tls;

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
    pub async fn run(self, addr: impl Into<SocketAddr>) -> Result<()> {
        let addr = addr.into();
        hyper::Server::bind(&addr)
            .serve(self)
            .await
            .context("Hyper server failed")
    }

    pub fn get_arc(&self) -> Arc<T> {
        self.0.clone()
    }
}

pub trait RemoteAddr {
    fn remote_addr(self) -> SocketAddr;
}

impl RemoteAddr for &AddrStream {
    fn remote_addr(self) -> SocketAddr {
        AddrStream::remote_addr(self)
    }
}

#[cfg(feature = "tokio-rustls")]
impl RemoteAddr for &crate::tls::TlsStream {
    fn remote_addr(self) -> SocketAddr {
        self.remote_addr
    }
}

impl<Req: RemoteAddr, T> Service<Req> for DispatchServer<T> {
    type Response = DispatchServerConn<T>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        std::future::ready(Ok(DispatchServerConn {
            addr: req.remote_addr(),
            app: self.0.clone(),
        }))
    }
}

impl<T: Dispatch> Service<Request<Body>> for DispatchServerConn<T> {
    type Response = Response<Body>;
    type Error = Infallible;
    #[allow(clippy::type_complexity)]
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

pub(crate) async fn helper<T: Dispatch>(
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
            let uuid = uuid::Uuid::new_v4();
            log::error!("New unhandled error message {}: {:?}", uuid, e);
            let mut res = respond::html(format!(
                r#"
<!DOCTYPE html>
<html>
  <head>
    <title>Unhandled error</title>
  </head>
  <body>
    <h1>Unhandled error</h1>
    <p>Error code is <code>{uuid}</code></p>
  </body>
</html>"#,
                uuid = uuid
            ));
            *res.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
            res
        }
    };
    Ok(res)
}
