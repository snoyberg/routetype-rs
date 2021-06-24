use std::task::Poll;

use hyper::{body::HttpBody, HeaderMap};
use tonic::body::BoxBody;

use super::*;

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

impl<T> DispatchServer<T> {
    pub fn with_grpc<GrpcService, F>(self, f: F) -> DispatchServerWithGrpc<T, GrpcService, F>
    where
        GrpcService:
            Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
        GrpcService::Future: Send,
        //GrpcService::Error: std::error::Error + Send + Sync,
        F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static,
    {
        DispatchServerWithGrpc {
            arc: self.0,
            f,
            _phantom: std::marker::PhantomData,
        }
    }
}

pub struct DispatchServerWithGrpc<T, GrpcService, F> {
    arc: Arc<T>,
    f: F,
    _phantom: std::marker::PhantomData<GrpcService>,
}

impl<T, GrpcService, F: Clone> Clone for DispatchServerWithGrpc<T, GrpcService, F> {
    fn clone(&self) -> Self {
        DispatchServerWithGrpc {
            arc: self.arc.clone(),
            f: self.f.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Req, T, GrpcService, F> Service<Req> for DispatchServerWithGrpc<T, GrpcService, F>
where
    Req: crate::RemoteAddr,
    T: Dispatch,
    GrpcService:
        Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
    GrpcService::Future: Send,
    F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static,
{
    type Response = DispatchServerWithGrpcConn<T, GrpcService, F>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Req) -> Self::Future {
        std::future::ready(Ok(DispatchServerWithGrpcConn {
            addr: req.remote_addr(),
            arc: self.arc.clone(),
            f: self.f.clone(),
            _phantom: std::marker::PhantomData,
        }))
    }
}
impl<T, GrpcService, F> Service<Request<Body>> for DispatchServerWithGrpcConn<T, GrpcService, F>
where
    T: Dispatch,
    GrpcService:
        Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
    GrpcService::Future: Send,
    F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static,
{
    type Response = Response<EitherBody<BoxBody, Body>>;
    type Error = Error;
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
        Box::pin(helper(self.addr, self.arc.clone(), self.f.clone(), req))
    }
}

pub struct DispatchServerWithGrpcConn<T, GrpcService, F> {
    addr: SocketAddr,
    arc: Arc<T>,
    f: F,
    _phantom: std::marker::PhantomData<GrpcService>,
}

async fn helper<T, GrpcService, F>(
    remote: SocketAddr,
    app: Arc<T>,
    mut make_grpc_service: F,
    request: Request<Body>,
) -> Result<Response<EitherBody<BoxBody, Body>>, Error>
where
    T: Dispatch,
    GrpcService:
        Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
    GrpcService::Future: Send,
    //GrpcService::Error: std::error::Error + Send + Sync,
    F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static,
{
    if request.headers().get("content-type").map(|x| x.as_bytes()) == Some(b"application/grpc") {
        let res = make_grpc_service(app).call(request).await;
        res.map(|res| res.map(EitherBody::Left))
            .map_err(Error::from)
    } else {
        let res = crate::helper(remote, app, request).await;
        res.map(|res| res.map(EitherBody::Right))
            .map_err(Error::from)
    }
}

pub enum EitherBody<A, B> {
    Left(A),
    Right(B),
}

impl<A, B> HttpBody for EitherBody<A, B>
where
    A: HttpBody + Send + Unpin,
    B: HttpBody<Data = A::Data> + Send + Unpin,
    A::Error: Into<Error>,
    B::Error: Into<Error>,
{
    type Data = A::Data;
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

    fn is_end_stream(&self) -> bool {
        match self {
            EitherBody::Left(b) => b.is_end_stream(),
            EitherBody::Right(b) => b.is_end_stream(),
        }
    }

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.get_mut() {
            EitherBody::Left(b) => Pin::new(b).poll_data(cx).map(map_option_err),
            EitherBody::Right(b) => Pin::new(b).poll_data(cx).map(map_option_err),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        match self.get_mut() {
            EitherBody::Left(b) => Pin::new(b).poll_trailers(cx).map_err(Into::into),
            EitherBody::Right(b) => Pin::new(b).poll_trailers(cx).map_err(Into::into),
        }
    }
}

fn map_option_err<T, U: Into<Error>>(err: Option<Result<T, U>>) -> Option<Result<T, Error>> {
    err.map(|e| e.map_err(Into::into))
}

impl<T, GrpcService, F> DispatchServerWithGrpc<T, GrpcService, F>
where
    T: Dispatch,
    GrpcService:
        Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
    GrpcService::Future: Send,
    //GrpcService::Error: std::error::Error + Send + Sync,
    F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static,
{
    pub async fn run(self, addr: impl Into<SocketAddr>) -> Result<()> {
        let addr = addr.into();
        hyper::Server::bind(&addr)
            .serve(self)
            .await
            .context("Hyper server failed")
    }
}
