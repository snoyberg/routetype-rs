use routetype_warp::*;
use std::convert::Infallible;

#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("css/style.css")]
    Style,
    #[route("/hello/{name}")]
    Hello { name: String },
}

async fn get_home() -> impl Reply {
    warp::reply::html(format!(
        "<link rel='stylesheet' href='{}'><h1>Hello World!</h1><a href='{}'>Hello!</a>",
        MyRoute::Style.render(),
        MyRoute::Hello {
            name: "Alice".to_owned()
        }
        .render(),
    ))
}

async fn get_style() -> impl Reply {
    warp::reply::with_header(
        "h1 { color: red }",
        "Content-Type",
        "text/css; charset=utf-8",
    )
}

async fn get_hello(name: String) -> impl Reply {
    warp::reply::html(format!("Hello {}", name))
}

#[tokio::main]
async fn main() {
    let app = route_filter_result().and_then(|route| async move {
        // This could be automatically derived in theory
        Ok::<_, Infallible>(match route {
            Ok(MyRoute::Home) => get_home().await.into_response(),
            Ok(MyRoute::Style) => get_style().await.into_response(),
            Ok(MyRoute::Hello { name }) => get_hello(name).await.into_response(),
            Err(RouteError::NoMatch) => default_not_found().into_response(),
            Err(RouteError::NormalizationFailed(dest)) => {
                let uri: warp::http::Uri = dest
                    .parse()
                    .expect("Normalization failure contained invalid URI");
                warp::redirect::permanent(uri).into_response()
            }
        })
    });
    serve(app).run(([127, 0, 0, 1], 3000)).await;
}
