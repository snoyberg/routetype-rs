use std::task::Poll;

use hyper::{HeaderMap, body::HttpBody, service::{make_service_fn, service_fn}};
use tonic::body::BoxBody;

use super::*;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

impl<T: Dispatch> DispatchServer<T> {
    pub async fn run_with_grpc<GrpcService, F>(self, addr: impl Into<SocketAddr>, make_grpc_service: F)
    where
        GrpcService: Service<Request<Body>, Response = Response<BoxBody>, Error = Error> + 'static + Send,
        GrpcService::Future: Send,
        //GrpcService::Error: std::error::Error + Send + Sync,
        F: FnMut(Arc<T>) -> GrpcService + Send + Clone + 'static
    {
        let web = self.0;
        let addr = addr.into();
        let server = hyper::Server::bind(&addr).serve(make_service_fn(move |conn: &AddrStream| {
            let web = web.clone();
            let mut make_grpc_service = make_grpc_service.clone();
            let addr = conn.remote_addr();
            std::future::ready(Ok::<_, Infallible>(service_fn(move |req| {
                let mut web_service = DispatchServerConn {
                    addr,
                    app: web.clone(),
                };
                let mut grpc_service = make_grpc_service(web.clone());
                async move {
                    if req.headers().get("content-type").map(|x| x.as_bytes())
                        == Some(b"application/grpc")
                    {
                        let res = grpc_service.call(req).await;
                        res.map(|res| res.map(EitherBody::Left))
                            .map_err(Error::from)
                    } else {
                        let res = web_service.call(req).await;
                        res.map(|res| res.map(EitherBody::Right))
                            .map_err(Error::from)
                    }
                }
            })))
        }));
        if let Err(e) = server.await {
            panic!("Hyper server exited with error: {}", e);
        }
    }
}

enum EitherBody<A, B> {
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
