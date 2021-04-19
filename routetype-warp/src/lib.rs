pub use routetype::Route;
pub use warp::{Filter, serve, Reply};
pub use async_trait::async_trait;
use std::{convert::Infallible, sync::Arc};

pub fn route_filter<R: Route>() -> impl Filter<Error = warp::Rejection, Extract = (R,)> + Clone + Send + Sync + 'static {
    route_filter_option().and_then(|r: Option<R>| async move {
        r.ok_or_else(warp::reject::reject)
    })
}

pub fn route_filter_option<R: Route>() -> impl Filter<Error = std::convert::Infallible, Extract = (Option<R>,)> + Clone + Send + Sync + 'static {
    use warp::filters::{path::{full, FullPath}, query::raw};
    let both = raw().and(full()).map(|query: String, path: FullPath| R::parse_strs(path.as_str(), &query));
    let just_path = full().map(|path: FullPath| R::parse_strs(path.as_str(), ""));
    both.or(just_path).unify()
}

#[async_trait]
pub trait Dispatch: Sized + Send + Sync + 'static {
    type Route: routetype::Route;

    async fn dispatch(self: Arc<Self>, route: Self::Route) -> warp::reply::Response;
    async fn not_found(self: Arc<Self>) -> warp::reply::Response {
        warp::reply::with_status(
            warp::reply::html("<h1>Not found</h1>"),
            warp::http::StatusCode::NOT_FOUND,
        ).into_response()
    }

    fn into_filter(self) -> warp::filters::BoxedFilter<(warp::reply::Response,)> {
        dispatch_filter(self).boxed()
    }
}

pub fn dispatch_filter<App: Dispatch>(app: App) -> impl Filter<Error = Infallible, Extract = (warp::reply::Response,)> + Clone + Send + Sync + 'static {
    let app = std::sync::Arc::new(app);
    route_filter_option::<App::Route>().and_then(move |route: Option<App::Route>| {
        let app = app.clone();
        async move {
            match route {
                Some(route) => Ok::<_, Infallible>(app.dispatch(route).await),
                None => Ok(app.not_found().await),
            }
        }
    })
}
