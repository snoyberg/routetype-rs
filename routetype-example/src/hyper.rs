use askama::Template;
use routetype_hyper::*;
use std::sync::atomic::AtomicUsize;
#[derive(Route, Clone, PartialEq, Debug)]
enum MyRoute {
    #[route("/")]
    Home,
    #[route("css/style.css")]
    Style,
    #[route("/hello/{name}")]
    Hello { name: String },
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    counter: usize,
    style_css: String,
    greetings: Vec<Greet>,
}

struct Greet {
    name: String,
    route: String,
}

impl From<&str> for Greet {
    fn from(name: &str) -> Self {
        Greet {
            name: name.to_owned(),
            route: MyRoute::Hello {
                name: name.to_owned(),
            }
            .render(),
        }
    }
}

async fn get_home(input: DispatchInput<MyApp>) -> Result<DispatchOutput> {
    let counter = input
        .app
        .counter
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let style_css = MyRoute::Style.render(); // FIXME use a OnceCell
    let greetings = vec![
        "Alice".into(),
        "Bob".into(),
        "Charlie".into(),
        "<super dangerous>".into(),
    ];
    respond::askama(HomeTemplate {
        counter,
        style_css,
        greetings,
    })
}

#[derive(Default)]
struct MyApp {
    counter: AtomicUsize,
}

#[async_trait]
impl Dispatch for MyApp {
    type Route = MyRoute;

    async fn dispatch(input: DispatchInput<Self>, route: Self::Route) -> Result<DispatchOutput> {
        match route {
            MyRoute::Home => get_home(input).await, // FIXME std::convert::TryFrom::try_from(get_home(input).await),
            MyRoute::Style => Ok(get_style(input).await), // std::convert::TryFrom::try_from(get_style(input).await),
            MyRoute::Hello { name } => get_hello(input, name).await, // std::convert::TryFrom::try_from(get_hello(input, name).await),
        }
    }
}

async fn get_style(_input: DispatchInput<MyApp>) -> DispatchOutput {
    respond::css("h1 { color: red }")
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
    home: String,
    style_css: String,
}

async fn get_hello(_input: DispatchInput<MyApp>, name: String) -> Result<DispatchOutput> {
    respond::askama(HelloTemplate {
        name,
        home: MyRoute::Home.render(),
        style_css: MyRoute::Style.render(),
    })
}

#[tokio::main]
async fn main() {
    MyApp::default()
        .into_server()
        .run(([127, 0, 0, 1], 3000))
        .await;
}
