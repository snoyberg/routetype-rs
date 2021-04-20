pub use async_trait::async_trait;
pub use routetype::{Route, RouteError};
use std::{convert::Infallible, sync::Arc};
pub use warp::{serve, Filter, Reply};

/* Would be nice to be able to generate a redirect from a filter like this...

/// Extract the route, redirect on a normalization failure and rejecting on a missing path
pub fn route_filter<R: Route>(
) -> impl Filter<Error = warp::Rejection, Extract = (R,)> + Clone + Send + Sync + 'static {
    route_filter_option().and_then(|r: Option<R>| async move { r.ok_or_else(warp::reject::reject) })
}

/// Extract the optional route, redirect on a normalization failure
pub fn route_filter_option<R: Route>(
) -> impl Filter<Error = std::convert::Infallible, Extract = (Option<R>,)> + Clone + Send + Sync + 'static
{
    route_filter_result().and_then(|r: Result<R, RouteError>| async move {
        match r {
            Ok(r) => Ok(Some(r)),
            Err(RouteError::NoMatch) => Ok(None),
            Err(RouteError::NormalizationFailed(s)) => {
                let uri: warp::http::Uri = s.parse().expect("Route parsing gave an invalid URI");
                Err(warp::redirect::permanent(uri))
            }
        }
    })
}
*/

/// Attempt to extract the route
pub fn route_filter_result<R: Route>(
) -> impl Filter<Error = std::convert::Infallible, Extract = (Result<R, RouteError>,)>
       + Clone
       + Send
       + Sync
       + 'static {
    use warp::filters::{
        path::{full, FullPath},
        query::raw,
    };
    let both = raw()
        .and(full())
        .map(|query: String, path: FullPath| R::parse_strs(path.as_str(), &query));
    let just_path = full().map(|path: FullPath| R::parse_str(path.as_str()));
    both.or(just_path).unify()
}

#[async_trait]
pub trait Dispatch: Sized + Send + Sync + 'static {
    type Route: routetype::Route;

    async fn dispatch(self: Arc<Self>, route: Self::Route) -> warp::reply::Response;
    async fn not_found(self: Arc<Self>) -> warp::reply::Response {
        default_not_found().into_response()
    }

    fn into_filter(self) -> warp::filters::BoxedFilter<(warp::reply::Response,)> {
        dispatch_filter(self).boxed()
    }
}

pub fn default_not_found() -> impl warp::Reply {
    warp::reply::with_status(
        warp::reply::html("<h1>Not found</h1>"),
        warp::http::StatusCode::NOT_FOUND,
    )
}

pub fn dispatch_filter<App: Dispatch>(
    app: App,
) -> impl Filter<Error = Infallible, Extract = (warp::reply::Response,)> + Clone + Send + Sync + 'static
{
    let app = std::sync::Arc::new(app);
    route_filter_result::<App::Route>().and_then(move |route: Result<App::Route, RouteError>| {
        let app = app.clone();
        async move {
            Ok::<_, Infallible>(match route {
                Ok(route) => app.dispatch(route).await,
                Err(RouteError::NoMatch) => app.not_found().await,
                Err(RouteError::NormalizationFailed(dest)) => {
                    let uri: warp::http::Uri = dest
                        .parse()
                        .expect("Normalization failure contained invalid URI");
                    warp::redirect::permanent(uri).into_response()
                }
            })
        }
    })
}
