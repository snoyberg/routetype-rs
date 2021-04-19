use routetype_warp::*;
use std::convert::Infallible;
use std::sync::Arc;

#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("style.css")]
    Style,
    #[route("hello/{name}")]
    Hello { name: String },
    #[route("foo?bar={bar}")]
    Foo { bar: i32 },
}

fn get_home(_app: Arc<MyApp>) -> impl warp::Reply {
    warp::reply::html("<h1>Hello World!</h1>")
}

struct MyApp;

#[async_trait]
impl Dispatch for MyApp {
    type Route = MyRoute;

    async fn dispatch(self: Arc<Self>, route: Self::Route) -> warp::reply::Response {
        match route {
            MyRoute::Home => warp::Reply::into_response(get_home(self)),
            MyRoute::Style => todo!(),
            MyRoute::Hello { name } => todo!(),
            MyRoute::Foo { bar } => todo!(),
        }
    }
}

#[tokio::main]
async fn main() {
    serve(MyApp.into_filter()).run(([127, 0, 0, 1], 3000)).await;
}
